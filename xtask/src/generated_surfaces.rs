use anyhow::{bail, Context, Result};
use rtemplate_contracts::config::ExampleConfig;
use rtemplate_contracts::providers::ProviderCatalog;
use rtemplate_service::{
    dynamic_provider_registry, static_provider_registry, ExampleClient, ExampleService,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "generated_surfaces_docs.rs"]
mod generated_surfaces_docs;

use generated_surfaces_docs::{render_provider_docs, render_provider_skill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Check,
    Write,
    CheckAndWrite,
    Help,
}

impl Mode {
    fn parse(args: &[String], usage: &str) -> Result<Self> {
        let mut check = false;
        let mut write = false;
        for arg in args {
            match arg.as_str() {
                "--check" => check = true,
                "--write" => write = true,
                "--help" | "-h" => {
                    println!("{usage}");
                    return Ok(Self::Help);
                }
                unknown => bail!("unknown option: {unknown}"),
            }
        }
        Ok(match (check, write) {
            (false, false) | (true, false) => Self::Check,
            (false, true) => Self::Write,
            (true, true) => Self::CheckAndWrite,
        })
    }

    fn should_check(self) -> bool {
        matches!(self, Self::Check | Self::CheckAndWrite)
    }

    fn should_write(self) -> bool {
        matches!(self, Self::Write | Self::CheckAndWrite)
    }
}

pub fn check_palette_manifest(args: &[String]) -> Result<()> {
    let mode = Mode::parse(
        args,
        "Usage: cargo xtask check-palette-manifest [--check] [--write]",
    )?;
    let root = std::env::current_dir().context("failed to read cwd")?;
    let rendered = canonical_json(&render_palette_manifest()?)?;
    let out = root.join("docs/generated/palette-manifest.json");

    if mode.should_write() {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&out, &rendered).with_context(|| format!("failed to write {}", out.display()))?;
        println!("wrote {}", relative_display(&root, &out));
    }

    if mode.should_check() {
        if !out.exists() {
            bail!("docs/generated/palette-manifest.json is missing; run cargo xtask check-palette-manifest --write");
        }
        let current = fs::read_to_string(&out)
            .with_context(|| format!("failed to read {}", out.display()))?;
        if current != rendered {
            bail!("docs/generated/palette-manifest.json is stale; run cargo xtask check-palette-manifest --write");
        }
        println!("Palette manifest is current");
    }
    Ok(())
}

pub fn provider_surfaces(args: &[String]) -> Result<()> {
    let mode = Mode::parse(
        args,
        "Usage: cargo xtask generate-provider-surfaces [--check] [--write]",
    )?;
    let root = std::env::current_dir().context("failed to read cwd")?;
    let snapshot = render_provider_snapshot()?;
    let files = [
        (
            root.join("docs/generated/provider-surfaces.json"),
            canonical_json(&snapshot)?,
        ),
        (
            root.join("docs/generated/provider-surfaces.md"),
            render_provider_docs(&snapshot)?,
        ),
        (
            root.join("docs/generated/plugin.json"),
            canonical_json(&render_distribution_plugin(&snapshot))?,
        ),
        (
            root.join(".agents/plugins/marketplace.json"),
            canonical_json(&render_codex_marketplace())?,
        ),
        (
            root.join(".claude-plugin/marketplace.json"),
            canonical_json(&render_claude_marketplace())?,
        ),
    ];

    for (path, content) in files {
        if mode.should_write() {
            write_if_changed(&path, &content)?;
            println!("wrote {}", relative_display(&root, &path));
        }
        if mode.should_check() {
            if !path.exists() {
                bail!(
                    "{} is missing; run cargo xtask generate-provider-surfaces --write",
                    relative_display(&root, &path)
                );
            }
            let current = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if current != content {
                bail!(
                    "{} is stale; run cargo xtask generate-provider-surfaces --write",
                    relative_display(&root, &path)
                );
            }
        }
    }
    write_or_check_generated_skills(&root, &snapshot, mode)?;
    if mode.should_check() {
        println!("Provider surface artifacts are current");
    }
    Ok(())
}

