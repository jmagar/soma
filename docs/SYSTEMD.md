# systemd

The template supports user-level systemd deployments when a unit named `example-mcp.service` is installed by the derived service.

## Expected binary

Install the release binary somewhere stable, commonly:

```bash
cargo build --release
install -m 755 target/release/example ~/.local/bin/example
```

## Runtime verification

Use the runtime checker to detect stale running processes:

```bash
scripts/check-runtime-current.sh --mode systemd --expected-binary target/release/example
just runtime-current
```

The checker compares:

- `/proc/<pid>/exe` for the running service process
- the unit `ExecStart` binary
- optional `--expected-binary`

If hashes differ, restart the unit after installing the new binary.

## Restart flow

```bash
systemctl --user daemon-reload
systemctl --user restart example-mcp.service
systemctl --user status example-mcp.service
```

## Environment

Prefer an `EnvironmentFile` that points at the repo or appdata `.env`. Never hardcode tokens in unit files. See `docs/ENV.md` for variable meanings.
