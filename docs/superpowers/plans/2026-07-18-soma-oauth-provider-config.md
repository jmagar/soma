# Soma OAuth Provider Config Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `soma` binary's own pre-flight validation (`soma setup`), `.env` persistence, plugin-option/env-var registry, and `soma doctor` output aware of the Authelia/GitHub providers added to `soma-auth` in `docs/superpowers/plans/2026-07-18-oauth-provider-trait.md`, instead of hardcoding "Google is the only OAuth provider."

**Architecture:** `apps/soma/src/bootstrap.rs`'s `http_auth_policy` already resolves OAuth config by calling `soma_integrations::auth::soma_auth_config_builder().build_from_sources(std::env::vars())` directly against the raw process environment — it does **not** go through `soma`'s own typed `soma_config::Config`/`AuthConfig` struct at all. That means an operator can already set `SOMA_MCP_AUTHELIA_CLIENT_ID` etc. and use the providers end-to-end at runtime with **zero changes to `apps/soma` or `crates/soma/runtime`**. What's not yet aware of the new providers is the product-support view used by pre-flight checks and generated docs: `crates/soma/config/src/config.rs` (typed config and env loading), `crates/soma/config/src/env_registry.rs` (canonical env-var/plugin-option metadata), `crates/soma/cli/src/setup.rs` (validation and `.env` persistence), and `crates/soma/cli/src/doctor/checks.rs` (the doctor auth label). `crates/soma/domain` owns action/error/scope contracts and is intentionally unchanged; OAuth configuration belongs in `soma-config` after the contracts-crate split.

**Tech Stack:** Rust 2024, same as the rest of `soma`. No new dependencies.

## Global Constraints

- Depends entirely on `docs/superpowers/plans/2026-07-18-oauth-provider-trait.md` being merged first — do not start this plan until that one's Task 13 verification pass is green.
- `crates/soma/config/src/config.rs`'s existing flat-field style for `AuthConfig` (no nested per-provider structs, unlike `soma_auth::config::AuthConfig`) is intentional and must be preserved — this file is a thin typed mirror of env vars for pre-flight/doc purposes, not the actual OAuth runtime config.
- Every new env var name must exactly match the corresponding `soma_auth` `env_key(&prefix, "...")` suffix from Plan 1 Task 7 (prefix is always `SOMA_MCP` in this app), so `soma doctor`/`soma setup` validate the SAME variables `AuthConfigBuilder::build_from_sources` actually reads at runtime. Mismatches here are silent, not compile-time — double check spelling against Plan 1's `key_a_*`/`key_gh_*`/`key_default_provider` variable list.
- `cargo test -p soma-config` and `cargo test -p soma-cli` plus `cargo clippy --workspace --all-targets -- -D warnings` must pass before the plan is complete.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/soma/config/src/config.rs` | Modify | Add `authelia_issuer_url`/`authelia_client_id`/`authelia_client_secret`/`github_client_id`/`github_client_secret`/`default_provider` typed fields + `SOMA_MCP_*` env loading |
| `crates/soma/config/src/config_tests.rs` | Modify | Cover typed config loading with the existing serialized env-test harness |
| `crates/soma/config/src/env_registry.rs` | Modify | Register all 10 new env-var specs (canonical docs/plugin-option table) |
| `crates/soma/config/src/env_registry_tests.rs` | Modify | Cover the new specs |
| `crates/soma/cli/Cargo.toml` | Modify | Add `soma-auth` as a dev dependency for validator-parity coverage |
| `crates/soma/cli/src/setup.rs` | Modify | `check_auth`: "at least one provider" instead of "Google required"; `write_env`: persist the new fields |
| `crates/soma/cli/src/doctor/checks.rs` | Modify | `check_auth_config`: print which provider(s) are actually configured instead of a hardcoded `"(Google)"` |
| `crates/soma/cli/src/doctor/checks_tests.rs` | Modify | Cover provider labels in the existing sibling test module |
| `docs/ENV.md`, `.env.example`, `docs/generated/plugin-settings.md`, plugin metadata | Regenerate | Refresh generated env/config surfaces with `cargo xtask generate-docs` |

---

### Task 1: `soma-config/config.rs` — typed fields + env loading

**Files:**
- Modify: `crates/soma/config/src/config.rs`
- Modify: `crates/soma/config/src/config_tests.rs`

**Interfaces:**
- Produces: `AuthConfig.authelia_issuer_url: Option<String>`, `AuthConfig.authelia_client_id: Option<String>`, `AuthConfig.authelia_client_secret: Option<String>`, `AuthConfig.github_client_id: Option<String>`, `AuthConfig.github_client_secret: Option<String>`, `AuthConfig.default_provider: Option<String>`. Consumed by Task 3 (`setup.rs`) and Task 4 (`doctor/checks.rs`).

- [ ] **Step 1: Add the 6 fields to the `AuthConfig` struct**

Right after `pub google_client_secret: Option<String>,` in `crates/soma/config/src/config.rs`, add:

```rust
    pub authelia_issuer_url: Option<String>,
    pub authelia_client_id: Option<String>,
    pub authelia_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub default_provider: Option<String>,
```

- [ ] **Step 2: Add them to `impl Default for AuthConfig`**

Right after `google_client_secret: None,`, add:

```rust
            authelia_issuer_url: None,
            authelia_client_id: None,
            authelia_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            default_provider: None,
