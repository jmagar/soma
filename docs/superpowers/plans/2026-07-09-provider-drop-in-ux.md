# Provider Drop-In UX Implementation Plan

> **For Jacob:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan step-by-step.

**Goal:** Make the already-wired dynamic provider runtime obvious and operable: a user can drop provider files into `providers/`, run first-class CLI checks, see clear docs/examples, and trust that MCP/CLI/HTTP will pick them up without a rebuild.

**Historical note:** This is the executed implementation plan, not the provider
manifest source of truth. Some inline snippets below are planning sketches from
before implementation adapted to the live `ProviderManifest` schema. Use
`docs/PROVIDERS.md`, `examples/providers/`, and
`docs/contracts/provider-manifest.schema.json` for authoritative manifest shape.

**Architecture:** Keep the existing runtime path intact. `rtemplate-service` remains the source of truth for provider discovery and loading. Add a non-executing inspection API around `FileProviderSource`, then build `rtemplate providers list|validate|status` on top of that same API. Document the workflow in `README.md`, `docs/PROVIDERS.md`, and safe example files under `examples/providers/`.

**Current evidence:**
- `dynamic_provider_registry()` already uses `RTEMPLATE_PROVIDER_DIR` or `./providers`.
- `FileProviderSource` already loads `.json`, `.ts`, and `.wasm`; missing directories produce an empty provider set.
- MCP `list_tools()` and resource reads refresh file providers, so running servers see newly dropped files.
- CLI startup refreshes file providers and can dispatch unknown action names as dynamic provider commands.
- `drop_provider_probe.rs` already proves hot-drop behavior across stdio MCP, CLI, and HTTP.
- Missing piece: discoverable UX. There is no polished `providers` CLI command, no clear `RTEMPLATE_PROVIDER_DIR` docs, and no root-level authoring guide.

**Out of scope:**
- No daemon control plane or remote reload endpoint. The runtime already refreshes on MCP tool/resource reads and CLI startup.
- No network calls or handler execution during validation. Validation inspects manifests only.
- No new provider kind. This is about making the existing provider kinds usable.

## Task 1: Add Provider Directory Inspection In `rtemplate-service`

**Files:**
- Modify: `crates/rtemplate-service/src/providers/filesystem.rs`
- Add: `crates/rtemplate-service/src/providers/filesystem_tests.rs`

**Step 1: Add a failing service-level inspection test**

Create `crates/rtemplate-service/src/providers/filesystem_tests.rs`:

```rust
use std::fs;

use tempfile::tempdir;

use super::{FileProviderSource, ProviderFileInspectionStatus};

#[test]
fn inspect_reports_loaded_disabled_and_invalid_files_without_executing_handlers() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path();

    fs::write(
        providers.join("hello.json"),
        r#"{
          "schema_version": "1",
          "kind": "static-rust",
          "metadata": { "id": "hello", "name": "Hello", "version": "0.1.0" },
          "tools": [
            {
              "name": "hello",
              "description": "Hello probe",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
              "output_schema": { "type": "object", "properties": {}, "additionalProperties": true }
            }
          ]
        }"#,
    )
    .expect("write provider");

    fs::write(
        providers.join("disabled.json"),
        r#"{
          "schema_version": "1",
          "kind": "static-rust",
          "metadata": { "id": "disabled", "name": "Disabled", "version": "0.1.0" },
          "enabled": false,
          "tools": []
        }"#,
    )
    .expect("write disabled provider");

    fs::write(providers.join("broken.json"), "{").expect("write invalid provider");
    fs::write(providers.join("notes.txt"), "ignored").expect("write ignored file");

    let report = FileProviderSource::new(providers).inspect().expect("inspect providers");

    assert_eq!(report.root, providers);
    assert!(report.exists);
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.providers_loaded, 1);
    assert_eq!(report.providers_disabled, 1);
    assert_eq!(report.providers_invalid, 1);

    let hello = report.files.iter().find(|file| file.file_name == "hello.json").unwrap();
    assert_eq!(hello.status, ProviderFileInspectionStatus::Loaded);
    assert_eq!(hello.provider_id.as_deref(), Some("hello"));
    assert_eq!(hello.actions, vec!["hello"]);

    let disabled = report.files.iter().find(|file| file.file_name == "disabled.json").unwrap();
    assert_eq!(disabled.status, ProviderFileInspectionStatus::Disabled);
    assert_eq!(disabled.provider_id.as_deref(), Some("disabled"));

    let broken = report.files.iter().find(|file| file.file_name == "broken.json").unwrap();
    assert_eq!(broken.status, ProviderFileInspectionStatus::Invalid);
    assert!(broken.error.as_deref().unwrap_or_default().contains("broken.json"));
}

#[test]
fn inspect_missing_directory_is_a_valid_empty_report() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("providers");

    let report = FileProviderSource::new(&missing).inspect().expect("inspect missing dir");

    assert_eq!(report.root, missing);
    assert!(!report.exists);
    assert!(report.files.is_empty());
    assert_eq!(report.providers_loaded, 0);
    assert_eq!(report.providers_disabled, 0);
    assert_eq!(report.providers_invalid, 0);
}
```

