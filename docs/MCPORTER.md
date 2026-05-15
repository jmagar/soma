# mcporter

`mcporter` is used for live MCP integration testing and CLI generation.

## Test harness

The live test script is:

```bash
tests/mcporter/test-mcp.sh
```

Run it through Just:

```bash
just dev
just test-mcporter
```

It validates:

- auth rejection when `EXAMPLE_MCP_TOKEN` is set
- tool semantic behavior for `greet`, `echo`, `status`, and `help`
- MCP resource behavior for `example://schema/mcp-tool`

The resource suite prefers mcporter resource commands when available and falls back to JSON-RPC `resources/read` for older mcporter versions.

## Configuration

The script targets `http://<EXAMPLE_MCP_HOST>:<EXAMPLE_MCP_PORT>/mcp`, defaulting to `http://localhost:40060/mcp` to match `just dev`. It remaps `0.0.0.0` to `localhost`. If `EXAMPLE_MCP_TOKEN` is set, it sends `Authorization: Bearer <token>`.

## Generated CLIs

`just generate-cli` demonstrates generating a standalone CLI from a running MCP server. Generated CLIs may embed auth material; do not commit them unless they are intentionally scrubbed and reviewed.

## Test philosophy

Use semantic assertions, not liveness-only checks. For example, `echo` must round-trip the exact input, and `greet(name="Alice")` must mention `Alice`.
