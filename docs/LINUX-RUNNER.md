---
title: "Linux CI Runner"
doc_type: "guide"
status: "active"
owner: "rmcp-template"
audience:
  - "contributors"
  - "operators"
  - "agents"
scope: "template"
source_of_truth: false
last_reviewed: "2026-06-22"
---

# Linux CI Runner

The Linux jobs in `.github/workflows/ci.yml` and `.github/workflows/scheduled.yml`
run on self-hosted runners (`runs-on: [self-hosted, Linux, rmcp-template, dookie]`),
mirroring the Windows/steamy setup in `docs/WINDOWS-RUNNER.md`. This avoids billed
GitHub-hosted minutes entirely.

> **TEMPLATE:** the `dookie` label and `/opt/gha-*` paths below are this repo's
> deployment. A fresh clone either keeps `runs-on: ubuntu-latest` (GitHub-hosted)
> or repoints the Linux jobs at its own runner label and re-derives the isolation
> below for its host.

## Why Self-Hosted Linux Runners

- **Free.** On GitHub Free a private repo caps GitHub-*hosted* minutes; **self-hosted
  minutes are unlimited and unbilled.** When hosted minutes run out every
  `ubuntu-latest` job fails at "Set up job" with a billing error — self-hosted
  sidesteps that.
- **Faster after warm-up.** dookie is ~20 cores / 48 GB vs a 4-core hosted runner,
  and the cargo registry + `target/` persist on local disk between runs.

Trade-off: dookie is a shared dev box, so CI competes with local agent/build load.

## Workflow Shape

Every `ubuntu-latest` job in `ci.yml` and `scheduled.yml` is repointed to:

```yaml
runs-on: [self-hosted, Linux, rmcp-template, dookie]
```

`dookie` is registered as a custom label in `.github/actionlint.yaml` (alongside
`rmcp-template` and `steamy`) so the `actionlint` job does not reject it.

## Current Dookie Runners

Three runners run in parallel (so ~3 jobs run concurrently):

| Runner | Dir | `HOME` | Work dir | systemd service |
|---|---|---|---|---|
| `dookie-linux-1` | `~/actions-runner` | `/opt/gha-home-1` | `/opt/gha-home-1/_work` | `actions.runner.jmagar-template-rmcp.dookie-linux-1` |
| `dookie-linux-2` | `~/actions-runner-2` | `/opt/gha-home-2` | `/opt/gha-home-2/_work` | `…dookie-linux-2` |
| `dookie-linux-3` | `~/actions-runner-3` | `/opt/gha-home-3` | `/opt/gha-home-3/_work` | `…dookie-linux-3` |

Labels on each: `self-hosted`, `Linux`, `X64`, `rmcp-template`, `dookie`.

Manage them:

```bash
# status (GitHub side)
gh api repos/jmagar/template-rmcp/actions/runners \
  -q '.runners[] | select(.name|startswith("dookie")) | "\(.name)\t\(.status)\t\(.busy)"'

# control a runner's service (run from its dir)
cd ~/actions-runner   # or -2 / -3
sudo ./svc.sh status   # status | stop | start
journalctl -u actions.runner.jmagar-template-rmcp.dookie-linux-1 -f
```

## Isolation Design

CI must never touch the developer's `$HOME`, cargo cache, sccache, or
`~/.<service>/.env`. Each runner is walled off via its `.env` and `.path` files
(read by the runner for every job). **`HOME` is per-runner and the work dir lives
inside it** so nothing under `/home/jmagar` is written, while toolchains and mise
are shared/redirected for speed.

`~/actions-runner*/.env`:

```ini
HOME=/opt/gha-home-1                              # per runner (-2, -3)
CARGO_HOME=/opt/gha-cargo                         # shared, isolated (not ~/.cargo)
RUSTUP_HOME=/opt/gha-rustup                       # shared, pre-seeded stable+default
NPM_CONFIG_PREFIX=/opt/gha-home-1/.npm-global     # per runner
MISE_DATA_DIR=/home/jmagar/.local/share/mise      # reuse dev mise installs/shims
MISE_CONFIG_DIR=/home/jmagar/.config/mise
MISE_STATE_DIR=/home/jmagar/.local/state/mise     # trust records live here
MISE_CACHE_DIR=/home/jmagar/.cache/mise
LANG=en_US.UTF-8
```