```

- [ ] **Step 3: Load them from env in `Config::load()`**

Right after the existing block:

```rust
        env_opt_str(
            "SOMA_MCP_GOOGLE_CLIENT_SECRET",
            &mut config.mcp.auth.google_client_secret,
        );
```

add:

```rust
        env_opt_str(
            "SOMA_MCP_AUTHELIA_ISSUER_URL",
            &mut config.mcp.auth.authelia_issuer_url,
        );
        env_opt_str(
            "SOMA_MCP_AUTHELIA_CLIENT_ID",
            &mut config.mcp.auth.authelia_client_id,
        );
        env_opt_str(
            "SOMA_MCP_AUTHELIA_CLIENT_SECRET",
            &mut config.mcp.auth.authelia_client_secret,
        );
        env_opt_str(
            "SOMA_MCP_GITHUB_CLIENT_ID",
            &mut config.mcp.auth.github_client_id,
        );
        env_opt_str(
            "SOMA_MCP_GITHUB_CLIENT_SECRET",
            &mut config.mcp.auth.github_client_secret,
        );
        env_opt_str(
            "SOMA_MCP_AUTH_DEFAULT_PROVIDER",
            &mut config.mcp.auth.default_provider,
        );
```

- [ ] **Step 4: Add a config-loading regression test**

Add the test to the existing sibling `crates/soma/config/src/config_tests.rs`. Reuse its `#[serial]` convention and `EnvRestore` cleanup helper; do not introduce a second process-environment locking mechanism. Exercise `Config::load()` itself so the test covers the real override path rather than calling `env_opt_str` directly.

```rust
    #[test]
    fn authelia_and_github_env_vars_populate_typed_auth_config() {
        // SAFETY / isolation: tests that mutate process env must run serially —
        // check whether this file already uses a `serial_test`-style guard or a
        // shared env-mutex for its other env-var tests; reuse that mechanism
        // instead of calling std::env::set_var bare, to avoid flaking against
        // parallel test execution in this same process.
        let vars = [
            ("SOMA_MCP_AUTHELIA_ISSUER_URL", "https://auth.example.com"),
            ("SOMA_MCP_AUTHELIA_CLIENT_ID", "authelia-id"),
            ("SOMA_MCP_AUTHELIA_CLIENT_SECRET", "authelia-secret"),
            ("SOMA_MCP_GITHUB_CLIENT_ID", "github-id"),
            ("SOMA_MCP_GITHUB_CLIENT_SECRET", "github-secret"),
            ("SOMA_MCP_AUTH_DEFAULT_PROVIDER", "authelia"),
        ];
        for (key, value) in vars {
            unsafe { std::env::set_var(key, value) };
        }
        let mut config = Config::default();
        env_opt_str(
            "SOMA_MCP_AUTHELIA_ISSUER_URL",
            &mut config.mcp.auth.authelia_issuer_url,
        );
        env_opt_str(
            "SOMA_MCP_AUTHELIA_CLIENT_ID",
            &mut config.mcp.auth.authelia_client_id,
        );
        env_opt_str(
            "SOMA_MCP_GITHUB_CLIENT_ID",
            &mut config.mcp.auth.github_client_id,
        );
        env_opt_str(
            "SOMA_MCP_AUTH_DEFAULT_PROVIDER",
            &mut config.mcp.auth.default_provider,
        );
        assert_eq!(
            config.mcp.auth.authelia_issuer_url.as_deref(),
            Some("https://auth.example.com")
        );
        assert_eq!(config.mcp.auth.github_client_id.as_deref(), Some("github-id"));
        assert_eq!(config.mcp.auth.default_provider.as_deref(), Some("authelia"));
        for (key, _) in vars {
            unsafe { std::env::remove_var(key) };
        }
    }
```

Before finalizing this step, read how the file's OTHER env-mutating tests (e.g. around the existing `SOMA_MCP_GOOGLE_CLIENT_ID` handling, if a test already exists for it) guard against cross-test env pollution — `unsafe { std::env::set_var }` is required in current Rust editions and this workspace may already have a `#[serial]`/mutex convention for these tests; follow it exactly rather than introducing a second, inconsistent pattern.

- [ ] **Step 5: Run tests**

