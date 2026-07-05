# Service Surface Suggestions

## Goal

Keep MCP, CLI, REST, and web surfaces thin while letting new business actions
live in one shared service path.

## Native Rust Action Add Workflow

To add a native Rust action:

1. Add the business method to `crates/rtemplate-service/src/app.rs` or a focused service module.
2. Add one action metadata entry and one executor match arm in `crates/rtemplate-service/src/actions.rs`.
3. Run `cargo test -p rtemplate-service -p rtemplate-cli -p rmcp-template --tests`.
4. Run `cargo xtask check-openapi --write`.

No edits should be required in `crates/rtemplate-api`, `crates/rtemplate-cli`,
or `crates/rtemplate-mcp`.

## Surface Shape

Business logic should live in `crates/rtemplate-service`.

The stable flow should be:

```text
MCP / CLI / REST / Web
  -> surface adapter
  -> service action registry
  -> rtemplate_service::dispatch_action(...)
  -> ExampleService method
  -> domain modules or standalone crates
```

Keep MCP as one action-dispatched tool so clients see one tool with an `action`
argument instead of one MCP tool per operation. REST should use direct typed
routes such as `POST /v1/echo`, `GET /v1/status`, and `GET /v1/help`; do not
reintroduce a REST action envelope like `POST /v1/example`.

Future runtime providers such as Wasm or TypeScript should stay deferred until
there is a second concrete provider. The current template intentionally uses a
native Rust registry only.
