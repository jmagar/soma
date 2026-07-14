# Codex Plugin — .codex-plugin/plugin.json

<!-- CUSTOMIZE: This README explains the Codex plugin manifest. Keep it in the repo
     so future maintainers understand what each field does. Update the CUSTOMIZE:
     comments in plugin.json before publishing. -->

## What this is

`plugin.json` is the Codex plugin manifest — the Codex equivalent of the Claude Code
`.claude-plugin/plugin.json`. Both files live next to each other in
`plugins/<service>/`; MCP registration is supplied by the client or gateway and
should run `soma mcp` for stdio mode.

## File structure

```
plugins/soma/
  .claude-plugin/
    plugin.json     ← Claude Code plugin manifest
  .codex-plugin/
    plugin.json     ← Codex plugin manifest (this file's sibling)
    README.md       ← You are here
  hooks/            ← Claude Code hooks (Claude-specific, not used by Codex)
  skills/           ← Shared skills (both Claude Code and Codex can load these)
```

## Field reference

| Field | Description |
|---|---|
| `name` | CUSTOMIZE: Unique plugin identifier. Convention: `<service>-mcp`. |
| `description` | CUSTOMIZE: One-line description for registries and `--help` output. |
| `homepage` | CUSTOMIZE: Your project's GitHub URL. |
| `repository` | CUSTOMIZE: Same as homepage for GitHub-hosted projects. |
| `license` | Keep `"MIT"` unless you chose a different license. |
| `keywords` | CUSTOMIZE: 3–6 tags for registry search. |
| `skills` | Path to shared skills directory. Do not change — convention is `"./skills/"`. |
| `mcpServers` | Omitted for this package; configure stdio MCP externally as `soma mcp`. |
| `interface.displayName` | CUSTOMIZE: Human-readable name shown in Codex UI. |
| `interface.shortDescription` | CUSTOMIZE: 50-char tagline shown in plugin listings. |
| `interface.longDescription` | CUSTOMIZE: Full description for the detail page. |
| `interface.developerName` | CUSTOMIZE: Your name or org name. |
| `interface.category` | One of: `"Infrastructure"`, `"Productivity"`, `"Developer Tools"`, `"Data"`. |
| `interface.capabilities` | CUSTOMIZE: `["Read"]` for read-only, `["Read", "Write"]` for write ops. |
| `interface.websiteURL` | CUSTOMIZE: Your project URL. |
| `interface.defaultPrompt` | CUSTOMIZE: 3 sample prompts showing the most useful actions. |
| `interface.brandColor` | CUSTOMIZE: Hex color for the plugin icon background. `#6366F1` is Indigo-500. |
| `interface.composerIcon` | Path to a square PNG (512×512) for the composer icon. |
| `interface.logo` | Path to an SVG logo for the plugin detail page. |
| `author.name` | CUSTOMIZE: Your full name. |
| `author.email` | CUSTOMIZE: Your GitHub noreply email or public email. |
| `author.url` | CUSTOMIZE: Your GitHub profile URL. |

## Capabilities: Read vs Write

Set `capabilities` based on what your MCP server actually does:

- `["Read"]` — server only fetches/queries data (no mutations, no destructive actions)
- `["Read", "Write"]` — server can modify state (create, update, delete operations)

Soma includes both `"Read"` and `"Write"` to show the pattern. If your server
is read-only, remove `"Write"`.

## Keeping plugin.json in sync

These fields must stay in sync across files:

| Field | plugin.json | Cargo.toml | server.json |
|---|---|---|---|
| `homepage` / `repository` | both fields | `homepage` | `repository.url` |

Plugin manifests intentionally stay versionless. The marketplace derives plugin
version from the git commit SHA; release version parity is checked through the
Rust crate, release metadata, generated docs, and MCP registry files.

## brandColor choices

| Color | Hex | Use case |
|---|---|---|
| Indigo-500 (default) | `#6366F1` | Generic/Soma |
| Amber-400 | `#F59E0B` | Unraid (warm hardware theme) |
| Emerald-500 | `#10B981` | Gotify (notifications/green) |
| Sky-500 | `#0EA5E9` | UniFi (networking/blue) |
| Violet-500 | `#8B5CF6` | Tailscale (purple brand) |

<!-- CUSTOMIZE: Pick a color that fits your service's brand or the color scheme
     of the upstream service's own UI. -->