`~/actions-runner*/.path` (single line):

```
/opt/gha-cargo/bin:/opt/gha-home-1/.npm-global/bin:/home/jmagar/.cargo/bin:/home/jmagar/.local/share/mise/shims:/home/jmagar/.local/bin:/home/linuxbrew/.linuxbrew/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
```

Why each entry matters:

- `/opt/gha-cargo/bin` — installed cargo proxies + `cargo install` outputs (taplo).
- `/home/jmagar/.cargo/bin` — the `rustup`/`cargo` proxy binaries (they read
  `CARGO_HOME`/`RUSTUP_HOME` at runtime, so they touch only the isolated dirs).
- `/home/jmagar/.local/share/mise/shims` — `node`, `npm`, `pnpm`, `jq`, `go`,
  `python3` (rust is **not** mise-managed).
- `/home/jmagar/.local/bin` — the `mise` **binary** itself; mise-managed `npm`
  calls `mise` to reshim after `npm install -g`, so the binary (not just shims)
  must be on `PATH`.

The shared `RUSTUP_HOME` is pre-seeded once so jobs that run `cargo` without a
`dtolnay/rust-toolchain` step still have a default toolchain:

```bash
RUSTUP_HOME=/opt/gha-rustup CARGO_HOME=/opt/gha-cargo \
  ~/.cargo/bin/rustup toolchain install stable --profile minimal --component rustfmt,clippy
RUSTUP_HOME=/opt/gha-rustup ~/.cargo/bin/rustup default stable
```

## Self-Hosted Linux Runner Setup

For each runner instance (repeat with a fresh dir + `HOME` for parallelism):

```bash
# 1. Download + extract the runner
mkdir -p ~/actions-runner && cd ~/actions-runner
curl -fsSL -o runner.tgz \
  https://github.com/actions/runner/releases/download/v2.335.1/actions-runner-linux-x64-2.335.1.tar.gz
tar xzf runner.tgz && rm runner.tgz
sudo ./bin/installdependencies.sh

# 2. Isolated work dir + home (OUTSIDE /home/jmagar so dev config can't leak)
sudo mkdir -p /opt/gha-home-1 && sudo chown jmagar:jmagar /opt/gha-home-1

# 3. Write .env and .path (see "Isolation Design" above)

# 4. Register (token via gh; expires in 1h)
TOKEN=$(gh api -X POST repos/jmagar/template-rmcp/actions/runners/registration-token -q .token)
./config.sh --unattended --url https://github.com/jmagar/template-rmcp --token "$TOKEN" \
  --name dookie-linux-1 --labels rmcp-template,dookie --work /opt/gha-home-1/_work

# 5. Install + start the systemd service
sudo ./svc.sh install jmagar
sudo ./svc.sh start
```

To remove/re-register: `sudo ./svc.sh stop && sudo ./svc.sh uninstall`, then
`./config.sh remove --token $(gh api -X POST .../actions/runners/remove-token -q .token)`.

## Security — Push-Only Triggers

Self-hosted runners must **never** run untrusted code. `ci.yml` therefore has **no
`pull_request` trigger** — only `push` to `main` and manual `workflow_dispatch`.
This stops automated dependabot PRs (and any PR) from building third-party code on
dookie. `scheduled.yml` runs only on a weekly cron + dispatch.

Invariant: **no workflow that is `runs-on: …dookie…` may have a `pull_request`
trigger.** Audit with:

```bash
for f in .github/workflows/*.yml; do
  grep -q dookie "$f" && grep -qE '^  pull_request:' "$f" && echo "LEAK: $f"
done
```

