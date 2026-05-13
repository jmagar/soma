# Quickstart — 5 minutes to a working MCP server

## Prerequisites

- Rust 1.86+ (`rustup update stable`)
- `just` command runner: `cargo install just` (optional but convenient)

## 1. Run the stub template

```bash
git clone https://github.com/jmagar/rmcp-template
cd rmcp-template
cargo run -- serve
```

The server starts on `http://localhost:3100`. In another terminal:

```bash
# Health check (no auth required)
curl http://localhost:3100/health
# {"status":"ok"}

# Call the greet action
curl -s -X POST http://localhost:3100/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"example","arguments":{"action":"greet","name":"Alice"}}}'

# List available tools
curl -s -X POST http://localhost:3100/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

## 2. Try the CLI

```bash
cargo run -- greet --name Alice
cargo run -- echo --message "Hello, MCP!"
cargo run -- status
cargo run -- --help
```

## 3. Try stdio transport

```bash
cargo run -- mcp
# Server reads JSON-RPC from stdin. Send:
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

## 4. Run the tests

```bash
cargo test
```

All tests pass with no credentials needed — the stubs return hardcoded JSON.

## 5. Add bearer auth

Generate a token:

```bash
openssl rand -hex 32
# → e.g. a3f2c1...
```

Start with auth:

```bash
EXAMPLE_MCP_TOKEN=a3f2c1... cargo run -- serve
```

Now all `/mcp` calls require `Authorization: Bearer a3f2c1...`:

```bash
curl -s -X POST http://localhost:3100/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Authorization: Bearer a3f2c1..." \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"example","arguments":{"action":"status"}}}'
```

## 6. Connect Claude Desktop

Add to your Claude Desktop MCP config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "example": {
      "command": "/path/to/rmcp-template/target/debug/example",
      "args": ["mcp"],
      "env": { "RUST_LOG": "warn" }
    }
  }
}
```

Or use Streamable HTTP (server must be running):

```json
{
  "mcpServers": {
    "example": {
      "url": "http://localhost:3100/mcp"
    }
  }
}
```

## Next steps

- Read the [README](../README.md) for the step-by-step guide to adapting this template for your own API.
- Read [CLAUDE.md](../CLAUDE.md) for the thin-shim rule and how to add actions.
- For OAuth setup, set `EXAMPLE_MCP_AUTH_MODE=oauth` and the `EXAMPLE_MCP_GOOGLE_*` env vars — see the env var table in the README.
