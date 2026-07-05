---
name: static-rust
description: Generated skill for the `static-rust` provider. Use when working with this provider's generated MCP, CLI, or REST surfaces.
---

# `static-rust` Provider

Native service actions compiled into the template.

## Tools

| tool | MCP | CLI | REST | CLI command | REST route | purpose |
|---|---:|---:|---:|---|---|---|
| `greet` | yes | yes | yes | `greet` | `POST /v1/greet` | Return a greeting. |
| `echo` | yes | yes | yes | `echo` | `POST /v1/echo` | Echo a message back unchanged. |
| `status` | yes | yes | yes | `status` | `GET /v1/status` | Return server status and configuration info. |
| `elicit_name` | yes | no | no | `elicit_name` | `` | Ask the MCP client to collect a name, then return a personalised greeting. |
| `scaffold_intent` | yes | no | no | `scaffold_intent` | `` | Collect scaffold setup intent through MCP elicitation and return JSON for the scaffold-project skill. |
| `help` | yes | yes | yes | `help` | `GET /v1/help` | Show the action reference. |

## Usage

- Prefer the MCP action when the server is connected.
- Use the CLI command for local scripts and smoke tests.
- Use REST routes for HTTP clients when the tool explicitly enables REST.
