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
  --define package_name=myservice-mcp \
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

`package_name`, `crate_prefix`, and binary names may use hyphens because Cargo
package names and executable names support them. `service_slug` is also used as
a Rust config field/module identifier, so keep it snake_case:
`^[a-z][a-z0-9_]*$`.

## After Generation

Run:

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
```

Then replace the stub transport client and service actions with the real
upstream or platform implementation.

## Template Verification

When changing the generator, run the real cargo-generate smoke test:

```bash
cargo xtask cargo-generate
```

For a faster shape-only check while iterating:

```bash
cargo xtask cargo-generate --no-cargo-check
```

The smoke test generates both a simple project and a project with hyphenated
Cargo package names, checks plugin/repository metadata, removes template-only
generation docs from the output, and runs `cargo check --workspace --all-targets`
inside each generated project. `scripts/check-cargo-generate.py` is only a
compatibility wrapper for the xtask command.