Wire the sibling test module at the bottom of `filesystem.rs`:

```rust
#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
```

Run:

```bash
cargo test -p rtemplate-service providers::filesystem_tests
```

Expected: FAIL because the inspection API does not exist yet.

**Step 2: Implement inspection structs and scanning**

In `filesystem.rs`, add public report types near `FileProviderSource`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDirectoryInspection {
    pub root: PathBuf,
    pub exists: bool,
    pub files: Vec<ProviderFileInspection>,
    pub providers_loaded: usize,
    pub providers_disabled: usize,
    pub providers_invalid: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFileInspection {
    pub path: PathBuf,
    pub file_name: String,
    pub status: ProviderFileInspectionStatus,
    pub provider_id: Option<String>,
    pub provider_kind: Option<String>,
    pub actions: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFileInspectionStatus {
    Loaded,
    Disabled,
    Invalid,
}
```

Add `inspect()` to `impl FileProviderSource`:

```rust
pub fn inspect(&self) -> Result<ProviderDirectoryInspection, FileProviderLoadError> {
    if !self.root.exists() {
        return Ok(ProviderDirectoryInspection {
            root: self.root.clone(),
            exists: false,
            files: Vec::new(),
            providers_loaded: 0,
            providers_disabled: 0,
            providers_invalid: 0,
        });
    }

    let mut files = Vec::new();

    for entry in fs::read_dir(&self.root).map_err(|source| FileProviderLoadError::ReadDir {
        path: self.root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| FileProviderLoadError::ReadDir {
            path: self.root.clone(),
            source,
        })?;
        let path = entry.path();

        if !path.is_file() || !is_provider_file(&path) {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>")
            .to_string();

        match load_catalog(&path) {
            Ok(catalog) => {
                let provider_id = Some(catalog.metadata.id.clone());
                let provider_kind = Some(catalog.kind.to_string());
                let actions = catalog
                    .tools
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect::<Vec<_>>();
                let status = if catalog.enabled {
                    ProviderFileInspectionStatus::Loaded
                } else {
                    ProviderFileInspectionStatus::Disabled
                };

                files.push(ProviderFileInspection {
                    path,
                    file_name,
                    status,
                    provider_id,
                    provider_kind,
                    actions,
                    error: None,
                });
            }
            Err(error) => files.push(ProviderFileInspection {
                path,
                file_name,
                status: ProviderFileInspectionStatus::Invalid,
                provider_id: None,
                provider_kind: None,
                actions: Vec::new(),
                error: Some(error.to_string()),
            }),
        }
    }

    files.sort_by(|left, right| left.file_name.cmp(&right.file_name));

    let providers_loaded = files
        .iter()
        .filter(|file| file.status == ProviderFileInspectionStatus::Loaded)
        .count();
    let providers_disabled = files
        .iter()
        .filter(|file| file.status == ProviderFileInspectionStatus::Disabled)
        .count();
    let providers_invalid = files
        .iter()
        .filter(|file| file.status == ProviderFileInspectionStatus::Invalid)
        .count();

    Ok(ProviderDirectoryInspection {
        root: self.root.clone(),
        exists: true,
        files,
        providers_loaded,
        providers_disabled,
        providers_invalid,
    })
}
```

Add `Display` for `ProviderKind` if it does not already exist:

```rust
impl fmt::Display for ProviderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderKind::StaticRust => formatter.write_str("static-rust"),
            ProviderKind::AiSdk => formatter.write_str("ai-sdk"),
            ProviderKind::Wasm => formatter.write_str("wasm"),
            ProviderKind::Mcp => formatter.write_str("mcp"),
            ProviderKind::OpenApi => formatter.write_str("openapi"),
        }
    }
}
```

If `ProviderKind` lives outside `filesystem.rs`, put the `Display` impl next to that enum instead.

Run:

```bash
cargo test -p rtemplate-service providers::filesystem_tests
```

Expected: PASS.

## Task 2: Add `rtemplate providers` CLI Commands

**Files:**
- Add: `crates/rtemplate-cli/src/providers.rs`
- Add: `crates/rtemplate-cli/src/providers_tests.rs`
- Modify: `crates/rtemplate-cli/src/lib.rs`
- Modify: `crates/rtemplate-cli/src/cli_tests.rs`
- Modify if needed: `crates/rtemplate-cli/Cargo.toml`

**Step 1: Add failing parser tests**

In `cli_tests.rs`, add:

```rust
#[test]
fn parses_providers_list_with_dir_and_json() {
    let command = parse_command(&[
        "rtemplate".to_string(),
        "providers".to_string(),
        "list".to_string(),
        "--dir".to_string(),
        "/tmp/providers".to_string(),
        "--json".to_string(),
    ])
    .expect("parse command");

    assert!(matches!(command, Command::Providers(_)));
}

