//! Per-provider OAuth config structs (`GoogleConfig`, `AutheliaConfig`,
//! `GitHubConfig`) and their default-value helpers, split out of
//! `config.rs` to keep it under the xtask patterns file-size gate. Declared
//! in `config.rs` via `#[path = "config_providers.rs"] mod config_providers;`.

use serde::{Deserialize, Serialize};
use url::Url;

use super::DEFAULT_CALLBACK_PATH;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default = "default_callback_path")]
    pub callback_path: String,
    #[serde(default = "default_google_scopes")]
    pub scopes: Vec<String>,
}

// Hand-rolled to match the `#[serde(default = "fn")]` attributes above:
// `#[derive(Default)]` would give `callback_path`/`scopes` their
// `String`/`Vec` zero values instead, since serde's per-field `default =`
// only wires into `Deserialize`, never into `impl Default`. Struct-literal
// callers using `..GoogleConfig::default()` (or `AuthConfig::default()`,
// which embeds this) need the same non-empty defaults a deserialized empty
// config would get, or `AuthConfig::validate()`'s callback-path checks
// reject them even when Google isn't configured at all.
impl Default for GoogleConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            callback_path: default_callback_path(),
            scopes: default_google_scopes(),
        }
    }
}

impl std::fmt::Debug for GoogleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleConfig")
            .field("client_id", &self.client_id)
            .field("callback_path", &self.callback_path)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutheliaConfig {
    #[serde(default)]
    pub issuer_url: Option<Url>,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default = "default_authelia_callback_path")]
    pub callback_path: String,
    #[serde(default = "default_authelia_scopes")]
    pub scopes: Vec<String>,
}

// See `impl Default for GoogleConfig` above for why this can't be derived.
impl Default for AutheliaConfig {
    fn default() -> Self {
        Self {
            issuer_url: None,
            client_id: String::new(),
            client_secret: String::new(),
            callback_path: default_authelia_callback_path(),
            scopes: default_authelia_scopes(),
        }
    }
}

impl std::fmt::Debug for AutheliaConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutheliaConfig")
            .field("issuer_url", &self.issuer_url)
            .field("client_id", &self.client_id)
            .field("callback_path", &self.callback_path)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default = "default_github_callback_path")]
    pub callback_path: String,
    #[serde(default = "default_github_scopes")]
    pub scopes: Vec<String>,
}

// See `impl Default for GoogleConfig` above for why this can't be derived.
impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            callback_path: default_github_callback_path(),
            scopes: default_github_scopes(),
        }
    }
}

impl std::fmt::Debug for GitHubConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubConfig")
            .field("client_id", &self.client_id)
            .field("callback_path", &self.callback_path)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

pub(super) fn default_callback_path() -> String {
    DEFAULT_CALLBACK_PATH.to_string()
}

pub(super) fn default_google_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

pub(super) fn default_authelia_callback_path() -> String {
    "/auth/authelia/callback".to_string()
}

pub(super) fn default_authelia_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
        "offline_access".to_string(),
    ]
}

pub(super) fn default_github_callback_path() -> String {
    "/auth/github/callback".to_string()
}

pub(super) fn default_github_scopes() -> Vec<String> {
    vec!["read:user".to_string(), "user:email".to_string()]
}
