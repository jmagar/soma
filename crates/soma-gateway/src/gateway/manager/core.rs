use crate::config::GatewayConfig;

use super::{GatewayManager, GatewayManagerError};

impl GatewayManager {
    pub fn reload(&self, next: GatewayConfig) -> Result<(), GatewayManagerError> {
        self.reload_config(next)
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