#[test]
fn parses_providers_validate_as_reserved_command() {
    let command = parse_command(&[
        "rtemplate".to_string(),
        "providers".to_string(),
        "validate".to_string(),
    ])
    .expect("parse command");

    assert!(matches!(command, Command::Providers(_)));
}
```

Run:

```bash
cargo test -p rtemplate-cli parses_providers
```

Expected: FAIL because `Command::Providers` does not exist.

**Step 2: Implement command parser**

In `providers.rs`, define:

```rust
use std::path::PathBuf;

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvidersCommand {
    List { dir: Option<PathBuf>, json: bool },
    Validate { dir: Option<PathBuf>, json: bool },
    Status { dir: Option<PathBuf>, json: bool },
}

pub fn parse_providers_command(args: &[String]) -> Result<ProvidersCommand> {
    let Some(subcommand) = args.first() else {
        return Err(anyhow!("missing providers subcommand: expected list, validate, or status"));
    };

    let mut dir = None;
    let mut json = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow!("--dir requires a value"))?;
                dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            unknown => return Err(anyhow!("unknown providers option: {unknown}")),
        }
    }

    match subcommand.as_str() {
        "list" => Ok(ProvidersCommand::List { dir, json }),
        "validate" => Ok(ProvidersCommand::Validate { dir, json }),
        "status" => Ok(ProvidersCommand::Status { dir, json }),
        unknown => Err(anyhow!("unknown providers subcommand: {unknown}")),
    }
}
```

In `lib.rs`:

```rust
mod providers;

pub use providers::ProvidersCommand;
```

Extend `Command`:

```rust
Providers(ProvidersCommand),
```

Route parsing:

```rust
"providers" => Command::Providers(providers::parse_providers_command(&args[2..])?),
```

Run:

```bash
cargo test -p rtemplate-cli parses_providers
```

Expected: PASS.

**Step 3: Add failing command output tests**

Create `providers_tests.rs`:

```rust
use std::fs;

