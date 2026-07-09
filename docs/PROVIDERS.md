# Drop-In Providers

`rtemplate` loads provider files from `./providers` by default. Override the
directory with `RTEMPLATE_PROVIDER_DIR` at runtime or with
`rtemplate providers ... --dir <path>` for local checks.

## Supported Files

| Extension | Provider kind | What is loaded |
|---|---|---|
| `.json` | `static-rust`, `mcp`, `openapi` | Provider manifest JSON |
| `.ts` | `ai-sdk` | `export default { ... }` provider catalog metadata |
| `.wasm` | `wasm` | `rtemplate.provider` custom section |

Disabled manifests with `"enabled": false` under `provider` are visible in
validation output and are not registered at runtime.

## Manifest Shape

Every provider declares:

- `schema_version`: numeric manifest contract version, currently `1`.
- `provider.name`: stable provider identifier shown in inspection output.
- `provider.kind`: one of `static-rust`, `ai-sdk`, `wasm`, `mcp`, or `openapi`.
- `tools[].name`: action name exposed through CLI, MCP, and HTTP.
- `tools[].input_schema`: JSON Schema object for action input.
- `tools[].output_schema`: optional JSON Schema for action output.

Set `provider.enabled` to `false` when you want a manifest checked and documented
without loading it at runtime.

## Check A Provider Directory

```bash
rtemplate providers status
rtemplate providers list --json
rtemplate providers validate
rtemplate providers validate --dir ./examples/providers
```

`validate` exits non-zero when any provider file is invalid.

## Runtime Loading

CLI commands refresh providers on startup:

```bash
rtemplate my_provider_action --json '{"message":"hello"}'
```

MCP servers refresh file providers when clients list tools or read the tools
resource, so a newly dropped provider appears without rebuilding the binary.

HTTP dispatch uses the same registry:

```bash
curl -sS -X POST http://127.0.0.1:8080/v1/providers/my_provider_action \
  -H 'content-type: application/json' \
  -d '{"message":"hello"}'
```

## MCP Providers

`mcp` providers infer their transport from `provider.meta.mcp`: `url` selects
Streamable HTTP and `stdio.command` selects stdio. Use `timeout_ms` to bound
upstream calls, and pin upstream tool mapping in each tool's `meta.mcp` block.

## OpenAPI Providers

`openapi` providers pin a base URL in `meta.openapi.base_url`; each tool supplies
a relative operation path in `tools[].meta.openapi.path` or `tools[].rest.path`.
Operation paths must stay relative to the pinned base URL. Declare allowed
network hosts in `capabilities.network.allowed_hosts` when network capability is
enabled.

## Safety Model

`providers list`, `providers status`, and `providers validate` inspect provider
catalogs only. They do not execute TypeScript handlers, instantiate WASM
handlers, call MCP upstreams, or fetch OpenAPI URLs.

## Examples

See `examples/providers/`.