Run: `cargo test -p soma-config config`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/soma/config/src/config.rs crates/soma/config/src/config_tests.rs
git commit -m "feat(soma): load Authelia/GitHub OAuth env vars into typed AuthConfig"
```

---

### Task 2: `env_registry.rs` — register all 10 new specs

**Files:**
- Modify: `crates/soma/config/src/env_registry.rs`
- Modify: `crates/soma/config/src/env_registry_tests.rs`

**Interfaces:**
- Produces: 10 new entries in `ENV_KEY_SPECS` — the 6 in Step 2 below plus 4 more (`SOMA_MCP_AUTHELIA_CALLBACK_PATH`, `SOMA_MCP_AUTHELIA_SCOPES`, `SOMA_MCP_GITHUB_CALLBACK_PATH`, `SOMA_MCP_GITHUB_SCOPES`) added in Step 2A. Engineering review caught that the original version of this task only registered 6 of the 10 env vars `soma-auth`'s `AuthConfigBuilder::build_from_sources` actually reads (Plan 1 Task 7 Step 4 reads `key_a_callback`/`key_a_scopes`/`key_gh_callback`/`key_gh_scopes` too) — those 4 are fully functional at runtime without this task (Plan 2's own Architecture note: `build_auth_policy` reads raw env directly), but were invisible to `soma doctor`, the canonical docs registry, and any plugin-option UI derived from `ENV_KEY_SPECS`. Consumed by anything that iterates `ENV_KEY_SPECS`/`all_specs()`/`plugin_option_mappings()` for docs generation or plugin manifest option wiring (grep `ENV_KEY_SPECS` workspace-wide before starting, to confirm nothing outside this crate hardcodes the array's length or exact ordering — `const` slices consumed elsewhere by index would be a red flag; expected to be consumed only by key/value lookup, not position).

- [ ] **Step 1: Confirm nothing depends on `ENV_KEY_SPECS`'s exact length or ordering**

Run: `grep -rn "ENV_KEY_SPECS\[" --include="*.rs" .`
Expected: no output (only iteration/lookup by key, never positional indexing). If output appears, read that call site before proceeding — appending entries could silently break it.

- [ ] **Step 2: Add the 6 specs**

Right after the existing:

```rust
    spec(
        "SOMA_MCP_GOOGLE_CLIENT_SECRET",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.google_client_secret"),
        LegacyBehavior::Canonical,
        true,
        Some("CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_SECRET"),
    ),
```

add:

```rust
    spec(
        "SOMA_MCP_AUTHELIA_ISSUER_URL",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.authelia_issuer_url"),
        LegacyBehavior::Canonical,
        false,
        Some("CLAUDE_PLUGIN_OPTION_AUTHELIA_ISSUER_URL"),
    ),
    spec(
        "SOMA_MCP_AUTHELIA_CLIENT_ID",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.authelia_client_id"),
        LegacyBehavior::Canonical,
        true,
        Some("CLAUDE_PLUGIN_OPTION_AUTHELIA_CLIENT_ID"),
    ),
    spec(
        "SOMA_MCP_AUTHELIA_CLIENT_SECRET",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.authelia_client_secret"),
        LegacyBehavior::Canonical,
        true,
        Some("CLAUDE_PLUGIN_OPTION_AUTHELIA_CLIENT_SECRET"),
    ),
    spec(
        "SOMA_MCP_GITHUB_CLIENT_ID",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.github_client_id"),
        LegacyBehavior::Canonical,
        true,
        Some("CLAUDE_PLUGIN_OPTION_GITHUB_CLIENT_ID"),
    ),
    spec(
        "SOMA_MCP_GITHUB_CLIENT_SECRET",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.github_client_secret"),
        LegacyBehavior::Canonical,
        true,
        Some("CLAUDE_PLUGIN_OPTION_GITHUB_CLIENT_SECRET"),
    ),
    spec(
        "SOMA_MCP_AUTH_DEFAULT_PROVIDER",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        Some("mcp.auth.default_provider"),
        LegacyBehavior::Canonical,
        false,
        Some("CLAUDE_PLUGIN_OPTION_AUTH_DEFAULT_PROVIDER"),
    ),
```

`secret: false` for `AUTHELIA_ISSUER_URL` and `AUTH_DEFAULT_PROVIDER` mirrors how `GOOGLE_CALLBACK_PATH`-shaped non-secret config would be classified (an issuer URL and a provider-name selector are not credentials); `secret: true` for the 4 client-id/secret pairs mirrors the existing `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` precedent exactly (note: the existing table marks even `GOOGLE_CLIENT_ID`, not just the secret, as `secret: true` — follow that same precedent for `AUTHELIA_CLIENT_ID`/`GITHUB_CLIENT_ID` for consistency, even though a client ID alone isn't normally sensitive; this file's convention treats the whole OAuth credential pair as secret-classified).

- [ ] **Step 2A: Add the 4 remaining specs (callback-path/scopes overrides)**

`crates/soma/config/src/config.rs` (Task 1) does NOT gain typed fields for these 4 — that matches the existing convention exactly (`google_callback_path`/`google_scopes` have no typed field in `soma_config::AuthConfig` either; these are power-user env-only knobs). `toml_destination: None` mirrors how this file already represents that shape (see `SOMA_SERVER_URL`'s spec earlier in `ENV_KEY_SPECS`, which also has `toml_destination: None`).

```rust
    spec(
        "SOMA_MCP_AUTHELIA_CALLBACK_PATH",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        None,
        LegacyBehavior::Advanced,
        false,
        None,
    ),
    spec(
        "SOMA_MCP_AUTHELIA_SCOPES",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        None,
        LegacyBehavior::Advanced,
        false,
        None,
    ),
    spec(
        "SOMA_MCP_GITHUB_CALLBACK_PATH",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        None,
        LegacyBehavior::Advanced,
        false,
        None,
    ),
    spec(
        "SOMA_MCP_GITHUB_SCOPES",
        EnvClassification::TrustedOperatorBootstrap,
        RuntimePlacement::Both,
        None,
        LegacyBehavior::Advanced,
        false,
        None,
    ),