use tempfile::tempdir;

use super::providers::{build_provider_report_json, build_provider_report_text, ProvidersCommand};

#[test]
fn providers_list_text_includes_loaded_provider_actions() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join("hello.json"),
        r#"{
          "schema_version": "1",
          "kind": "static-rust",
          "metadata": { "id": "hello", "name": "Hello", "version": "0.1.0" },
          "tools": [
            {
              "name": "hello",
              "description": "Hello probe",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
              "output_schema": { "type": "object", "properties": {}, "additionalProperties": true }
            }
          ]
        }"#,
    )
    .expect("write provider");

    let output = build_provider_report_text(&ProvidersCommand::List {
        dir: Some(temp.path().to_path_buf()),
        json: false,
    })
    .expect("build report");

    assert!(output.contains("Provider directory:"));
    assert!(output.contains("hello.json"));
    assert!(output.contains("hello"));
}

#[test]
fn providers_validate_json_marks_invalid_files_and_returns_valid_false() {
    let temp = tempdir().expect("tempdir");
    fs::write(temp.path().join("broken.json"), "{").expect("write invalid provider");

    let value = build_provider_report_json(&ProvidersCommand::Validate {
        dir: Some(temp.path().to_path_buf()),
        json: true,
    })
    .expect("build report");

    assert_eq!(value["valid"], false);
    assert_eq!(value["summary"]["invalid"], 1);
    assert_eq!(value["files"][0]["status"], "invalid");
}
```

Wire sibling tests in `lib.rs`:

```rust
#[cfg(test)]
#[path = "providers_tests.rs"]
mod providers_tests;
```

Run:

```bash
cargo test -p rtemplate-cli providers_
```

Expected: FAIL because report builders do not exist yet.

**Step 4: Implement CLI report generation and execution**

In `providers.rs`, implement:

```rust
use serde_json::{json, Value};

use rtemplate_service::providers::filesystem::{
    FileProviderSource, ProviderDirectoryInspection, ProviderFileInspectionStatus,
};

pub fn run_providers_command(command: ProvidersCommand) -> Result<()> {
    let json_output = match &command {
        ProvidersCommand::List { json, .. }
        | ProvidersCommand::Validate { json, .. }
        | ProvidersCommand::Status { json, .. } => *json,
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&build_provider_report_json(&command)?)?);
    } else {
        println!("{}", build_provider_report_text(&command)?);
    }

    if matches!(command, ProvidersCommand::Validate { .. }) {
        let report = inspect_for_command(&command)?;
        if report.providers_invalid > 0 {
            std::process::exit(1);
        }
    }

    Ok(())
}

pub fn build_provider_report_json(command: &ProvidersCommand) -> Result<Value> {
    let report = inspect_for_command(command)?;
    Ok(json!({
        "provider_dir": report.root,
        "exists": report.exists,
        "valid": report.providers_invalid == 0,
        "summary": {
            "loaded": report.providers_loaded,
            "disabled": report.providers_disabled,
            "invalid": report.providers_invalid,
            "files": report.files.len(),
        },
        "files": report.files.iter().map(|file| {
            json!({
                "path": file.path,
                "file_name": file.file_name,
                "status": status_label(file.status),
                "provider_id": file.provider_id,
                "provider_kind": file.provider_kind,
                "actions": file.actions,
                "error": file.error,
            })
        }).collect::<Vec<_>>(),
    }))
}

