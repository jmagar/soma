# JSON contracts

Durable contracts for scaffold handoff payloads, generated artifacts, and
profile behavior. Some contracts are machine-readable JSON Schemas; others are
normative Markdown checklists backed by tests and validators.

## Plugin stdio adapter

- Contract: `plugin-stdio-adapter.md`
- Decision record: `../adr/0001-stdio-first-plugin-adapter.md`

The local plugin default is the stdio MCP adapter (`soma mcp`) that can target a
deployed platform API through `SOMA_API_URL`. The `soma serve` runtime keeps REST
API, Web, Streamable HTTP MCP, health, and auth surfaces for Docker, systemd, and
gateway deployments.

Validate with:

```bash
bash scripts/check-plugin-stdio-smoke.sh
bash scripts/validate-plugin-layout.sh
cargo test --test plugin_contract
```

## Scaffold intent

- Schema: `scaffold-intent.schema.json`
- Examples:
  - `examples/scaffold-intent-upstream-client.json`
  - `examples/scaffold-intent-application-platform.json`
- Spec: `../specs/scaffold-intent-handoff.md`

`soma_scaffold_intent` is returned by the MCP-only `scaffold_intent` elicitation action and consumed by the `scaffold-project` skill. The payload is intent only; it is not permission to mutate files.

Validate with:

```bash
just scaffold-contract-check
```
