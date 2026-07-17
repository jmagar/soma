# soma-cli-core

Reusable command-line plumbing extracted from the Soma CLI: output-format
selection, table and JSON rendering, confirmation I/O primitives,
terminal/color capability policy, shell-completion script generation, and
progress helpers.

The crate owns generic terminal mechanics only. It does not depend on any
Soma product crate and knows nothing about Soma commands, action names,
scopes, or product exit-code policy — those stay in `soma-cli`.

```rust
use soma_cli_core::color::{green, red};
use soma_cli_core::terminal::stderr_supports_color;

let enabled = stderr_supports_color();
println!("{}  {}", green("ok", enabled), red("failed", false));
```

## What this crate owns

- common CLI flag-scanning primitives (`common_args`)
- output-format selection between human and JSON rendering (`output`)
- JSON pretty-printing helpers (`json`)
- a minimal fixed-width table renderer (`table`)
- confirmation I/O primitives — "type the name to confirm" prompts (`confirmation`)
- terminal capability detection and `NO_COLOR` / `--color` policy (`terminal`)
- ANSI color/style helpers, plus the Aurora CLI token palette as reusable
  defaults (`color`)
- minimal progress-line helpers for long-running commands (`progress`)
- static shell-completion script generation for bash and zsh (`completion`)
- reusable human/JSON CLI error presentation (`error`)

## What this crate does not own

- Soma's CLI parser or command set
- dynamic provider command projection
- mapping to application requests
- product exit-code policy
- business confirmation policy — this crate can prompt a human, but does not
  decide which operations require confirmation