pub fn build_provider_report_text(command: &ProvidersCommand) -> Result<String> {
    let report = inspect_for_command(command)?;
    let mut output = String::new();

    output.push_str(&format!("Provider directory: {}\n", report.root.display()));
    output.push_str(&format!("Exists: {}\n", report.exists));
    output.push_str(&format!(
        "Summary: {} loaded, {} disabled, {} invalid\n",
        report.providers_loaded, report.providers_disabled, report.providers_invalid
    ));

    if report.files.is_empty() {
        output.push_str("Files: none\n");
        return Ok(output);
    }

    output.push_str("Files:\n");
    for file in report.files {
        let provider = file.provider_id.as_deref().unwrap_or("-");
        let kind = file.provider_kind.as_deref().unwrap_or("-");
        let actions = if file.actions.is_empty() {
            "-".to_string()
        } else {
            file.actions.join(", ")
        };

        output.push_str(&format!(
            "  {} [{}] provider={} kind={} actions={}\n",
            file.file_name,
            status_label(file.status),
            provider,
            kind,
            actions
        ));

        if let Some(error) = file.error {
            output.push_str(&format!("    error: {error}\n"));
        }
    }

    Ok(output)
}

fn inspect_for_command(command: &ProvidersCommand) -> Result<ProviderDirectoryInspection> {
    let dir = match command {
        ProvidersCommand::List { dir, .. }
        | ProvidersCommand::Validate { dir, .. }
        | ProvidersCommand::Status { dir, .. } => dir
            .clone()
            .or_else(|| std::env::var_os("RTEMPLATE_PROVIDER_DIR").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("providers")),
    };

    Ok(FileProviderSource::new(dir).inspect()?)
}

fn status_label(status: ProviderFileInspectionStatus) -> &'static str {
    match status {
        ProviderFileInspectionStatus::Loaded => "loaded",
        ProviderFileInspectionStatus::Disabled => "disabled",
        ProviderFileInspectionStatus::Invalid => "invalid",
    }
}
```

If `serde_json::json!` cannot serialize `PathBuf` directly in this crate, use `report.root.display().to_string()` and `file.path.display().to_string()`.

In `lib.rs::run()`, route before dynamic provider dispatch:

```rust
Command::Providers(command) => return providers::run_providers_command(command),
```

Run:

```bash
cargo test -p rtemplate-cli providers_
cargo test -p rtemplate-cli parses_providers
```

Expected: PASS.

## Task 3: Add End-To-End Provider CLI Coverage

**Files:**
- Add: `crates/rmcp-template/tests/provider_cli.rs`

**Step 1: Add failing black-box CLI tests**

Create `provider_cli.rs`:

```rust
use std::fs;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn binary() -> String {
    env!("CARGO_BIN_EXE_rtemplate").to_string()
}

#[test]
fn providers_list_json_reports_dropped_provider() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("providers");
    fs::create_dir(&providers).expect("create providers dir");
    fs::write(
        providers.join("hello.json"),
        r#"{
          "schema_version": "1",
          "kind": "static-rust",
          "metadata": { "id": "hello", "name": "Hello", "version": "0.1.0" },
          "tools": [
            {
              "name": "hello",
              "description": "Hello probe",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
              "output_schema": { "type": "object", "properties": {}, "additionalProperties": true }
            }
          ]
        }"#,
    )
    .expect("write provider");

    let output = Command::new(binary())
        .args(["providers", "list", "--dir"])
        .arg(&providers)
        .arg("--json")
        .output()
        .expect("run providers list");

    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let value: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(value["summary"]["loaded"], 1);
    assert_eq!(value["files"][0]["provider_id"], "hello");
    assert_eq!(value["files"][0]["actions"][0], "hello");
}

#[test]
fn providers_validate_fails_for_invalid_provider_file() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("providers");
    fs::create_dir(&providers).expect("create providers dir");
    fs::write(providers.join("broken.json"), "{").expect("write invalid provider");

    let output = Command::new(binary())
        .args(["providers", "validate", "--dir"])
        .arg(&providers)
        .output()
        .expect("run providers validate");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("invalid"));
}

