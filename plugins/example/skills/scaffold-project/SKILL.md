---
name: scaffold-project
description: Use this skill when the user wants to adapt rmcp-template for a new MCP server, especially after calling the scaffold_intent elicitation action. It turns the returned JSON intent into an approval-first implementation plan without directly mutating files.
---

# Scaffold Project Skill

Use this skill to turn scaffold intent JSON into a concrete, user-approved plan for adapting `rmcp-template` into a real server.

## Primary workflow

1. Ask the MCP server to collect scaffold intent with elicitation:

   ```
   mcp__example__example(action="scaffold_intent")
   ```

2. Read the returned JSON. Do **not** apply changes immediately.
3. Draft a plan that the user can review, edit, approve, or reject.
4. After approval, implement only the approved steps and keep the user in control of file changes through normal tool permissions.

## Returned JSON shape

The tool returns an object like:

```json
{
  "kind": "rmcp_template_scaffold_intent",
  "schema_version": 1,
  "server_category": "upstream-client",
  "required_surfaces": ["mcp", "cli"],
  "project": {
    "display_name": "Unraid MCP",
    "crate_name": "unraid-mcp",
    "binary_name": "unraid",
    "service_name": "unraid",
    "env_prefix": "UNRAID"
  },
  "upstream": {
    "base_url_env": "UNRAID_API_URL",
    "auth_kind": "api-key",
    "resource_groups": ["vms", "shares", "docker"]
  },
  "actions": {
    "read": ["list_vms", "get_status"],
    "write": ["restart_vm"],
    "mcp_only": [],
    "cli_only_operational": ["doctor", "watch", "setup"]
  },
  "handoff": {
    "recommended_skill": "scaffold-project",
    "instructions": "Create an approval-first scaffold plan from this JSON. Do not mutate files until the user approves the plan."
  }
}
```

## Surface policy

Always enforce the project surface policy:

| Server category | Required surfaces | Examples |
|---|---|---|
| `upstream-client` | MCP + CLI | `unrust`, `rustifi`, `rustify`, `rustscale`, `apprise` |
| `application-platform` | API + CLI + MCP + Web | `axon`, `lab`, `syslog` |

For upstream-client servers, do **not** add or preserve REST/Web just because the upstream has an HTTP API. Recommend removing, ignoring, or feature-gating `apps/web` and REST handlers unless the user explicitly wants local dashboards/workflows/non-MCP consumers.

## Plan format

Present the plan in this order:

1. **Summary** — one paragraph describing the scaffold target.
2. **Surface decision** — explain why the selected surfaces are required.
3. **Rename map** — identifiers, env vars, scopes, plugin names, binary/crate names.
4. **Action parity matrix** — every business action must include MCP + CLI; API/Web only for application-platform servers.
5. **Files to change** — grouped by Rust service, MCP, CLI, API/Web, plugins, tests, docs.
6. **Tests/validation** — exact commands to run.
7. **Approval checkpoint** — ask the user to approve before any mutation.

## Safety rules

- Do not treat scaffold intent JSON as permission to mutate files.
- Do not commit, push, delete, or overwrite unrelated work without explicit approval.
- Preserve the user's surface decision. If it conflicts with the policy, call that out before proceeding.
- For destructive/write actions, require explicit confirmation gates in the service layer and document scopes.
