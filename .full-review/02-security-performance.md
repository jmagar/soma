# Security And Performance Review

Scope: PR #138 REST bridge and app-server client runtime behavior.

## Findings

- P2: `rest::router()` exposed unauthenticated prompt execution by default.
- P2: trusted bridge mode automatically admitted request-controlled command/config/sandbox overrides.
- P2: `max_sessions` checked session count before spawning, allowing concurrent process exhaustion.
- P2: compatibility checks ran blocking `codex --version` work on Tokio workers.
- P2: stateful raw calls had no in-flight concurrency limit.
- P2: terminal turn events could be dropped under event-channel backpressure.
- P2: idle TTL cleanup only ran on future traffic.
- P3: REST sessions had no principal binding because the adapter has no built-in auth layer.

## Fixes

- Made the default REST router non-executing: health and compatibility only.
- Added `rest::text_turn_router()`/`RestRouterOptions::text_turn()` for explicit one-shot prompt execution.
- Kept `trusted_bridge_router()` authless but safer: raw bridge/session routes are enabled, unsafe client options remain disabled unless `.with_unsafe_client_options(true)` is explicit.
- Added session-slot semaphore permits acquired before spawning Codex and stored with sessions.
- Moved compatibility cache refresh through `tokio::task::spawn_blocking`.
- Added global and per-session semaphores for stateful calls.
- Made `turn/completed` a must-deliver notification under temporary channel backpressure.
- Added session activity leases and prune-on-access behavior; opaque ids and docs make the auth/tenancy boundary explicit for host apps.

## Not Fixed By Scope

- P4 raw upstream error hygiene was observed but outside the user-requested P0-P3 fix set.