```

`LegacyBehavior::Advanced` (rather than `Canonical`) mirrors how this file already classifies other power-user-only knobs (e.g. `SOMA_MCP_NO_AUTH`) — check the file's existing precedent for that enum variant's other usages before finalizing, and match whichever one is used for "works, but not part of the primary guided setup path."

- [ ] **Step 3: Extend `env_registry_tests.rs`**

Add to the existing test file:

```rust
#[test]
fn registry_contains_authelia_and_github_keys() {
    let keys: Vec<&str> = all_specs().iter().map(|spec| spec.key).collect();
    for expected in [
        "SOMA_MCP_AUTHELIA_ISSUER_URL",
        "SOMA_MCP_AUTHELIA_CLIENT_ID",
        "SOMA_MCP_AUTHELIA_CLIENT_SECRET",
        "SOMA_MCP_AUTHELIA_CALLBACK_PATH",
        "SOMA_MCP_AUTHELIA_SCOPES",
        "SOMA_MCP_GITHUB_CLIENT_ID",
        "SOMA_MCP_GITHUB_CLIENT_SECRET",
        "SOMA_MCP_GITHUB_CALLBACK_PATH",
        "SOMA_MCP_GITHUB_SCOPES",
        "SOMA_MCP_AUTH_DEFAULT_PROVIDER",
    ] {
        assert!(keys.contains(&expected), "missing {expected}");
    }
}

#[test]
fn authelia_and_github_client_credentials_are_marked_secret() {
    assert!(spec_for("SOMA_MCP_AUTHELIA_CLIENT_ID").unwrap().secret);
    assert!(spec_for("SOMA_MCP_AUTHELIA_CLIENT_SECRET").unwrap().secret);
    assert!(spec_for("SOMA_MCP_GITHUB_CLIENT_ID").unwrap().secret);
    assert!(spec_for("SOMA_MCP_GITHUB_CLIENT_SECRET").unwrap().secret);
    assert!(!spec_for("SOMA_MCP_AUTHELIA_ISSUER_URL").unwrap().secret);
    assert!(!spec_for("SOMA_MCP_AUTH_DEFAULT_PROVIDER").unwrap().secret);
}

