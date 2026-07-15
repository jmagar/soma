# Provider Examples

These files are examples only. Copy one into `./providers/` or point the runtime
at this directory:

```bash
SOMA_PROVIDER_DIR=./examples/providers soma providers list
```

The examples are intentionally outside the default `./providers` directory so
local development does not load sample actions by accident.

Markdown files in this directory are exposed as MCP prompts. For example,
`code-review.md` appears as the `code-review` prompt.
