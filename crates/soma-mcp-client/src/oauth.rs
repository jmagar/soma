use thiserror::Error;

use crate::config::{GatewayUpstreamOauthMode, GatewayUpstreamOauthRegistration, UpstreamConfig};

pub use soma_auth::config::AuthConfig;
pub use soma_auth::upstream::manager::UpstreamOauthManager;
pub use soma_auth::upstream::runtime::{build_upstream_oauth_runtime, UpstreamOauthRuntime};
pub use soma_auth::upstream::types::BeginAuthorization;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UpstreamOAuthConfigError {
    #[error("upstream oauth config requires oauth")]
    MissingOauthConfig,
    #[error("upstream oauth config requires url")]
    MissingUrl,
}

pub fn to_soma_auth_upstream_config(
    upstream: &UpstreamConfig,
) -> Result<soma_auth::upstream::config::UpstreamConfig, UpstreamOAuthConfigError> {
    let oauth = upstream
        .oauth
        .as_ref()
        .ok_or(UpstreamOAuthConfigError::MissingOauthConfig)?;
    let url = upstream
        .url
        .clone()
        .ok_or(UpstreamOAuthConfigError::MissingUrl)?;
    Ok(soma_auth::upstream::config::UpstreamConfig {
        name: upstream.name.clone(),
        url: Some(url),
        oauth: Some(soma_auth::upstream::config::UpstreamOauthConfig {
            mode: match oauth.mode {
                GatewayUpstreamOauthMode::AuthorizationCodePkce => {
                    soma_auth::upstream::config::UpstreamOauthMode::AuthorizationCodePkce
                }
            },
            registration: match &oauth.registration {
                GatewayUpstreamOauthRegistration::ClientMetadataDocument { url } => {
                    soma_auth::upstream::config::UpstreamOauthRegistration::ClientMetadataDocument {
                        url: url.clone(),
                    }
                }
                GatewayUpstreamOauthRegistration::Preregistered {
                    client_id,
                    client_secret_env,
                } => soma_auth::upstream::config::UpstreamOauthRegistration::Preregistered {
                    client_id: client_id.clone(),
                    client_secret_env: client_secret_env.clone(),
                },
                GatewayUpstreamOauthRegistration::Dynamic => {
                    soma_auth::upstream::config::UpstreamOauthRegistration::Dynamic
                }
            },
            scopes: oauth.scopes.clone(),
            prefer_client_metadata_document: oauth.prefer_client_metadata_document,
        }),
    })
}

#[cfg(test)]
#[path = "oauth_tests.rs"]
mod tests;
