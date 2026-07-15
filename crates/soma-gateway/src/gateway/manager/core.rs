use crate::config::{GatewayConfig, UpstreamConfig};

use super::{GatewayManager, GatewayManagerError};

impl GatewayManager {
    pub fn reload(&self, next: GatewayConfig) -> Result<(), GatewayManagerError> {
        self.reload_config(next)
    }

    pub fn upstream_config(&self, name: &str) -> Option<UpstreamConfig> {
        self.config
            .read()
            .expect("gateway config poisoned")
            .upstream
            .iter()
            .find(|upstream| upstream.name == name)
            .cloned()
    }

    #[cfg(feature = "oauth")]
    pub async fn upstream_oauth_access_token(
        &self,
        upstream: &UpstreamConfig,
        subject: &str,
    ) -> Result<Option<String>, GatewayManagerError> {
        if upstream.oauth.is_none() {
            return Ok(None);
        }
        let runtime = self
            .oauth_runtime
            .read()
            .expect("gateway oauth runtime poisoned")
            .clone()
            .ok_or_else(|| {
                GatewayManagerError::OAuth(format!(
                    "upstream `{}` is not connected with OAuth",
                    upstream.name
                ))
            })?;
        let auth_upstream = crate::gateway::oauth::to_soma_auth_upstream_config(upstream)
            .map_err(|error| GatewayManagerError::OAuth(error.to_string()))?;
        let client = runtime
            .cache
            .get_or_build(&auth_upstream, subject)
            .await
            .map_err(|error| GatewayManagerError::OAuth(error.to_string()))?;
        client
            .get_access_token()
            .await
            .map(Some)
            .map_err(|error| GatewayManagerError::OAuth(error.to_string()))
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
