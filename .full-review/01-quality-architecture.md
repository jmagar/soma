# Quality And Architecture Review

Scope: PR #138, `009a03d373690c1bb58caace09a6092d078538c7..c87ac8905cc0b3e84b7acca6f915a54b8985126a`.

## Findings

- P1: one-shot raw REST calls were advertised as an "every callable" bridge even though stateful Codex workflows need sessions, events, and request replies.
- P1: one-shot text turns could wait indefinitely for completion.
- P2: REST sessions had no tenant ownership model and used predictable session/request ids.
- P2: REST request replies could return success after the underlying app-server reply channel had already expired.
- P2: idle session cleanup was opportunistic and could prune sessions while they were being used.
- P3: `RestBackend` required full bridge implementation even for helper-only host apps.
- P3: stateful call routes accepted `client` options that were ignored.
- P3: `RestEventResponse` allowed invalid response states.
- P3: `ApprovalHandler` was synchronous, making UI/channel-backed approval policies unsafe on async runtimes.

## Fixes

- Documented and tested the distinction between one-shot helper calls and stateful session bridge calls; the "every callable" path is now the session bridge.
- Added text-turn duration and output-size limits, returning `504`/`413` and attempting `turn_interrupt`.
- Switched REST session/request ids to opaque UUID-backed ids, and kept the adapter explicitly authless/trusted-boundary-only.
- Carried absolute reply deadlines through `PendingServerRequest`; REST reply routes now return `410 Gone` if delivery is no longer possible.
- Added session activity leases and session call gates to prevent active idle pruning and bound stateful call concurrency.
- Added default `RestBackend` methods so custom helper-only backends can lift the API with less boilerplate.
- Rejected `client` on stateful session calls with `400 invalid_request`.
- Replaced `RestEventResponse` with a tagged enum.
- Changed `ApprovalHandler` to return `ApprovalFuture`, added `AsyncFnApprovalHandler`, and updated session draining to await approval decisions.
