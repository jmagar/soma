# Philosophy

`rmcp-template` exists to make new MCP servers safe, boring, and easy for agents to operate.

## Boring by design

- One binary.
- One HTTP port.
- One action-dispatch MCP tool.
- Clear layering between client, service, and transport shims.
- Repeatable scripts and release gates.

## Thin shims, rich service layer

MCP, REST, and CLI code should parse inputs and delegate. Validation, transformation, and business decisions belong in `ExampleService` and action metadata.

## Secure defaults

- `.env` is ignored and blocked from commits.
- Non-loopback HTTP should be authenticated unless explicitly behind a trusted gateway.
- Plugin manifests do not carry versions; marketplace versioning comes from git SHA/tags.
- Secrets are never documented inline.

## Agent-first outputs

Agents need compact, stable, self-describing output. Prefer predictable JSON, helpful errors, and semantic examples.

## Tests prove meaning

A good test proves the returned data is correct. `echo` must return the exact message. `greet` must include the requested name. Resource tests must inspect schema content, not just status codes.