Optional belt-and-suspenders: GitHub → Settings → Actions → General → "Require
approval for all outside collaborators" (closes the residual case of a PR that
*adds* a dookie-targeted workflow; dependabot cannot do this).

## Required Tools

Jobs install their own toolchains via actions where possible; the runner host
supplies the rest via `PATH`:

| Tool | Source |
|---|---|
| `cargo` / `rustc` | `dtolnay/rust-toolchain` into shared `RUSTUP_HOME` (pre-seeded default) |
| `node` / `npm` / `pnpm` / `jq` / `go` / `python3` | mise shims (`MISE_*` env points at dev mise dirs) |
| `mise` binary | `/home/jmagar/.local/bin` on `PATH` (needed for npm reshim) |
| `taplo` | `cargo install taplo-cli --locked` (not install-action — see Troubleshooting) |
| `docker` / `docker compose` | system docker (runner user is in the `docker` group) |
| `cargo-nextest` | `taiki-e/install-action` (works; taplo is the exception) |

## Troubleshooting

Every CI failure during bring-up was self-hosted environment integration, not repo
code. Root causes and fixes:

- **All jobs fail in ~2s at "Set up job"** → GitHub-hosted billing exhausted. The
  whole point of self-hosted; ignore the billing prompt.
- **Builds use sccache / mold / `[unstable] codegen-backend`** → the dev's
  `~/.cargo/config.toml` leaked in because the work dir was under `/home/jmagar`
  (cargo walks parent dirs for `.cargo/config.toml`). Fix: work dir under an
  isolated `HOME` (`/opt/gha-home-N`), plus `CARGO_BUILD_RUSTC_WRAPPER=""` in
  `ci.yml` to disable the repo's `scripts/cargo-rustc-wrapper`.
- **`actions/cache` reads/writes `~/.cargo`** → on dookie `~` is the dev home. Fix:
  cache only `target/` (the registry persists in the isolated `CARGO_HOME`). Never
  cache `~/.cargo` on a self-hosted runner whose `HOME` is a real user.
- **MCP Smoke: `status`/`echo` return `execution_error`** → the test server loaded
  the dev's `~/.example/.env` (real `RTEMPLATE_API_URL`) → DeployedApi mode. Fix:
  isolated `HOME` → `load_dotenv()` finds nothing → stub mode.
- **Secret Scan: `rootDirectory … is not a parent of … results.sarif`** → gitleaks
  requires the workspace under `$HOME`. Fix: work dir inside the isolated `HOME`.
- **Frontend Assets: `mise: command not found`** → mise-managed `npm` reshims by
  calling the `mise` binary, which was not on `PATH` (only shims were). Fix: add
  `/home/jmagar/.local/bin` to `.path`.
- **mise shims: "config … not trusted" / GitHub rate limit** → under an isolated
  `HOME` mise lost its trust/cache. Fix: point `MISE_DATA_DIR`/`CONFIG_DIR`/
  `STATE_DIR`/`CACHE_DIR` at the dev's real mise dirs.
- **TOML: `install-action requires bash` / can't rm `cargo-binstall`** → taplo has
  no install-action prebuilt, so it falls back to cargo-binstall and collides with
  the mise-provided `cargo-binstall` on `PATH`. Fix: `cargo install taplo-cli`.
- **`rustup could not choose a version of cargo … no default configured`** → a
  fresh isolated `RUSTUP_HOME` has no default toolchain for steps that call `cargo`
  without `dtolnay/rust-toolchain`. Fix: pre-seed the shared `RUSTUP_HOME` with
  `rustup default stable`.
- **Build Windows: `cargo-rustc-wrapper … not a valid Win32 application`** → the
  repo's bash `scripts/cargo-rustc-wrapper` can't run as a rustc wrapper on
  Windows. Fix: `CARGO_BUILD_RUSTC_WRAPPER=""` in `ci.yml` (CI never needs the dev
  binary-install wrapper). See `docs/WINDOWS-RUNNER.md`.
