use rtemplate_contracts::providers::{CapabilityGrant, HostCapabilities};

use crate::provider_errors::ProviderError;

#[derive(Debug, Clone, Default)]
pub struct CapabilityBroker {
    grants: Vec<CapabilityGrant>,
}

impl CapabilityBroker {
    pub fn new(grants: Vec<CapabilityGrant>) -> Self {
        Self { grants }
    }

    pub fn default_deny() -> Self {
        Self::default()
    }

    pub fn authorize(
        &self,
        provider: &str,
        action: &str,
        requested: &HostCapabilities,
    ) -> Result<(), ProviderError> {
        if requested
            .filesystem
            .as_ref()
            .map(|cap| cap.enabled)
            .unwrap_or(false)
            && !self
                .grants
                .iter()
                .any(|grant| matches!(grant, CapabilityGrant::Filesystem { .. }))
        {
            return Err(denied(provider, action, "filesystem"));
        }
        if requested
            .network
            .as_ref()
            .map(|cap| cap.enabled)
            .unwrap_or(false)
            && !self
                .grants
                .iter()
                .any(|grant| matches!(grant, CapabilityGrant::Network { .. }))
        {
            return Err(denied(provider, action, "network"));
        }
        if requested
            .env
            .as_ref()
            .map(|cap| cap.enabled)
            .unwrap_or(false)
            && !self
                .grants
                .iter()
                .any(|grant| matches!(grant, CapabilityGrant::Env { .. }))
        {
            return Err(denied(provider, action, "env"));
        }
        if requested
            .terminal
            .as_ref()
            .map(|cap| cap.enabled)
            .unwrap_or(false)
            && !self
                .grants
                .iter()
                .any(|grant| matches!(grant, CapabilityGrant::Terminal { .. }))
        {
            return Err(denied(provider, action, "terminal"));
        }
        if requested
            .browser
            .as_ref()
            .map(|cap| cap.enabled)
            .unwrap_or(false)
            && !self
                .grants
                .iter()
                .any(|grant| matches!(grant, CapabilityGrant::Browser { .. }))
        {
            return Err(denied(provider, action, "browser"));
        }
        if requested
            .github
            .as_ref()
            .map(|cap| cap.enabled)
            .unwrap_or(false)
            && !self
                .grants
                .iter()
                .any(|grant| matches!(grant, CapabilityGrant::Github { .. }))
        {
            return Err(denied(provider, action, "github"));
        }
        Ok(())
    }
}

fn denied(provider: &str, action: &str, capability: &str) -> ProviderError {
    ProviderError::new(
        "capability_denied",
        provider,
        Some(action.to_owned()),
        format!("provider requested denied {capability} capability"),
        "Grant the specific host capability in policy or disable the provider action.",
    )
}