fn render_palette_manifest() -> Result<Value> {
    let client = ExampleClient::new(&ExampleConfig {
        api_url: String::new(),
        api_key: "xtask".to_owned(),
    })?;
    let service = ExampleService::new(client);
    let registry = static_provider_registry(service)?;
    let snapshot = registry.snapshot();
    Ok(json!({
        "schema_version": 1,
        "provider_fingerprint": snapshot.fingerprint,
        "commands": snapshot.action_names(),
        "builtins": {
            "file_explorer": false,
            "github": false,
            "browser": false,
            "terminal": false
        },
        "limits": {
            "max_inline_schema_bytes": 16384,
            "max_examples_per_command": 3
        }
    }))
}

fn render_provider_snapshot() -> Result<Value> {
    let provider_dir = provider_dir();
    let client = ExampleClient::new(&ExampleConfig {
        api_url: String::new(),
        api_key: "xtask".to_owned(),
    })?;
    let service = ExampleService::new(client);
    let registry = dynamic_provider_registry(service)?;
    let snapshot = registry.refresh_file_providers()?;
    Ok(json!({
        "schema_version": 1,
        "provider_fingerprint": snapshot.fingerprint,
        "providers": snapshot.catalogs.iter().map(provider_summary).collect::<Vec<_>>(),
        "surfaces": {
            "mcp_actions": surface_actions(&snapshot.catalogs, Surface::Mcp),
            "cli_actions": surface_actions(&snapshot.catalogs, Surface::Cli),
            "cli_commands": cli_commands(&snapshot.catalogs),
            "rest_routes": rest_routes(&snapshot.catalogs),
            "docs": "docs/generated/provider-surfaces.md",
            "plugin": "docs/generated/plugin.json",
            "codex_marketplace": ".agents/plugins/marketplace.json",
            "claude_marketplace": ".claude-plugin/marketplace.json",
            "node_package": "packages/rtemplate-mcp/package.json",
            "provider_dir": provider_dir.display().to_string(),
            "provider_files": provider_files(&provider_dir)?,
            "generated_skills": generated_skill_paths(&snapshot.catalogs),
            "palette": "deferred until Axon tauri-palette port lands"
        }
    }))
}

