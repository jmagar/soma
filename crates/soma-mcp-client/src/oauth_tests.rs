use crate::config::{
    GatewayUpstreamOauthConfig, GatewayUpstreamOauthMode, GatewayUpstreamOauthRegistration,
    UpstreamConfig,
};

use super::*;

#[test]
fn adapts_client_config_into_shared_auth_upstream_config() {
    let auth_config = to_soma_auth_upstream_config(&UpstreamConfig {
        name: "oauth".to_owned(),
        url: Some("https://upstream.example/mcp".to_owned()),
        oauth: Some(GatewayUpstreamOauthConfig {
            mode: GatewayUpstreamOauthMode::AuthorizationCodePkce,
            registration: GatewayUpstreamOauthRegistration::Preregistered {
                client_id: "client".to_owned(),
                client_secret_env: Some("UPSTREAM_CLIENT_SECRET".to_owned()),
            },
            scopes: Some(vec!["read".to_owned()]),
            prefer_client_metadata_document: Some(true),
        }),
        ..UpstreamConfig::default()
    })
    .unwrap();

    assert_eq!(auth_config.name, "oauth");
    assert_eq!(
        auth_config.url.as_deref(),
        Some("https://upstream.example/mcp")
    );
    assert!(auth_config.oauth.is_some());
}
