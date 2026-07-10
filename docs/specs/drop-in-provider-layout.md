# Drop-In Provider Layout

## Status

Draft specification for the frictionless provider directory layout.

## Goal

Make provider authoring feel like adding files to a well-known folder, not like
assembling a platform manifest by hand. The runtime should infer the common
case from path and extension:

```text
providers/
  tools/
  prompts/
  resources/
```

The directory name selects the MCP primitive:

| Directory | Primitive | Happy path |
|---|---|---|
| `providers/tools/` | MCP tools plus optional CLI/REST overlays | Drop `.json`, `.ts`, or `.wasm` files |
| `providers/prompts/` | MCP prompts | Drop `.md` files |
| `providers/resources/` | MCP resources and resource templates | Drop static files or dynamic reader files |

The core rule is that a file path is useful by itself. Metadata may refine the
surface, but it should not be required for the happy path.

## Non-Goals

- Do not require JSON manifests for static prompts or static resources.
- Do not expose prompts or resources through REST or CLI by default.
- Do not execute dynamic resource handlers during `providers validate`.
- Do not let resource files escape the configured provider directory through
  symlinks, `..`, or URI decoding tricks.
- Do not make root-level provider files ambiguous once the structured layout is
  available.

## Directory Semantics

### Tools

`providers/tools/` owns action-like provider files:

```text
providers/tools/weather.ts
providers/tools/github.openapi.json
providers/tools/analyzer.wasm
```

These files load into the existing provider tool registry. They may expose MCP
tools, CLI commands, REST routes, OpenAPI, Palette commands, and generated docs
according to their provider metadata.

The existing root-level provider loading can remain as a compatibility path, but
new docs and examples should prefer `providers/tools/`.

### Prompts

`providers/prompts/` owns prompt templates:

```text
providers/prompts/code-review.md
providers/prompts/release-notes.md
```

Each Markdown file becomes one MCP prompt. The filename defines the prompt name,
the first `# Heading` provides the description when present, and the full
Markdown file becomes the returned prompt message.

Prompt files are not tools. They are not callable actions and do not get CLI or
REST exposure.

### Resources

`providers/resources/` owns URI-addressable context:

```text
providers/resources/runbook.md
providers/resources/api/schema.json
providers/resources/service/[name].ts
providers/resources/repo/file/[...path].ts
```

Static files become resources directly. Dynamic files become resource readers.
The path under `providers/resources/` defines the URI.

## Resource URI Mapping

Static resource:

```text
providers/resources/runbook.md
```

maps to:

```text
rtemplate://resources/runbook
```

Nested static resource:

```text
providers/resources/api/schema.json
```

maps to:

```text
rtemplate://resources/api/schema
```

Dynamic resource:

```text
providers/resources/status.ts
```

maps to:

```text
rtemplate://resources/status
```

Parameterized dynamic resource:

```text
providers/resources/service/[name].ts
```

maps to:

```text
rtemplate://resources/service/{name}
```

Catch-all dynamic resource:

```text
providers/resources/repo/file/[...path].ts
```

maps to:

```text
rtemplate://resources/repo/file/{path}
```

URI matching should prefer exact static resources, then exact dynamic resources,
then parameterized resources, then catch-all resources. Ambiguous templates are
invalid.

## Static Resources

Static resources are regular files. The runtime infers:

- `name` from the path stem;
- `uri` from the path relative to `providers/resources/`;
- `mime_type` from extension;
- `description` from the first Markdown heading for Markdown files, or a simple
  generated description for other file types.

Text MIME types return `ResourceContents::text`. Binary MIME types return
`ResourceContents::blob`.

Examples:

| File | URI | MIME |
|---|---|---|
| `runbook.md` | `rtemplate://resources/runbook` | `text/markdown` |
| `notes.txt` | `rtemplate://resources/notes` | `text/plain` |
| `api/schema.json` | `rtemplate://resources/api/schema` | `application/json` |
| `images/logo.png` | `rtemplate://resources/images/logo` | `image/png` |

## Dynamic Resources

Dynamic resources are files whose extension declares a reader runtime. The
minimal TypeScript dynamic resource should be:

```ts
export async function read() {
  return { text: "current status" };
}
```

Parameterized resources receive params extracted from the URI:

```ts
export async function read({ params }) {
  return { text: `status for ${params.name}` };
}
```

Dynamic readers may return one of:

```ts
return { text: "plain text", mimeType: "text/plain" };
return { json: { ok: true } };
return { blob: base64Bytes, mimeType: "image/png" };
```

The happy path should not require an exported manifest. Optional metadata may
override inferred fields:

```ts
export const meta = {
  name: "service-status",
  description: "Live service status",
  mimeType: "application/json",
};
```

WASM resources should follow the same path-to-URI convention. A resource WASM
ABI can be added after the TypeScript reader contract is proven.

## Validation

`rtemplate providers validate` should:

- report tools, prompts, and resources separately;
- validate path-derived names and URIs;
- reject duplicate resource URIs and ambiguous URI templates;
- reject malformed dynamic parameter names;
- reject symlink escapes and paths outside the provider root;
- enforce file size limits for static resources;
- validate dynamic reader shape without executing the reader.

Validation must not run TypeScript, instantiate WASM, call MCP upstreams, fetch
OpenAPI URLs, or read outside the provider root.

## MCP Runtime Behavior

The MCP server refreshes provider files on:

- `tools/list`
- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- schema resource reads

`resources/list` returns static resources and dynamic resource templates.
`resources/read` resolves the URI against the active immutable provider snapshot
and returns the matching resource contents.

If a resource disappears or becomes invalid, a reload must leave the last valid
snapshot active until a valid replacement snapshot is available.

## Migration Path

1. Keep root-level `.json`, `.ts`, `.wasm`, and `.md` support for compatibility.
2. Add structured directory discovery for `tools/`, `prompts/`, and `resources/`.
3. Move examples to the structured layout.
4. Teach docs to prefer the structured layout.
5. Optionally warn when root-level files are used after the structured layout is
   stable.
