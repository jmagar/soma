# cargo-generate

`rmcp-template` can be used directly with `cargo-generate` while still staying a
normal compileable Rust repository.

The template avoids Liquid placeholders in live Rust and TOML files. Instead,
`cargo-generate` collects the project values, then runs a post-generation hook
that renames packages, binaries, env prefixes, scopes, type names, and plugin
paths in the generated copy.

## Install

```bash
cargo install cargo-generate
```

## Generate

Because the post hook runs a local Python rewrite script, pass
`--allow-commands`:

```bash
cargo generate \
  --git https://github.com/jmagar/rtemplate-mcp \
  --name myservice-mcp \
  --allow-commands
```

Useful non-interactive form:

```bash
cargo generate \
  --git https://github.com/jmagar/rtemplate-mcp \
  --name myservice-mcp \
  --allow-commands \
  --define crate_name=myservice-mcp \
  --define crate_prefix=myservice \
  --define binary_name=myservice \
  --define server_binary_name=myservice-server \
  --define service_slug=myservice \
  --define type_prefix=MyService \
  --define env_prefix=MYSERVICE \
  --define scope_prefix=myservice \
  --define default_port=40060 \
  --define github_owner=jmagar \
  --define github_repo=myservice-mcp
```

## After Generation

Run:

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
```

Then replace the stub transport client and service actions with the real
upstream or platform implementation.