fn provider_summary(catalog: &ProviderCatalog) -> Value {
    json!({
        "name": catalog.provider.name,
        "kind": catalog.provider.kind.as_str(),
        "title": catalog.provider.title,
        "description": catalog.provider.description,
        "when_to_use": catalog.docs.as_ref().and_then(|docs| docs.when_to_use.clone()),
        "tools": catalog.tools.iter().map(|tool| json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.input_schema,
            "output_schema": tool.output_schema,
            "scope": tool.scope,
            "destructive": tool.destructive,
            "requires_admin": tool.requires_admin,
            "cost": tool.cost,
            "env": tool.env.iter().map(|env| json!({
                "name": env.name,
                "required": env.required,
                "sensitive": env.sensitive,
                "server_prefixed": env.server_prefixed,
                "allow_unprefixed": env.allow_unprefixed,
                "description": env.description,
            })).collect::<Vec<_>>(),
            "mcp": tool.mcp.as_ref().map(|mcp| mcp.enabled).unwrap_or(true),
            "cli": tool.cli.as_ref().map(|cli| cli.enabled).unwrap_or(false),
            "cli_command": tool.cli.as_ref().filter(|cli| cli.enabled).and_then(|cli| cli.command.clone()).unwrap_or_else(|| if tool.cli.as_ref().map(|cli| cli.enabled).unwrap_or(false) { tool.name.clone() } else { "N/A".to_owned() }),
            "cli_aliases": tool.cli.as_ref().map(|cli| cli.aliases.clone()).unwrap_or_default(),
            "cli_flags": tool.cli.as_ref().map(|cli| cli.flags.clone()).unwrap_or_default(),
            "cli_default_output": tool.cli.as_ref().and_then(|cli| cli.default_output.clone()),
            "cli_usage": tool.meta.get("cli_usage").and_then(Value::as_str).map(ToOwned::to_owned),
            "rest": tool.rest.as_ref().map(|rest| rest.enabled).unwrap_or(false),
            "rest_route": tool.rest.as_ref().filter(|rest| rest.enabled).map(|rest| format!("{} {}", rest.method.as_deref().unwrap_or("POST"), rest.path.clone().unwrap_or_else(|| format!("/v1/{}", tool.name)))).unwrap_or_else(|| "N/A".to_owned()),
            "examples": tool.examples,
            "meta": tool.meta,
        })).collect::<Vec<_>>(),
        "prompts": catalog.prompts.iter().map(|prompt| json!({
            "name": prompt.name,
            "description": prompt.description,
            "arguments_schema": prompt.arguments_schema,
        })).collect::<Vec<_>>(),
        "resources": catalog.resources.iter().map(|resource| json!({
            "name": resource.name,
            "description": resource.description,
            "uri_template": resource.uri_template,
            "mime_type": resource.mime_type,
        })).collect::<Vec<_>>(),
        "tasks": catalog.tasks.iter().map(|task| json!({
            "name": task.name,
            "description": task.description,
            "input_schema": task.input_schema,
            "output_schema": task.output_schema,
        })).collect::<Vec<_>>(),
        "elicitation": catalog.elicitation.iter().map(|elicitation| json!({
            "name": elicitation.name,
            "description": elicitation.description,
            "schema": elicitation.schema,
        })).collect::<Vec<_>>(),
    })
}

fn render_distribution_plugin(snapshot: &Value) -> Value {
    json!({
        "schema_version": 1,
        "name": "rtemplate",
        "description": "Generated distributable plugin surface for rmcp-template.",
        "provider_fingerprint": snapshot["provider_fingerprint"].clone(),
        "plugin_root": "plugins/rtemplate",
        "codex": {
            "plugin_json": "plugins/rtemplate/.codex-plugin/plugin.json",
            "marketplace": ".agents/plugins/marketplace.json"
        },
        "claude": {
            "plugin_json": "plugins/rtemplate/.claude-plugin/plugin.json",
            "marketplace": ".claude-plugin/marketplace.json"
        },
        "skills": "plugins/rtemplate/skills",
        "node_package": "packages/rtemplate-mcp/package.json",
        "docs": "docs/generated/provider-surfaces.md",
        "mcp_server": "server.json",
        "provider_files": snapshot["surfaces"]["provider_files"].clone(),
        "surfaces": snapshot["surfaces"].clone(),
        "providers": snapshot["providers"].clone()
    })
}