#[test]
fn providers_status_uses_rtemplate_provider_dir_environment() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("custom-providers");
    fs::create_dir(&providers).expect("create providers dir");

    let output = Command::new(binary())
        .args(["providers", "status"])
        .env("RTEMPLATE_PROVIDER_DIR", &providers)
        .output()
        .expect("run providers status");

    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains(&providers.display().to_string()));
}
```

Run:

```bash
cargo test -p rmcp-template --test provider_cli
```

Expected: PASS if Task 2 is complete. If it fails because the root package does not expose `CARGO_BIN_EXE_rtemplate` for this integration test, move the test to the package that owns the binary and keep the same assertions.

## Task 4: Document The Drop-In Provider Workflow

**Files:**
- Add: `docs/PROVIDERS.md`
- Add: `examples/providers/README.md`
- Add: `examples/providers/hello-static.json`
- Add: `examples/providers/hello-ai-sdk.ts`
- Add: `examples/providers/hello-openapi.json`
- Modify: `README.md`
- Modify if present: `CLAUDE.md`

**Step 1: Add provider authoring guide**

Create `docs/PROVIDERS.md` with these sections:

```markdown
# Drop-In Providers

`rtemplate` loads provider files from `./providers` by default. Override the directory with `RTEMPLATE_PROVIDER_DIR` or with `rtemplate providers ... --dir <path>` for local checks.

## Supported Files

| Extension | Provider kind | What is loaded |
|---|---|---|
| `.json` | `static-rust`, `mcp`, `openapi` | Provider manifest JSON |
| `.ts` | `ai-sdk` | `export default { ... }` provider catalog metadata |
| `.wasm` | `wasm` | `rtemplate.provider` custom section |

Disabled manifests with `"enabled": false` are visible in validation output and are not registered at runtime.

## Check A Provider Directory

```bash
rtemplate providers status
rtemplate providers list --json
rtemplate providers validate
rtemplate providers validate --dir ./examples/providers
```

`validate` exits non-zero when any provider file is invalid.

## Runtime Loading

CLI commands refresh providers on startup:

```bash
rtemplate my_provider_action --json '{"message":"hello"}'
```

MCP servers refresh file providers when clients list tools or read the tools resource, so a newly dropped provider appears without rebuilding the binary.

HTTP dispatch uses the same registry:

```bash
curl -sS -X POST http://127.0.0.1:8080/v1/providers/my_provider_action \
  -H 'content-type: application/json' \
  -d '{"message":"hello"}'
```

## Safety Model

`providers list`, `providers status`, and `providers validate` inspect provider catalogs only. They do not execute TypeScript handlers, instantiate WASM handlers, call MCP upstreams, or fetch OpenAPI URLs.

## Examples

See `examples/providers/`.
```

Add enough concrete manifest fields to explain:
- `schema_version`
- `kind`
- `metadata.id`
- `tools[].name`
- JSON Schema input/output
- `enabled: false`
- `mcp` URL transport inference
- `openapi` pinned local specs and URL restrictions

**Step 2: Add safe examples outside the default `providers/` directory**

Create `examples/providers/README.md`:

```markdown
# Provider Examples

These files are examples only. Copy one into `./providers/` or point the runtime at this directory:

```bash
RTEMPLATE_PROVIDER_DIR=./examples/providers rtemplate providers list
```

The examples are intentionally outside the default `./providers` directory so local development does not load sample actions by accident.
```

Create `examples/providers/hello-static.json`:

```json
{
  "schema_version": "1",
  "kind": "static-rust",
  "metadata": {
    "id": "hello-static",
    "name": "Hello Static",
    "version": "0.1.0"
  },
  "tools": [
    {
      "name": "hello_static",
      "description": "Static manifest example action.",
      "input_schema": {
        "type": "object",
        "properties": {
          "message": { "type": "string" }
        },
        "additionalProperties": false
      },
      "output_schema": {
        "type": "object",
        "additionalProperties": true
      }
    }
  ]
}
```

Create `examples/providers/hello-ai-sdk.ts`:

```ts
export default {
  schema_version: "1",
  kind: "ai-sdk",
  metadata: {
    id: "hello-ai-sdk",
    name: "Hello AI SDK",
    version: "0.1.0",
  },
  tools: [
    {
      name: "hello_ai_sdk",
      description: "AI SDK TypeScript provider example.",
      input_schema: {
        type: "object",
        properties: {
          message: { type: "string" },
        },
        additionalProperties: false,
      },
      output_schema: {
        type: "object",
        additionalProperties: true,
      },
    },
  ],
  handler: async (input) => ({
    ok: true,
    echoed: input.message ?? null,
  }),
};
```

Create `examples/providers/hello-openapi.json` with a local pinned spec path rather than a remote URL. Use an existing fixture path if one already exists in the repo; otherwise include a tiny companion spec under `examples/providers/openapi/hello.yaml` and point the manifest at it.

**Step 3: Update README**

Add a short top-level section:

```markdown
## Drop-In Providers

