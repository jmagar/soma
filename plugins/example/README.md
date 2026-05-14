# example plugin

Multi-platform plugin package that connects Claude Code, Codex, and Gemini CLI to the Example MCP server.

## Structure

```
plugins/example/
├── .claude-plugin/
│   └── plugin.json         # Claude Code manifest
├── .codex-plugin/
│   ├── plugin.json         # Codex manifest
│   └── README.md           # Codex manifest field reference
├── gemini-extension.json   # Gemini CLI extension manifest
├── .mcp.json               # Shared MCP server connection config (all three platforms)
├── bin/
│   └── example             # Release binary (populate with: just install)
├── hooks/
│   ├── hooks.json          # SessionStart + ConfigChange hook definitions
│   └── plugin-setup.sh     # Deployment and validation script
├── monitors/
│   └── monitors.json       # Background health monitor (requires Claude Code v2.1.105+)
└── skills/
    └── example/
        └── SKILL.md        # Tool documentation (shared by Claude and Codex)
```

## Platform manifests

All three manifests point at the same `.mcp.json` and `skills/` directory. Only the manifest format and UI metadata differ.

| File | Platform | Variable syntax |
|---|---|---|
| `.claude-plugin/plugin.json` | Claude Code | `${user_config.*}` |
| `.codex-plugin/plugin.json` | Codex | `${user_config.*}` |
| `gemini-extension.json` | Gemini CLI | `${settings.*}` |

**No `version` field in any manifest.** The marketplace assigns version from the git commit SHA. Adding an explicit version creates duplicate entries on every push.

## MCP connection

`.mcp.json` is shared across all platforms:

```json
{
  "mcpServers": {
    "example": {
      "type": "http",
      "url": "${user_config.server_url}/mcp",
      "headers": { "Authorization": "Bearer ${user_config.api_token}" }
    }
  }
}
```

The `${user_config.*}` / `${settings.*}` variables are populated from each platform's user-configurable settings at runtime.

## Hooks

`hooks/hooks.json` fires `plugin-setup.sh` on `SessionStart` and `ConfigChange`.

The setup script runs in two modes:
- **Server mode** (localhost URL) — writes `.env`, manages systemd or Docker deployment, symlinks the binary to `~/.local/bin/`
- **Client mode** (remote URL) — validates `/health` reachability

The script is idempotent; it only restarts the service when `.env` content actually changes.

## Monitors

**Requires Claude Code v2.1.105+.**

`monitors/monitors.json` declares a background `server-health` monitor that starts automatically at session start. It runs `example watch` (the binary in `bin/`) and delivers each stdout line to Claude as a notification whenever the MCP server changes state.

The monitor emits only on state transitions — Claude is not notified while the server is stable. Three states:

- `UP` — `/health` returned 2xx
- `DOWN` — connection refused / timeout
- `DEGRADED(HTTP N)` — non-2xx HTTP response

The command references `${CLAUDE_PLUGIN_ROOT}/bin/example` — populate `bin/` before installing the plugin:

```bash
just install   # builds release binary and copies to plugins/example/bin/example
```

Disabling the plugin mid-session does not stop an already-running monitor; it stops when the session ends.

## Skills

`skills/example/SKILL.md` is the three-tier structured documentation for the `example` MCP tool. The AI reads Tier 1 for quick lookups, Tier 2 for parameter details, Tier 3 for multi-step workflows.

## TEMPLATE checklist

1. Replace every `example` / `Example` / `EXAMPLE_` identifier with your service name
2. Update `userConfig` / `settings` in all three manifests to match your service's credentials
3. Update `skills/example/SKILL.md` — action table, parameters, response shapes, workflows
4. Set `brandColor` and `defaultPrompt` in `.codex-plugin/plugin.json`
5. Update `hooks/plugin-setup.sh` env var block to match your service's `EXAMPLE_*` vars
6. Run `cargo xtask symlink-docs` after adding any new `CLAUDE.md`
