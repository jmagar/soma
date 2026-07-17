# unifi

Standalone client for UniFi Network Controllers: authentication, both the
official and internal REST APIs, capability discovery, and dynamic action
dispatch.

This crate has no dependency on `rmcp`, `axum`, or anything soma-specific —
it only knows how to talk to a UniFi controller. See
`crates/integrations/README.md` (one level up) for what makes this a
`crates/integrations/*` crate and the checklist to follow when extracting
the next one.

## Quick start

```rust,no_run
use unifi::{UnifiClient, UnifiConfig};

# async fn run() -> Result<(), unifi::UnifiError> {
let client = UnifiClient::new(&UnifiConfig {
    url: "https://unifi.local".to_string(),
    api_key: std::env::var("UNIFI_API_KEY").unwrap_or_default(),
    ..UnifiConfig::default()
})?;

let clients = client.clients().await?;
println!("{clients}");
# Ok(())
# }
```

Most embedders should build on [`UnifiService`] rather than [`UnifiClient`]
directly — it's the stable seam for adding cross-cutting behavior (result
shaping, caching, metrics) without touching every call site.

## Layout

| Module | Owns |
|---|---|
| `client` / `UnifiClient` | The pooled HTTP client and its named, fixed endpoints (`clients`, `devices`, `wlans`, ...) |
| `service` / `UnifiService` | The facade embedders should depend on |
| `actions` | Dynamic action dispatch (`ActionDispatcher`) driven by the capability catalog |
| `api` | Path/URL construction for the official and internal APIs |
| `capabilities` | The action catalog, built from the JSON inventories in `data/` |
| `config` / `UnifiConfig` | Connection configuration |
| `http` | The one place HTTP requests are made and errors mapped |
| `error` / `UnifiError` | Every typed failure this crate can return |

## Error handling

Every fallible function returns [`UnifiError`] (aliased as `Result`), never
`anyhow::Error` or a boxed `dyn Error`. Match on the variant when a caller
needs to react differently — e.g. prompt for a new API key on
`UnifiError::Unauthorized` versus retry on `UnifiError::Timeout`.

## Testing

- Pure logic (path substitution, request normalization, name mapping, hybrid
  routing) has inline `#[cfg(test)] mod tests` next to the code it tests.
- `tests/client.rs` exercises the HTTP layer end-to-end against a
  [`wiremock`](https://docs.rs/wiremock) mock server — no real controller
  needed. Copy that file's pattern for testing another integration crate's
  HTTP client.

```bash
cargo test -p unifi
cargo clippy -p unifi --all-targets -- -D warnings
cargo doc -p unifi --open
```

## Status

Not yet published — `publish = false` in `Cargo.toml`. The package is named
`unifi` for now (matching the crate it was extracted from); pick a
brand-neutral, crates.io-available name before publishing.
