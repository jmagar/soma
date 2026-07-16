# Testing And Documentation Review

Scope: PR #138 tests, examples, README, and public liftability story.

## Findings

- P2: production REST session lifecycle behavior was under-tested.
- P2: unsafe client-option rejection was only tested on `/v1/text-turn`.
- P3: poll concurrency/error mapping coverage was thin.
- P2: downstream REST docs omitted direct `tokio` and `axum` dependencies.
- P2: trusted bridge docs understated the auth and host-execution risks.
- P3: stateful flow docs omitted `thread/start`.
- P3: quick-start docs omitted first-run/cost warnings and used inconsistent model names.

## Fixes

- Added REST tests for bridge unsafe-option rejection, explicit trusted unsafe opt-in, simultaneous poll conflict/release, session-limit enforcement, and HTTP error mapping.
- Added transport regression coverage for non-droppable `turn/completed`.
- Isolated the live smoke test with temporary `CODEX_HOME`/`HOME` to avoid real `~/.codex` sqlite-state flakes.
- Added an async approval handler regression test.
- Updated README REST quick-start deps, first-run/cost warnings, model examples, trusted bridge warnings, and stateful `thread/start` then `turn/start` flow.
- Updated the REST example to mount `text_turn_router()` because `router()` is now non-executing.
- Updated rustdoc quick-start model to `gpt-5`.