Drop `.json`, `.ts`, or `.wasm` provider files into `./providers` and `rtemplate` will expose their actions through CLI, MCP, and HTTP without rebuilding the binary. Use `RTEMPLATE_PROVIDER_DIR` to point at another directory.

```bash
rtemplate providers validate
rtemplate providers list
rtemplate providers status --json
```

See `docs/PROVIDERS.md` and `examples/providers/`.
```

Also fix stale architecture wording that still points only at `actions.rs`; mention the dynamic provider registry if the section currently implies static actions are the only dispatch path.

**Step 4: Update agent memory if the repo has `CLAUDE.md`**

If `CLAUDE.md` exists in the repo, add one concise bullet under project conventions:

```markdown
- Dynamic providers load from `./providers` by default or `RTEMPLATE_PROVIDER_DIR`; use `rtemplate providers validate` before committing provider examples or runtime docs.
```

If sibling `AGENTS.md` or `GEMINI.md` exists, ensure they are symlinks to `CLAUDE.md` and do not edit them directly.

Run:

```bash
test ! -e AGENTS.md -o -L AGENTS.md
test ! -e GEMINI.md -o -L GEMINI.md
```

Expected: PASS or no files present.

## Task 5: Full Verification

Run focused tests first:

```bash
cargo test -p rtemplate-service providers::filesystem_tests
cargo test -p rtemplate-cli parses_providers
cargo test -p rtemplate-cli providers_
cargo test -p rmcp-template --test provider_cli
```

Run existing provider runtime probes:

```bash
cargo test -p rmcp-template --test drop_provider_probe
cargo test -p rmcp-template --test ai_sdk_provider
cargo test -p rmcp-template --test wasm_provider
cargo test -p rmcp-template --test mcp_provider
cargo test -p rmcp-template --test openapi_provider
```

Run workspace gates:

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo xtask check-provider-manifest-contract
cargo xtask check-openapi-drift
cargo xtask check-schema-docs --check
cargo xtask check-palette-manifest --check
cargo xtask check-version-sync
git diff --check
```

If any generated artifact check fails with a write hint, run the matching `--write` command, inspect the diff, and rerun the check.

## Task 6: Commit And Close The Loop

Inspect changes:

```bash
git status --short
git diff --stat
git diff -- README.md docs/PROVIDERS.md examples/providers
git diff -- crates/rtemplate-service/src/providers/filesystem.rs crates/rtemplate-cli/src/lib.rs crates/rtemplate-cli/src/providers.rs
```

Commit:

```bash
git add README.md docs/PROVIDERS.md examples/providers crates/rtemplate-service/src/providers/filesystem.rs crates/rtemplate-service/src/providers/filesystem_tests.rs crates/rtemplate-cli/src/lib.rs crates/rtemplate-cli/src/providers.rs crates/rtemplate-cli/src/providers_tests.rs crates/rtemplate-cli/src/cli_tests.rs crates/rmcp-template/tests/provider_cli.rs
git commit -m "feat: add provider drop-in CLI workflow"
```

Final response should include:
- `rtemplate providers list|validate|status` added.
- Provider docs/examples added.
- Verification commands run and result.
- Any files intentionally left uncommitted.
