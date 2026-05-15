# Philosophy

`rmcp-template` exists to make new MCP servers safe, boring, and easy for agents to operate.

## Boring by design

- One binary.
- One HTTP port.
- One action-dispatch MCP tool.
- Clear layering between client, service, and transport shims.
- Repeatable scripts and release gates.

New servers from this template should be easy to understand, audit, and extend — not clever.

## Thin shims, rich service layer

MCP, REST, and CLI code should parse inputs and delegate. Validation, transformation, and business decisions belong in `ExampleService`:

```
MCP shim   → parse JSON args     → service.method()  → return Value
CLI shim   → parse argv          → service.method()  → format/print
REST shim  → parse HTTP body     → service.method()  → return JSON
```

Zero business logic in shims. If you're writing validation in `mcp/tools.rs`, move it to `app.rs`.

## Secure defaults

- `.env` is ignored and blocked from commits by `scripts/block-env-commits.sh`.
- Non-loopback HTTP requires auth unless explicitly behind a trusted gateway (`EXAMPLE_NOAUTH=true`).
- Secrets in plugin settings must be marked `sensitive: true`.
- Plugin manifests do not carry version fields — marketplace versioning comes from git SHA/tags.
- Never hard-code tokens in unit files or documentation.

## Agent-first outputs

Agents have finite context windows. All outputs must be:

- **Bounded** — 10K token cap, truncation with a clear message
- **Structured** — stable JSON shapes that don't change between versions
- **Paginated** — every list action supports `limit` and `offset`
- **Self-describing** — `action="help"` always available, no auth required

Error messages must be correctable: state what failed, the bad value, why it failed, and the next command to run.

## Tests prove meaning

A good test proves the returned data is correct. Examples:
- `echo` must return the exact message.
- `greet(name="Alice")` must include the name `Alice` in the response.
- Resource tests must inspect schema content, not just check that `resources/read` returned HTTP 200.

A test that only checks `is_error: false` proves nothing about the service.

## Glass house, not black box

Every server must expose its internal state:
- `/health` — fast liveness, always public
- `/status` — redacted runtime state, always public
- `action="status"` — same data via MCP for clients that can't call HTTP directly
- Structured tracing on every upstream call
- Atomic counters for requests, errors, upstream calls

Operators and agents should never have to guess what the server is doing.

## Surface parity

Every action reachable from MCP must also be reachable from the CLI and REST API. The service layer is called identically from all three surfaces — no logic is duplicated, no behavior diverges. Elicitation-based actions are the one intentional exception: they require a live MCP client interaction.

See `docs/PATTERNS.md` for the full catalog of patterns.
