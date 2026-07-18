use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Header, Validation, decode, decode_header};
use reqwest::Url;
use reqwest::header;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::AuthError;

const DEFAULT_JWKS_TTL: Duration = Duration::from_secs(60 * 60);
/// Per-request timeout on the JWKS GET. Bound aggressively (5s) so a slow
/// upstream JWKS endpoint cannot starve a tokio worker holding the JWKS
/// write lock. Token exchange / refresh keep the provider's own looser
/// timeout because those can legitimately take longer.
const JWKS_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub(crate) struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    kid: String,
    #[serde(default)]
    alg: Option<String>,
    n: String,
    e: String,
}

#[derive(Clone, Debug)]
struct CachedJwks {
    jwks: Jwks,
    expires_at: Instant,
}

/// Shared RS256 ID-token verifier for OIDC-shaped upstream providers
/// (Google, Authelia). Caches the provider's JWKS document and validates
/// signature, expiry, audience, and issuer on every [`Self::verify`] call.
///
/// `Clone` (all fields are cheap-to-clone handles: `Arc`, `String`,
/// `reqwest::Client`, `Url`) so `GoogleProvider`/`AutheliaProvider` can stay
/// `#[derive(Clone)]` themselves, matching their pre-existing public API.
#[derive(Clone)]
pub(crate) struct OidcVerifier {
    provider_id: &'static str,
    issuer: String,
    jwks_endpoint: Url,
    http: reqwest::Client,
    jwks_cache: Arc<RwLock<Option<CachedJwks>>>,
}

impl OidcVerifier {
    pub(crate) fn new(
        provider_id: &'static str,
        issuer: String,
        jwks_endpoint: Url,
        http: reqwest::Client,
    ) -> Self {
        Self {
            provider_id,
            issuer,
            jwks_endpoint,
            http,
            jwks_cache: Arc::new(RwLock::new(None)),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_jwks_endpoint(mut self, jwks_endpoint: Url) -> Self {
        self.jwks_endpoint = jwks_endpoint;
        self
    }

    pub(crate) async fn verify(
        &self,
        id_token: &str,
        audience: &str,
    ) -> Result<IdTokenClaims, AuthError> {
        let header = decode_header(id_token).map_err(|error| {
            AuthError::Storage(format!("verify {} id_token: {error}", self.provider_id))
        })?;
        validate_header_alg(self.provider_id, &header)?;
        let kid = header.kid.ok_or_else(|| {
            AuthError::Storage(format!(
                "{} id_token is missing a key id",
                self.provider_id
            ))
        })?;
        let key = self.find_jwk_for_kid(&kid).await?;
        if let Some(alg) = key.alg.as_deref()
            && alg != "RS256"
        {
            return Err(AuthError::Storage(format!(
                "{} JWKS key `{}` uses unsupported algorithm `{alg}`",
                self.provider_id, key.kid
            )));
        }

        let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e).map_err(|error| {
            AuthError::Storage(format!(
                "build {} id_token decoding key: {error}",
                self.provider_id
            ))
        })?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.leeway = 0;
        validation.set_audience(&[audience]);

        let claims = decode::<IdTokenClaims>(id_token, &decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|error| {
                AuthError::Storage(format!("invalid {} id_token: {error}", self.provider_id))
            })?;

        if claims.iss != self.issuer {
            return Err(AuthError::Storage(format!(
                "invalid {} id_token issuer `{}`",
                self.provider_id, claims.iss
            )));
        }

        Ok(claims)
    }

    async fn find_jwk_for_kid(&self, kid: &str) -> Result<Jwk, AuthError> {
        let jwks = self.fetch_jwks().await?;
        if let Some(key) = jwks.keys.into_iter().find(|key| key.kid == kid) {
            return Ok(key);
        }

        debug!(
            provider = self.provider_id,
            kid, "jwks cache miss for token key id; refreshing"
        );
        self.refresh_jwks()
            .await?
            .keys
            .into_iter()
            .find(|key| key.kid == kid)
            .ok_or_else(|| {
                AuthError::Storage(format!(
                    "{} id_token key id was not found in JWKS",
                    self.provider_id
                ))
            })
    }

    async fn fetch_jwks(&self) -> Result<Jwks, AuthError> {
        if let Some(jwks) = self.cached_jwks().await {
            debug!(provider = self.provider_id, "jwks cache hit");
            return Ok(jwks);
        }

        let jwks = {
            let mut cache = self.jwks_cache.write().await;
            if let Some(cached) = cache
                .as_ref()
                .filter(|cached| cached.expires_at > Instant::now())
            {
                debug!(provider = self.provider_id, "jwks cache hit after refresh lock");
                cached.jwks.clone()
            } else {
                self.refresh_jwks_locked(&mut cache).await?
            }
        };
        Ok(jwks)
    }

    async fn refresh_jwks(&self) -> Result<Jwks, AuthError> {
        let mut cache = self.jwks_cache.write().await;
        self.refresh_jwks_locked(&mut cache).await
    }

    async fn refresh_jwks_locked(
        &self,
        cache: &mut Option<CachedJwks>,
    ) -> Result<Jwks, AuthError> {
        let response = self
            .http
            .get(self.jwks_endpoint.clone())
            .timeout(JWKS_FETCH_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                warn!(provider = self.provider_id, error = %error, "jwks request failed");
                AuthError::Storage(format!("fetch {} jwks: {error}", self.provider_id))
            })?;
        let status = response.status();
        let ttl = jwks_ttl(response.headers());
        let response = response.error_for_status().map_err(|error| {
            warn!(provider = self.provider_id, error = %error, "jwks request returned error status");
            AuthError::Storage(format!("{} jwks endpoint error: {error}", self.provider_id))
        })?;
        let _ = status;
        let jwks = response.json::<Jwks>().await.map_err(|error| {
            warn!(provider = self.provider_id, error = %error, "jwks payload unreadable");
            AuthError::Storage(format!("decode {} jwks response: {error}", self.provider_id))
        })?;

        *cache = Some(CachedJwks {
            jwks: jwks.clone(),
            expires_at: Instant::now() + ttl,
        });

        Ok(jwks)
    }

    async fn cached_jwks(&self) -> Option<Jwks> {
        let cache = self.jwks_cache.read().await;
        cache
            .as_ref()
            .filter(|cached| cached.expires_at > Instant::now())
            .map(|cached| cached.jwks.clone())
    }
}

fn jwks_ttl(headers: &header::HeaderMap) -> Duration {
    headers
        .get(header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_max_age)
        .map_or(DEFAULT_JWKS_TTL, Duration::from_secs)
}

fn parse_max_age(cache_control: &str) -> Option<u64> {
    cache_control.split(',').find_map(|directive| {
        let directive = directive.trim();
        let value = directive.strip_prefix("max-age=")?;
        value.parse::<u64>().ok()
    })
}

fn validate_header_alg(provider_id: &str, header: &Header) -> Result<(), AuthError> {
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::Storage(format!(
            "verify {provider_id} id_token: unsupported algorithm `{:?}`",
            header.alg
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_max_age_reads_cache_control_max_age() {
        assert_eq!(super::parse_max_age("public, max-age=3600"), Some(3600));
        assert_eq!(super::parse_max_age("no-cache"), None);
    }
}
