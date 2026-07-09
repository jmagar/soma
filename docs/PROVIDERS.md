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
| `.md` | `static-rust` prompt | Markdown prompt exposed through MCP prompts |

Disabled manifests with `"enabled": false` under `provider` are visible in
validation output and are not registered at runtime.

## Manifest Shape

Every provider declares:

- `schema_version`: numeric manifest contract version, currently `1`.
- `provider.name`: stable provider identifier shown in inspection output.
- `provider.kind`: one of `static-rust`, `ai-sdk`, `wasm`, `mcp`, or `openapi`.
- `tools[].name`: action name registered in the provider registry.
- `tools[].cli`: optional CLI overlay; set `enabled` to expose a dynamic CLI command.
- `tools[].rest`: optional HTTP overlay; set `enabled` to expose an HTTP route.
- `tools[].input_schema`: JSON Schema object for action input.
- `tools[].output_schema`: optional JSON Schema for action output.
- `prompts[].template`: prompt body returned by MCP `prompts/get`.

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

CLI commands refresh providers on startup when the tool has an enabled CLI
overlay:

```bash
rtemplate my_provider_action --json '{"message":"hello"}'
```

For destructive provider actions, pass `--yes` or `-y` to confirm in
non-interactive runs. `--yes` is reserved for CLI confirmation; use `--json`
when a provider needs an input property named `yes`.

MCP servers refresh file providers when clients list tools or read the tools
resource, so a newly dropped provider appears without rebuilding the binary.
MCP servers also refresh when clients list or get prompts, so a newly dropped
Markdown prompt appears without rebuilding the binary.

HTTP dispatch uses the same registry:

```bash
curl -sS -X POST http://127.0.0.1:40060/v1/providers/my_provider_action \
  -H 'content-type: application/json' \
  -d '{"message":"hello"}'
```

## MCP Providers

The HTTP port defaults to `40060`; replace it if `RTEMPLATE_MCP_PORT` is set.

`mcp` providers infer their transport from `meta.mcp`: `url` selects
Streamable HTTP and `stdio.command` selects stdio. Use `timeout_ms` to bound
upstream calls, and pin upstream tool mapping in each tool's `meta.mcp` block.

## OpenAPI Providers

`openapi` providers pin a base URL in `meta.openapi.base_url`; each tool supplies
a relative operation path in `tools[].meta.openapi.path` or `tools[].rest.path`.
Operation paths must stay relative to the pinned base URL. Declare allowed
network hosts in `capabilities.network.allowed_hosts` when network capability is
enabled.

## Markdown Prompts

Drop a `.md` file into the provider directory to expose it as an MCP prompt. The
file stem becomes the prompt name after lowercasing and replacing punctuation
with hyphens, so `Code Review.md` becomes `code-review`. The first `# Heading`
becomes the prompt description when present, and the full Markdown file is
returned as the prompt message.

## Safety Model

`providers list`, `providers status`, and `providers validate` inspect provider
catalogs only. They do not execute TypeScript handlers, instantiate WASM
handlers, call MCP upstreams, fetch OpenAPI URLs, or run Markdown prompts.
Validation checks manifest
schema, semantic registry rules, JSON Schema compilation, and non-executing
runtime configuration such as OpenAPI base URLs, MCP transport shape, AI SDK
handler export presence, and WASM module exports.

## Examples

See `examples/providers/`.
