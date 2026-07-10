---
title: "Drop-in provider layout contract"
doc_type: "contract"
status: "draft"
owner: "rmcp-template"
audience:
  - "contributors"
  - "agents"
scope: "template"
source_of_truth: true
upstream_refs:
  - "docs/specs/drop-in-provider-layout.md"
  - "docs/contracts/provider-manifest.schema.json"
  - "crates/rtemplate-service/src/providers/filesystem.rs"
  - "crates/rtemplate-mcp/src/rmcp_server.rs"
last_reviewed: "2026-07-10"
---

# Drop-In Provider Layout Contract

This contract pins the expected behavior for the structured provider directory:

```text
providers/
  tools/
  prompts/
  resources/
```

The spec explains the design intent. This document is the implementation
checklist.

## Provider Root

The provider root is selected by `RTEMPLATE_PROVIDER_DIR`, an explicit
`--dir <path>` argument, or the default `./providers`.

Implementations MUST treat the selected provider root as the trust boundary for
path resolution. No provider file, prompt file, resource file, symlink, decoded
URI, or dynamic parameter may resolve outside that root.

## Directory Ownership

| Directory | Required behavior |
|---|---|
| `tools/` | Load `.json`, `.ts`, and `.wasm` provider tool files. |
| `prompts/` | Load `.md` prompt files. |
| `resources/` | Load static resource files and dynamic resource reader files. |

Root-level file loading MAY remain supported for compatibility. New examples and
docs MUST prefer the structured layout.

## Tool Files

The `tools/` directory uses the existing provider manifest contract:

- `.json` files MUST contain provider manifest JSON.
- `.ts` files MUST expose provider metadata and runtime tool handlers according
  to the AI SDK provider rules.
- `.wasm` files MUST contain a `rtemplate.provider` custom section.

Tool files MAY expose MCP tools, CLI commands, REST routes, Palette commands, and
OpenAPI entries according to their provider metadata.

## Prompt Files

Each Markdown file under `prompts/` MUST become one MCP prompt.

The prompt name MUST be derived from the file path:

- lowercase ASCII letters and digits are preserved;
- punctuation, spaces, and path separators become `-`;
- repeated separators collapse;
- leading and trailing separators are removed;
- if the result does not start with a lowercase ASCII letter, prefix `prompt-`.

The first Markdown line matching `# Heading` SHOULD become the prompt
description. If no heading exists, the implementation MUST generate a stable
description from the filename.

The full Markdown file contents MUST be returned as a user prompt message from
`prompts/get`.

`README.md` files SHOULD be ignored by prompt discovery.

## Static Resource Files

Every non-reader file under `resources/` MUST become an MCP resource.

The resource URI MUST be derived from the path relative to `resources/`:

```text
providers/resources/api/schema.json
```

maps to:

```text
rtemplate://resources/api/schema
```

The implementation MUST remove the final file extension from the URI. Directory
segments MUST be normalized with the same separator rules as prompt names, except
that `/` remains a path separator.

Static resource metadata MUST be inferred as follows:

| Field | Rule |
|---|---|
| `uri` | `rtemplate://resources/{normalized_path_without_extension}` |
| `name` | final normalized path segment |
| `description` | first Markdown heading or generated stable description |
| `mime_type` | inferred from file extension or `application/octet-stream` |

Text MIME types MUST return `ResourceContents::text`. Binary MIME types MUST
return `ResourceContents::blob`.

## Dynamic Resource Reader Files

Dynamic reader files under `resources/` MUST become MCP resource templates.

The initial required reader extension is `.ts`. Additional reader runtimes such
as `.wasm` MAY be added later if they obey this contract.

Path parameters use bracket segments:

| File path | URI template |
|---|---|
| `status.ts` | `rtemplate://resources/status` |
| `service/[name].ts` | `rtemplate://resources/service/{name}` |
| `repo/file/[...path].ts` | `rtemplate://resources/repo/file/{path}` |

Parameter names MUST match:

```text
^[A-Za-z_][A-Za-z0-9_]*$
```

At most one catch-all segment MAY appear in a resource template, and it MUST be
the final path segment.

TypeScript dynamic resource files MUST export:

```ts
export async function read(input) { ... }
```

The input object MUST include:

```ts
{
  uri: string,
  params: Record<string, string>,
  query?: Record<string, string>
}
```

The return value MUST be one of:

```ts
{ text: string, mimeType?: string }
{ json: unknown }
{ blob: string, mimeType: string }
```

`json` results MUST be serialized as `application/json`. `blob` MUST be base64.

Dynamic readers MAY export optional metadata:

```ts
export const meta = {
  name: "service-status",
  description: "Live service status",
  mimeType: "application/json"
};
```

Metadata is optional. Path-derived defaults MUST be sufficient.

## URI Matching

When resolving `resources/read`, implementations MUST match resources in this
order:

1. exact static resource URI;
2. exact dynamic resource URI;
3. parameterized dynamic resource URI;
4. catch-all dynamic resource URI.

Ambiguous templates at the same precedence level MUST make validation fail.

The resolved resource MUST be read from the same immutable provider snapshot
that supplied the matching resource metadata.

## Validation Contract

`rtemplate providers validate` MUST validate the structured layout without
executing dynamic readers.

Validation MUST reject:

- duplicate resource URIs;
- ambiguous URI templates;
- invalid parameter names;
- catch-all segments that are not final;
- symlink or path traversal escapes;
- static files larger than the configured resource size limit;
- dynamic reader files missing the required reader export;
- binary static files whose MIME type cannot be safely represented.

Validation MUST report discovered files by primitive type: tools, prompts, and
resources.

## MCP Surface Contract

`resources/list` MUST include:

- built-in resources;
- static resources from the active provider snapshot;
- dynamic resource templates from the active provider snapshot.

`resources/read` MUST:

- refresh file providers before matching;
- match the request URI against the active snapshot;
- return text/blob contents with the declared MIME type;
- return an MCP invalid-params error for unknown resources;
- preserve the previous valid snapshot if a refresh fails.

`prompts/list`, `prompts/get`, and `tools/list` MUST continue to refresh the same
provider snapshot, so a single dropped file can affect multiple primitive
surfaces consistently.

## Verification Requirements

Changes to this contract require focused tests for:

- static resource path-to-URI mapping;
- dynamic parameter and catch-all URI matching;
- duplicate and ambiguous resource rejection;
- symlink/path traversal rejection;
- `resources/list` including provider resources;
- `resources/read` returning static resource contents;
- dynamic `.ts` resource read dispatch;
- validation not executing dynamic readers.

At minimum, run:

```bash
cargo test -p rtemplate-service providers::filesystem_tests
cargo test -p rtemplate-mcp resources
cargo test -p rmcp-template --test provider_cli
cargo xtask check-provider-manifest-contract
```