fn generated_skill_paths(catalogs: &[ProviderCatalog]) -> Vec<String> {
    let mut paths = catalogs
        .iter()
        .map(|catalog| format!("docs/generated/skills/{}/SKILL.md", catalog.provider.name))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn write_or_check_generated_skills(root: &Path, snapshot: &Value, mode: Mode) -> Result<()> {
    let skills_root = root.join("docs/generated/skills");
    for provider in snapshot["providers"].as_array().into_iter().flatten() {
        let name = provider["name"].as_str().unwrap_or("provider");
        let path = skills_root.join(name).join("SKILL.md");
        let content = render_provider_skill(provider)?;
        if mode.should_write() {
            write_if_changed(&path, &content)?;
            println!("wrote {}", relative_display(root, &path));
        }
        if mode.should_check() {
            if !path.exists() {
                bail!(
                    "{} is missing; run cargo xtask generate-provider-surfaces --write",
                    relative_display(root, &path)
                );
            }
            let current = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if current != content {
                bail!(
                    "{} is stale; run cargo xtask generate-provider-surfaces --write",
                    relative_display(root, &path)
                );
            }
        }
    }
    Ok(())
}

fn render_codex_marketplace() -> Value {
    json!({
        "name": "rmcp-template",
        "plugins": [{
            "name": "rtemplate",
            "source": {
                "source": "local",
                "path": "./plugins/rtemplate"
            },
            "policy": {
                "installation": "AVAILABLE",
                "authentication": "ON_INSTALL"
            },
            "category": "Infrastructure",
            "interface": {
                "displayName": "Example MCP"
            }
        }]
    })
}

fn render_claude_marketplace() -> Value {
    json!({
        "$schema": "https://json.schemastore.org/claude-code-marketplace.json",
        "name": "rmcp-template",
        "description": "Generated marketplace catalog for rmcp-template plugins.",
        "owner": {
            "name": "Jacob Magar",
            "email": "jmagar@users.noreply.github.com"
        },
        "plugins": [{
            "name": "rtemplate",
            "description": "Example MCP plugin generated from rmcp-template.",
            "source": "./plugins/rtemplate",
            "category": "infrastructure"
        }]
    })
}

#[derive(Debug, Clone, Copy)]
enum Surface {
    Mcp,
    Cli,
}

fn surface_actions(catalogs: &[ProviderCatalog], surface: Surface) -> Vec<String> {
    let mut actions = catalogs
        .iter()
        .flat_map(|catalog| catalog.tools.iter())
        .filter_map(|tool| {
            let enabled = match surface {
                Surface::Mcp => tool.mcp.as_ref().map(|mcp| mcp.enabled).unwrap_or(true),
                Surface::Cli => tool.cli.as_ref().map(|cli| cli.enabled).unwrap_or(false),
            };
            enabled.then(|| tool.name.clone())
        })
        .collect::<Vec<_>>();
    actions.sort();
    actions
}

fn rest_routes(catalogs: &[ProviderCatalog]) -> Vec<String> {
    let mut routes = catalogs
        .iter()
        .flat_map(|catalog| catalog.tools.iter())
        .filter_map(|tool| {
            let rest = tool.rest.as_ref()?;
            rest.enabled.then(|| {
                format!(
                    "{} {}",
                    rest.method.as_deref().unwrap_or("POST"),
                    rest.path
                        .clone()
                        .unwrap_or_else(|| format!("/v1/{}", tool.name))
                )
            })
        })
        .collect::<Vec<_>>();
    routes.sort();
    routes
}

fn cli_commands(catalogs: &[ProviderCatalog]) -> Vec<String> {
    let mut commands = catalogs
        .iter()
        .flat_map(|catalog| catalog.tools.iter())
        .flat_map(|tool| {
            let Some(cli) = &tool.cli else {
                return Vec::new();
            };
            if !cli.enabled {
                return Vec::new();
            }
            let mut commands = vec![cli.command.clone().unwrap_or_else(|| tool.name.clone())];
            commands.extend(cli.aliases.clone());
            commands
        })
        .collect::<Vec<_>>();
    commands.sort();
    commands
}

fn provider_dir() -> std::path::PathBuf {
    std::env::var_os("RTEMPLATE_PROVIDER_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("providers"))
}

fn provider_files(provider_dir: &Path) -> Result<Vec<String>> {
    if !provider_dir.exists() {
        return Ok(Vec::new());
    }
    let label = provider_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("providers");
    let mut files = fs::read_dir(provider_dir)
        .with_context(|| format!("failed to read {}", provider_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("json" | "ts" | "wasm")
            )
        })
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| format!("{label}/{name}"))
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn canonical_json(value: &Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if path.exists()
        && fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?
            == content
    {
        return Ok(());
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
#[path = "generated_surfaces_tests.rs"]
mod generated_surfaces_tests;
