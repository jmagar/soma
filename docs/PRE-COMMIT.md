# Pre-commit

The repo uses `lefthook` for lightweight pre-commit checks. Install hooks with:

```bash
just install-hooks
```

Remove them with:

```bash
just uninstall-hooks
```

## Hook scripts

| Script | Purpose |
|---|---|
| `scripts/block-env-commits.sh` | Blocks staged `.env*` files except `.env.example`. |
| `scripts/check-version-sync.sh` | Ensures version-bearing files agree. |
| `scripts/check-file-size.sh` | Warns/fails on staged files above size budgets. |
| `taplo check` | Checks TOML formatting. |

## Manual equivalents

```bash
bash scripts/block-env-commits.sh
bash scripts/check-version-sync.sh
bash scripts/check-file-size.sh
taplo check
```

## Philosophy

Pre-commit checks should be fast and local. Full release confidence comes from `scripts/pre-release-check.sh`, not from blocking every commit with long builds.