#[test]
fn new_provider_plugin_option_mappings_are_derived_from_specs() {
    let mappings: Vec<_> = plugin_option_mappings().collect();
    assert!(mappings.contains(&(
        "CLAUDE_PLUGIN_OPTION_AUTHELIA_CLIENT_ID",
        "SOMA_MCP_AUTHELIA_CLIENT_ID"
    )));
    assert!(mappings.contains(&(
        "CLAUDE_PLUGIN_OPTION_GITHUB_CLIENT_SECRET",
        "SOMA_MCP_GITHUB_CLIENT_SECRET"
    )));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p soma-config env_registry`
Expected: all `env_registry` tests PASS, including the 3 new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/soma/config/src/env_registry.rs crates/soma/config/src/env_registry_tests.rs
git commit -m "feat(soma): register Authelia/GitHub OAuth env vars in the canonical registry"
```

---

### Task 3: `cli/setup.rs` — unblock Google-only requirement, persist new fields

**Files:**
- Modify: `crates/soma/cli/Cargo.toml`
- Modify: `crates/soma/cli/src/setup.rs`

**Interfaces:**
- Consumes: `AuthConfig.authelia_issuer_url`/`authelia_client_id`/`authelia_client_secret`/`github_client_id`/`github_client_secret`/`default_provider` (Task 1).

- [ ] **Step 1: Rewrite `check_auth`'s OAuth-mode block**

Replace:

```rust
    if config.mcp.auth.mode == AuthMode::OAuth {
        require_oauth_field(
            report,
            &config.mcp.auth.public_url,
            "missing_oauth_public_url",
            "SOMA_MCP_PUBLIC_URL is required for OAuth mode",
        );
        require_oauth_field(
            report,
            &config.mcp.auth.google_client_id,
            "missing_oauth_client_id",
            "SOMA_MCP_GOOGLE_CLIENT_ID is required for OAuth mode",
        );
        require_oauth_field(
            report,
            &config.mcp.auth.google_client_secret,
            "missing_oauth_client_secret",
            "SOMA_MCP_GOOGLE_CLIENT_SECRET is required for OAuth mode",
        );
        require_oauth_field(
            report,
            &Some(config.mcp.auth.admin_email.clone()),
            "missing_oauth_admin_email",
            "SOMA_MCP_AUTH_ADMIN_EMAIL is required for OAuth mode",
        );
    } else if config.mcp.api_token.as_deref().unwrap_or("").is_empty() {
```

with:

```rust
    if config.mcp.auth.mode == AuthMode::OAuth {
        require_oauth_field(
            report,
            &config.mcp.auth.public_url,
            "missing_oauth_public_url",
            "SOMA_MCP_PUBLIC_URL is required for OAuth mode",
        );

        let google_configured = config.mcp.auth.google_client_id.is_some();
        let authelia_configured = config.mcp.auth.authelia_client_id.is_some();
        let github_configured = config.mcp.auth.github_client_id.is_some();

        if google_configured && config.mcp.auth.google_client_secret.is_none() {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_secret",
                message: "SOMA_MCP_GOOGLE_CLIENT_SECRET is required when SOMA_MCP_GOOGLE_CLIENT_ID is set".into(),
            });
        }
        if authelia_configured {
            if config.mcp.auth.authelia_issuer_url.is_none() {
                report.blocking_failures.push(SetupFailure {
                    code: "missing_oauth_client_secret",
                    message: "SOMA_MCP_AUTHELIA_ISSUER_URL is required when SOMA_MCP_AUTHELIA_CLIENT_ID is set".into(),
                });
            }
            if config.mcp.auth.authelia_client_secret.is_none() {
                report.blocking_failures.push(SetupFailure {
                    code: "missing_oauth_client_secret",
                    message: "SOMA_MCP_AUTHELIA_CLIENT_SECRET is required when SOMA_MCP_AUTHELIA_CLIENT_ID is set".into(),
                });
            }
        }
        if github_configured && config.mcp.auth.github_client_secret.is_none() {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_secret",
                message: "SOMA_MCP_GITHUB_CLIENT_SECRET is required when SOMA_MCP_GITHUB_CLIENT_ID is set".into(),
            });
        }
        if !google_configured && !authelia_configured && !github_configured {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_id",
                message: "OAuth mode requires at least one provider: set SOMA_MCP_GOOGLE_CLIENT_ID, \
                          SOMA_MCP_AUTHELIA_CLIENT_ID (+ SOMA_MCP_AUTHELIA_ISSUER_URL), or \
                          SOMA_MCP_GITHUB_CLIENT_ID (each paired with its matching _CLIENT_SECRET)"
                    .into(),
            });
        }
        // Engineering review finding: without this check, `soma setup` can
        // pass cleanly while `soma serve` still refuses to start, because
        // the ACTUAL runtime validator — `soma_auth::config::AuthConfig::
        // validate()` (Plan 1 Task 7 Step 3) — rejects a `default_provider`
        // that doesn't name a configured provider, and this pre-flight check
        // ran on a completely separate typed struct that never looked at
        // the field at all.
        if let Some(default_provider) = &config.mcp.auth.default_provider {
            let names_a_configured_provider = match default_provider.as_str() {
                "google" => google_configured,
                "authelia" => authelia_configured,
                "github" => github_configured,
                _ => false,
            };
            if !names_a_configured_provider {
                report.blocking_failures.push(SetupFailure {
                    code: "invalid_oauth_default_provider",
                    message: format!(
                        "SOMA_MCP_AUTH_DEFAULT_PROVIDER=\"{default_provider}\" must be `google`, \
                         `authelia`, or `github`, and must name a provider that is actually \
                         configured (has its _CLIENT_ID/_CLIENT_SECRET set)"
                    ),
                });
            }
        }
        require_oauth_field(
            report,
            &Some(config.mcp.auth.admin_email.clone()),
            "missing_oauth_admin_email",
            "SOMA_MCP_AUTH_ADMIN_EMAIL is required for OAuth mode",
        );
    } else if config.mcp.api_token.as_deref().unwrap_or("").is_empty() {
```

- [ ] **Step 1B: Add a cross-check test that `check_auth` and `soma_auth::AuthConfig::validate()` agree**

Engineering review's simplicity pass flagged that this file, `soma_auth::config::AuthConfig::validate()` (Plan 1 Task 7), and `doctor/checks.rs` (Task 4) now independently re-derive the same "is at least one provider configured, with matching required fields" logic three times, with hand-spelled error strings that must be kept in lockstep by convention rather than by anything mechanical. Full delegation (`check_auth` constructing a real `soma_auth::config::AuthConfig` and calling `.validate()`) would lose this file's current per-field `code` granularity (`missing_oauth_client_secret` vs `missing_oauth_client_id`, etc.), which is real UX value for an interactive setup wizard — not worth giving up. Instead, add one test that feeds the same env-var combinations through both validators and asserts they agree on pass/fail, so drift between the three copies is caught in CI rather than relying on a code comment:

```rust
    #[test]
    fn check_auth_agrees_with_soma_auth_validate_on_provider_combinations() {
        // Every combination below must produce the same pass/fail verdict
        // from this file's `check_auth` and from `soma_auth::config::
        // AuthConfig::validate()` (via `AuthConfigBuilder::build_from_sources`
        // against the same env vars, mirroring what `apps/soma/src/bootstrap.rs`
        // actually does at runtime). If this test ever needs updating because
        // the two disagree, that disagreement IS the bug — fix whichever
        // validator is wrong, don't just update the test to match.
        let cases: &[(&[(&str, &str)], bool)] = &[
            (
                &[
                    ("SOMA_MCP_AUTH_MODE", "oauth"),
                    ("SOMA_MCP_PUBLIC_URL", "https://example.com"),
                    ("SOMA_MCP_AUTH_ADMIN_EMAIL", "admin@example.com"),
                ],
                false, // no provider configured — both must reject
            ),
            (
                &[
                    ("SOMA_MCP_AUTH_MODE", "oauth"),
                    ("SOMA_MCP_PUBLIC_URL", "https://example.com"),
                    ("SOMA_MCP_AUTH_ADMIN_EMAIL", "admin@example.com"),
                    ("SOMA_MCP_GITHUB_CLIENT_ID", "gh-id"),
                    ("SOMA_MCP_GITHUB_CLIENT_SECRET", "gh-secret"),
                ],
                true, // GitHub-only — both must accept
            ),
            (
                &[
                    ("SOMA_MCP_AUTH_MODE", "oauth"),
                    ("SOMA_MCP_PUBLIC_URL", "https://example.com"),
                    ("SOMA_MCP_AUTH_ADMIN_EMAIL", "admin@example.com"),
                    ("SOMA_MCP_GOOGLE_CLIENT_ID", "g-id"),
                    ("SOMA_MCP_GOOGLE_CLIENT_SECRET", "g-secret"),
                    ("SOMA_MCP_AUTH_DEFAULT_PROVIDER", "github"),
                ],
                false, // default_provider names an unconfigured provider — both must reject
            ),
        ];
        for (vars, should_pass) in cases {
            let soma_auth_result = soma_auth::config::AuthConfigBuilder::new()
                .env_prefix("SOMA_MCP")
                .build_from_sources(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
            assert_eq!(
                soma_auth_result.is_ok(),
                *should_pass,
                "soma_auth::AuthConfig::validate disagreement for {vars:?}"
            );

            let mut config = Config::default();
            for (key, value) in *vars {
                // Reuse this file's own env-loading helpers (env_str/env_opt_str/
                // the AUTH_MODE match) against a scoped var set rather than the
                // real process environment, so this test doesn't mutate global
                // process state. If `Config::load()` doesn't expose a way to load
                // from an explicit map rather than `std::env`, either add one
                // (small, low-risk refactor of `Config::load`) or fall back to the
                // same `unsafe { std::env::set_var }`-plus-serial-guard pattern
                // used elsewhere in this file's env-mutating tests.
                let _ = (key, value);
            }
            // ... populate `config.mcp.auth.*` from `vars` here, then:
            let mut report = SetupReport::default();
            check_auth(&config, &mut report);
            assert_eq!(
                report.blocking_failures.is_empty(),
                *should_pass,
                "check_auth disagreement for {vars:?}: {:?}",
                report.blocking_failures
            );
        }
    }
```

`Config::load()` reads the process environment. Follow `soma-config`'s existing serialized env-test pattern (`#[serial]` plus scoped restoration) if the parity test drives it directly. Add `soma-auth = { workspace = true }` under `crates/soma/cli/Cargo.toml`'s `[dev-dependencies]` so production dependency boundaries remain unchanged.

- [ ] **Step 2: Persist the new fields in `write_env`**

Replace:

```rust
        if let Some(v) = &config.mcp.auth.google_client_id {
            lines.push(dotenv_assignment("SOMA_MCP_GOOGLE_CLIENT_ID", v)?);
        }
        if let Some(v) = &config.mcp.auth.google_client_secret {
            lines.push(dotenv_assignment("SOMA_MCP_GOOGLE_CLIENT_SECRET", v)?);
        }
```

with:

```rust
        if let Some(v) = &config.mcp.auth.google_client_id {
            lines.push(dotenv_assignment("SOMA_MCP_GOOGLE_CLIENT_ID", v)?);
        }
        if let Some(v) = &config.mcp.auth.google_client_secret {
            lines.push(dotenv_assignment("SOMA_MCP_GOOGLE_CLIENT_SECRET", v)?);
        }
        if let Some(v) = &config.mcp.auth.authelia_issuer_url {
            lines.push(dotenv_assignment("SOMA_MCP_AUTHELIA_ISSUER_URL", v)?);
        }
        if let Some(v) = &config.mcp.auth.authelia_client_id {
            lines.push(dotenv_assignment("SOMA_MCP_AUTHELIA_CLIENT_ID", v)?);
        }
        if let Some(v) = &config.mcp.auth.authelia_client_secret {
            lines.push(dotenv_assignment("SOMA_MCP_AUTHELIA_CLIENT_SECRET", v)?);
        }
        if let Some(v) = &config.mcp.auth.github_client_id {
            lines.push(dotenv_assignment("SOMA_MCP_GITHUB_CLIENT_ID", v)?);
        }
        if let Some(v) = &config.mcp.auth.github_client_secret {
            lines.push(dotenv_assignment("SOMA_MCP_GITHUB_CLIENT_SECRET", v)?);
        }
        if let Some(v) = &config.mcp.auth.default_provider {
            lines.push(dotenv_assignment("SOMA_MCP_AUTH_DEFAULT_PROVIDER", v)?);
        }
```

- [ ] **Step 3: Add regression tests for the new `check_auth` branches**

Find this file's existing tests exercising `check_auth` (search `fn check_auth` call sites in `#[cfg(test)]`) and add sibling cases:

```rust
    #[test]
    fn check_auth_accepts_github_only_oauth_config() {
        let mut config = Config::default();
        config.mcp.auth.mode = AuthMode::OAuth;
        config.mcp.auth.public_url = Some("https://example.com".into());
        config.mcp.auth.github_client_id = Some("gh-id".into());
        config.mcp.auth.github_client_secret = Some("gh-secret".into());
        config.mcp.auth.admin_email = "admin@example.com".into();
        let mut report = SetupReport::default();
        check_auth(&config, &mut report);
        assert!(
            report.blocking_failures.is_empty(),
            "unexpected failures: {:?}",
            report.blocking_failures
        );
    }

    #[test]
    fn check_auth_rejects_oauth_mode_with_no_provider_configured() {
        let mut config = Config::default();
        config.mcp.auth.mode = AuthMode::OAuth;
        config.mcp.auth.public_url = Some("https://example.com".into());
        config.mcp.auth.admin_email = "admin@example.com".into();
        let mut report = SetupReport::default();
        check_auth(&config, &mut report);
        assert!(
            report
                .blocking_failures
                .iter()
                .any(|f| f.code == "missing_oauth_client_id"),
            "expected a missing_oauth_client_id failure: {:?}",
            report.blocking_failures
        );
    }

    #[test]
    fn check_auth_rejects_authelia_client_id_without_issuer_url() {
        let mut config = Config::default();
        config.mcp.auth.mode = AuthMode::OAuth;
        config.mcp.auth.public_url = Some("https://example.com".into());
        config.mcp.auth.authelia_client_id = Some("authelia-id".into());
        config.mcp.auth.authelia_client_secret = Some("authelia-secret".into());
        config.mcp.auth.admin_email = "admin@example.com".into();
        let mut report = SetupReport::default();
        check_auth(&config, &mut report);
        assert!(
            report
                .blocking_failures
                .iter()
                .any(|f| f.message.contains("SOMA_MCP_AUTHELIA_ISSUER_URL")),
            "expected a missing-issuer-url failure: {:?}",
            report.blocking_failures
        );
    }
```

Check this file's `SetupReport`/`SetupFailure` construction pattern (`SetupReport::default()` vs a builder) against the file's existing tests before finalizing — mirror whatever's already there exactly rather than guessing the constructor shape.

- [ ] **Step 4: Run tests**

Run: `cargo test -p soma-cli setup`
Expected: all `setup` tests PASS, including the 3 new ones. The pre-existing Google-only `check_auth` tests must also still pass unmodified (Google-only OAuth config is still fully valid).

Run: `cargo clippy --workspace -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/soma/cli/Cargo.toml crates/soma/cli/src/setup.rs
git commit -m "feat(soma): accept Authelia/GitHub-only OAuth config in setup pre-flight checks"
```

---

### Task 4: `cli/doctor/checks.rs` — accurate provider label

**Files:**
- Modify: `crates/soma/cli/src/doctor/checks.rs`
- Modify: `crates/soma/cli/src/doctor/checks_tests.rs`

**Interfaces:**
- Consumes: `AuthConfig.google_client_id`/`authelia_client_id`/`github_client_id` (Task 1).

- [ ] **Step 1: Replace the hardcoded `"OAuth (Google)"` label**

Replace:

```rust
        Ok(AuthPolicyKind::MountedOAuth) => {
            DoctorCheck::pass("auth", "Auth mode", "OAuth (Google)")
        }
```

Add the helper function right above `check_auth_config`:

```rust
/// Validate the typed OAuth provider mirror and return a human-readable
/// summary for the doctor report. `resolve_auth_policy_kind` only selects the
/// mounted OAuth policy from `auth.mode`; it does not validate provider
/// credentials, so doctor must reject partial provider configurations here.
fn configured_oauth_providers_label(config: &Config) -> Result<String, String> {
    let mut providers = Vec::new();
    match (
        config.mcp.auth.google_client_id.as_deref(),
        config.mcp.auth.google_client_secret.as_deref(),
    ) {
        (Some(_), Some(_)) => providers.push("Google"),
        (Some(_), None) | (None, Some(_)) => {
            return Err("Google OAuth requires both client ID and client secret".into());
        }
        (None, None) => {}
    }
    match (
        config.mcp.auth.authelia_issuer_url.as_deref(),
        config.mcp.auth.authelia_client_id.as_deref(),
        config.mcp.auth.authelia_client_secret.as_deref(),
    ) {
        (Some(_), Some(_), Some(_)) => providers.push("Authelia"),
        (None, None, None) => {}
        _ => {
            return Err(
                "Authelia OAuth requires issuer URL, client ID, and client secret".into(),
            );
        }
    }
    match (
        config.mcp.auth.github_client_id.as_deref(),
        config.mcp.auth.github_client_secret.as_deref(),
    ) {
        (Some(_), Some(_)) => providers.push("GitHub"),
        (Some(_), None) | (None, Some(_)) => {
            return Err("GitHub OAuth requires both client ID and client secret".into());
        }
        (None, None) => {}
    }
    if providers.is_empty() {
        return Err("OAuth mode requires at least one fully configured provider".into());
    }
    if let Some(default_provider) = config.mcp.auth.default_provider.as_deref() {
        let is_configured = match default_provider {
            "google" => providers.contains(&"Google"),
            "authelia" => providers.contains(&"Authelia"),
            "github" => providers.contains(&"GitHub"),
            _ => false,
        };
        if !is_configured {
            return Err(format!(
                "OAuth default provider `{default_provider}` must name a configured provider"
            ));
        }
    }
    Ok(providers.join(", "))
}
```

Use the helper from the `MountedOAuth` match arm and turn validation failures
into a failed doctor check:

```rust
        Ok(AuthPolicyKind::MountedOAuth) => match configured_oauth_providers_label(config) {
            Ok(providers) => {
                DoctorCheck::pass("auth", "Auth mode", format!("OAuth ({providers})"))
            }
            Err(message) => DoctorCheck::fail("auth", "Auth mode", message),
        },
```

- [ ] **Step 2: Add a regression test**

Add the regression to the existing sibling `crates/soma/cli/src/doctor/checks_tests.rs` module:

```rust
    #[test]
    fn auth_doctor_check_labels_multiple_configured_providers() {
        let mut config = Config::default();
        config.mcp.host = "0.0.0.0".into();
        config.mcp.auth.mode = AuthMode::OAuth;
        config.mcp.auth.public_url = Some("https://example.com".into());
        config.mcp.auth.google_client_id = Some("g-id".into());
        config.mcp.auth.google_client_secret = Some("g-secret".into());
        config.mcp.auth.github_client_id = Some("gh-id".into());
        config.mcp.auth.github_client_secret = Some("gh-secret".into());
        let check = check_auth_config(&config);
        let value = check.value.expect("passing check has a value");
        assert!(value.contains("Google"));
        assert!(value.contains("GitHub"));
        assert!(!value.contains("Authelia"));
    }

    #[test]
    fn auth_doctor_check_rejects_partial_provider_configuration() {
        let mut config = Config::default();
        config.mcp.host = "0.0.0.0".into();
        config.mcp.auth.mode = AuthMode::OAuth;
        config.mcp.auth.google_client_id = Some("g-id".into());
        let check = check_auth_config(&config);
        assert!(!check.ok);
        assert!(check.hint.expect("failed check has a hint").contains("client secret"));
    }

    #[test]
    fn auth_doctor_check_rejects_unconfigured_default_provider() {
        let mut config = Config::default();
        config.mcp.host = "0.0.0.0".into();
        config.mcp.auth.mode = AuthMode::OAuth;
        config.mcp.auth.google_client_id = Some("g-id".into());
        config.mcp.auth.google_client_secret = Some("g-secret".into());
        config.mcp.auth.default_provider = Some("github".into());
        let check = check_auth_config(&config);
        assert!(!check.ok);
        assert!(check.hint.expect("failed check has a hint").contains("default provider"));
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p soma-cli doctor`
Expected: PASS, including the new test.

Run: `cargo clippy --workspace -- -D warnings`

- [ ] **Step 4: Commit**

```bash
git add crates/soma/cli/src/doctor/checks.rs crates/soma/cli/src/doctor/checks_tests.rs
git commit -m "feat(soma): report which OAuth provider(s) are configured in soma doctor"
```

---

### Task 5: Verification + docs

**Files:**
- Modify: `CLAUDE.md` (Environment variables table)
- Modify: `CHANGELOG.md`
- Regenerate: `.env.example`, `config.soma.toml`, `docs/ENV.md`, `docs/generated/plugin-settings.md`, and plugin metadata selected by `scripts/generate-docs.py`

**Interfaces:**
- None (docs + verification only).

- [ ] **Step 1: Add the 6 new env vars to `CLAUDE.md`'s Environment variables table**

In the root `CLAUDE.md`'s `## Environment variables` table, right after the `SOMA_MCP_AUTH_MODE` row, add:

```markdown
| `SOMA_MCP_AUTHELIA_ISSUER_URL` | — | Authelia OIDC issuer base URL (e.g. `https://auth.example.com`) |
| `SOMA_MCP_AUTHELIA_CLIENT_ID` | — | Authelia OIDC client ID |
| `SOMA_MCP_AUTHELIA_CLIENT_SECRET` | — | Authelia OIDC client secret |
| `SOMA_MCP_GITHUB_CLIENT_ID` | — | GitHub OAuth App client ID |
| `SOMA_MCP_GITHUB_CLIENT_SECRET` | — | GitHub OAuth App client secret |
| `SOMA_MCP_AUTH_DEFAULT_PROVIDER` | first configured (`google` > `authelia` > `github`) | Which provider `/authorize` and `/auth/login` use when the request omits `?provider=` |
```

- [ ] **Step 2: Update the CLI ↔ MCP action parity table's customization note if needed**

Verify whether the parity table needs a note about `?provider=` on `/authorize`/`/auth/login` — these are HTTP OAuth endpoints, not MCP actions or CLI subcommands, so they fall outside the parity table's scope (same reasoning as `/register`, `/jwks`, `/.well-known/*` already being excluded). No edit needed here; confirm by re-reading the "CLI ↔ MCP action parity" section's framing before skipping this step.

- [ ] **Step 3: Regenerate derived docs and plugin metadata**

Run: `cargo xtask generate-docs`

Review every generated diff. The generator reads `crates/soma/config/src/config.rs` and `crates/soma/config/src/env_registry.rs`; do not hand-edit generated files.

- [ ] **Step 4: Add the CHANGELOG entry**

Add under the SAME `## [Unreleased]` → `### Added` section used by Plan 1's Task 13 Step 5 (append right after that entry, don't create a duplicate `## [Unreleased]` heading):

```markdown
- `soma setup`/`soma doctor` and the canonical env-var registry
  (`crates/soma/config/src/env_registry.rs`) now recognize
  `SOMA_MCP_AUTHELIA_ISSUER_URL`/`_CLIENT_ID`/`_CLIENT_SECRET`,
  `SOMA_MCP_GITHUB_CLIENT_ID`/`_CLIENT_SECRET`, and
  `SOMA_MCP_AUTH_DEFAULT_PROVIDER` — OAuth mode now accepts any one of
  Google, Authelia, or GitHub (or several at once) instead of requiring
  Google specifically.
```

- [ ] **Step 5: Full workspace verification**

Run: `cargo test --workspace`
Expected: PASS.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings.

Run: `cargo fmt --check`
Expected: no diff.

Run: `cargo xtask generate-docs --check`
Expected: generated docs are current.

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md CHANGELOG.md .env.example config.soma.toml docs/ENV.md docs/generated/plugin-settings.md plugins/soma
git commit -m "docs: document Authelia/GitHub OAuth env vars"
```
