# Scripts

Maintenance scripts live in `scripts/`. The authoritative per-script usage reference is `scripts/README.md`.

## Categories

| Category | Scripts |
|---|---|
| Release gates | `pre-release-check.sh`, `check-version-sync.sh`, `check-blob-size.py`, `check-coupled-files.sh` |
| Hygiene | `asciicheck.py`, `check-file-size.sh`, `block-env-commits.sh` |
| MCP/plugin validation | `check-schema-docs.py`, `validate-plugin-layout.sh`, `check-plugin-hook-contract.py`, `test-mcp-auth.sh` |
| Runtime/deploy | `check-runtime-current.sh`, `sync-cargo.sh`, `bump-version.sh` |
| Reference docs | `refresh-docs.sh` |

## Important commands

```bash
scripts/pre-release-check.sh
scripts/pre-release-check.sh --mcporter
scripts/refresh-docs.sh --dry-run
scripts/test-mcp-auth.sh --url http://localhost:3100/mcp --token <token>
```

## Contract

- Scripts should be portable Bash or Python.
- Mutating scripts must be explicit about what they write.
- Release checks must be repeatable; generated plugin binaries are allowlisted in `scripts/blob-size-allowlist.txt`.
- Keep `scripts/README.md` current when adding, renaming, or changing scripts.
