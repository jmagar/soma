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
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
