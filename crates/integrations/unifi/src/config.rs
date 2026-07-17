use std::fmt;

use serde::{Deserialize, Serialize};

/// UniFi controller connection config.
///
/// `Debug` redacts [`api_key`](Self::api_key) so this can't leak into logs or
/// traces through an incidental `{:?}` — only its length is shown.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UnifiConfig {
    /// Controller base URL, e.g. `https://unifi.local` (`UNIFI_URL`).
    pub url: String,
    /// API key for the `X-API-KEY` header (`UNIFI_API_KEY`).
    pub api_key: String,
    /// Site name (`UNIFI_SITE`, default `"default"`).
    pub site: String,
    /// Skip TLS certificate verification; required for self-signed certs.
    pub skip_tls_verify: bool,
    /// Legacy controller mode: no `/proxy/network` prefix, typically port 8443.
    pub legacy: bool,
}

impl Default for UnifiConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            api_key: String::new(),
            site: "default".to_string(),
            skip_tls_verify: true,
            legacy: false,
        }
    }
}

impl fmt::Debug for UnifiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnifiConfig")
            .field("url", &self.url)
            .field(
                "api_key",
                &format_args!("<redacted, {} bytes>", self.api_key.len()),
            )
            .field("site", &self.site)
            .field("skip_tls_verify", &self.skip_tls_verify)
            .field("legacy", &self.legacy)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_api_key() {
        let cfg = UnifiConfig {
            api_key: "super-secret-key".to_string(),
            ..UnifiConfig::default()
        };

        let debug = format!("{cfg:?}");

        assert!(!debug.contains("super-secret-key"));
        assert!(debug.contains("redacted"));
    }
}
