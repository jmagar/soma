use std::{collections::HashMap, sync::Arc};

use jsonschema::JSONSchema;
use rtemplate_contracts::{
    provider_validation::{validate_provider_manifest, ProviderValidationError},
    providers::ProviderCatalog,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::provider_registry::{Provider, ProviderSurface, RegistrySnapshot, ToolEntry};

pub(crate) fn build_snapshot(
    providers: Vec<Arc<dyn Provider>>,
) -> Result<RegistrySnapshot, ProviderValidationError> {
    let mut catalogs = Vec::new();
    let mut action_index = HashMap::new();
    let mut rest_index = HashMap::new();
    let mut cli_index = HashMap::new();
    let mut primitive_index = HashMap::new();
    let mut compiled_validator_count = 0usize;

    for provider in providers {
        let catalog = provider.catalog();
        validate_provider_catalog_for_runtime(&catalog)?;
        for tool in &catalog.tools {
            let input_validator =
                Arc::new(JSONSchema::compile(&tool.input_schema).map_err(|error| {
                    ProviderValidationError::new(
                        "input_schema_invalid",
                        format!("tool `{}` has invalid input_schema: {error}", tool.name),
                    )
                })?);
            compiled_validator_count += 1;
            let action = tool.name.clone();
            let entry = ToolEntry {
                provider: catalog.provider.name.clone(),
                action: action.clone(),
                tool: tool.clone(),
                capabilities: catalog.capabilities.clone(),
                input_validator,
            };
            if action_index.insert(action.clone(), entry).is_some() {
                return Err(ProviderValidationError::new(
                    "duplicate_tool_name",
                    format!("duplicate action `{action}`"),
                ));
            }
            index_rest_route(&mut rest_index, tool)?;
            index_cli_command(&mut cli_index, tool)?;
        }
        for prompt in &catalog.prompts {
            insert_primitive(&mut primitive_index, "prompt", &prompt.name)?;
        }
        for resource in &catalog.resources {
            insert_primitive(&mut primitive_index, "resource", &resource.name)?;
        }
        for task in &catalog.tasks {
            insert_primitive(&mut primitive_index, "task", &task.name)?;
        }
        for elicitation in &catalog.elicitation {
            insert_primitive(&mut primitive_index, "elicitation", &elicitation.name)?;
        }
        catalogs.push(catalog);
    }

    catalogs.sort_by(|left, right| left.provider.name.cmp(&right.provider.name));
    let fingerprint = fingerprint_catalogs(&catalogs);
    let id = fingerprint.clone();
    let mut action_names = action_index.keys().cloned().collect::<Vec<_>>();
    action_names.sort();
    let openapi_paths = openapi_paths_from_rest_index(&rest_index);
    let cached_catalog_summary = Arc::new(json!({
        "schema_version": 1,
        "provider_fingerprint": fingerprint,
        "actions": action_names.clone(),
    }));
    let cached_palette_manifest = Arc::new(json!({
        "schema_version": 1,
        "provider_fingerprint": fingerprint,
        "commands": action_names,
        "builtins": {
            "file_explorer": false,
            "github": false,
            "browser": false,
            "terminal": false
        }
    }));
    let cached_openapi_bytes = Arc::new(
        serde_json::to_vec_pretty(&json!({
            "openapi": "3.1.0",
            "info": {"title": "rmcp-template provider API", "version": env!("CARGO_PKG_VERSION")},
            "x-template": {"preferred_rest_style": "direct_routes"},
            "x-rtemplate": {"provider_fingerprint": fingerprint},
            "paths": openapi_paths
        }))
        .expect("static OpenAPI summary serializes"),
    );

    Ok(RegistrySnapshot {
        id,
        fingerprint,
        catalogs,
        action_index,
        rest_index,
        cli_index,
        primitive_index,
        compiled_validator_count,
        cached_openapi_bytes,
        cached_catalog_summary,
        cached_palette_manifest,
    })
}

pub fn validate_provider_catalog_for_runtime(
    catalog: &ProviderCatalog,
) -> Result<(), ProviderValidationError> {
    validate_provider_manifest(catalog)?;
    for tool in &catalog.tools {
        JSONSchema::compile(&tool.input_schema).map_err(|error| {
            ProviderValidationError::new(
                "input_schema_invalid",
                format!("tool `{}` has invalid input_schema: {error}", tool.name),
            )
        })?;
    }
    Ok(())
}

pub(crate) fn surface_actions(
    snapshot: &RegistrySnapshot,
    surface: ProviderSurface,
) -> Vec<String> {
    let mut actions = snapshot
        .action_index
        .iter()
        .filter(|(_, entry)| surface_enabled(entry, surface))
        .map(|(action, _)| action.clone())
        .collect::<Vec<_>>();
    actions.sort();
    actions
}

pub(crate) fn rest_routes(snapshot: &RegistrySnapshot) -> Vec<String> {
    let mut routes = snapshot
        .rest_index
        .keys()
        .map(|(method, path)| format!("{method} {path}"))
        .collect::<Vec<_>>();
    routes.sort();
    routes
}

fn index_rest_route(
    rest_index: &mut HashMap<(String, String), String>,
    tool: &rtemplate_contracts::providers::ProviderTool,
) -> Result<(), ProviderValidationError> {
    let Some(rest) = &tool.rest else {
        return Ok(());
    };
    if !rest.enabled {
        return Ok(());
    }
    let method = rest.method.clone().unwrap_or_else(|| "POST".to_owned());
    let path = rest
        .path
        .clone()
        .unwrap_or_else(|| format!("/v1/{}", tool.name));
    if rest_index
        .insert((method.clone(), path.clone()), tool.name.clone())
        .is_some()
    {
        return Err(ProviderValidationError::new(
            "duplicate_rest_route",
            format!("duplicate REST route {method} {path}"),
        ));
    }
    Ok(())
}

fn index_cli_command(
    cli_index: &mut HashMap<String, String>,
    tool: &rtemplate_contracts::providers::ProviderTool,
) -> Result<(), ProviderValidationError> {
    let Some(cli) = &tool.cli else {
        return Ok(());
    };
    if !cli.enabled {
        return Ok(());
    }
    let command = cli.command.clone().unwrap_or_else(|| tool.name.clone());
    insert_cli_name(cli_index, &command, &tool.name, "command")?;
    for alias in &cli.aliases {
        insert_cli_name(cli_index, alias, &tool.name, "alias")?;
    }
    Ok(())
}

fn insert_cli_name(
    cli_index: &mut HashMap<String, String>,
    cli_name: &str,
    action: &str,
    label: &str,
) -> Result<(), ProviderValidationError> {
    if cli_index
        .insert(cli_name.to_owned(), action.to_owned())
        .is_some()
    {
        return Err(ProviderValidationError::new(
            "duplicate_cli_command",
            format!("duplicate CLI {label} `{cli_name}`"),
        ));
    }
    Ok(())
}

fn surface_enabled(entry: &ToolEntry, surface: ProviderSurface) -> bool {
    match surface {
        ProviderSurface::Mcp => entry
            .tool
            .mcp
            .as_ref()
            .map(|mcp| mcp.enabled)
            .unwrap_or(true),
        ProviderSurface::Rest => entry
            .tool
            .rest
            .as_ref()
            .map(|rest| rest.enabled)
            .unwrap_or(false),
        ProviderSurface::Cli => entry
            .tool
            .cli
            .as_ref()
            .map(|cli| cli.enabled)
            .unwrap_or(false),
        ProviderSurface::Palette => entry
            .tool
            .palette
            .as_ref()
            .map(|palette| palette.enabled)
            .unwrap_or(true),
    }
}

fn openapi_paths_from_rest_index(rest_index: &HashMap<(String, String), String>) -> Value {
    let mut paths = Map::new();
    paths.insert(
        "/v1/capabilities".to_owned(),
        json!({
            "get": {
                "summary": "List REST capabilities",
                "operationId": "v1Capabilities",
                "responses": {
                    "200": {"description": "Route inventory and server metadata"}
                }
            }
        }),
    );

    let mut routes = rest_index
        .iter()
        .map(|((method, path), action)| (method.clone(), path.clone(), action.clone()))
        .collect::<Vec<_>>();
    routes.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));

    for (method, path, action) in routes {
        let entry = paths
            .entry(path)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(methods) = entry {
            methods.insert(
                method.to_ascii_lowercase(),
                json!({
                    "summary": format!("Provider action `{action}`"),
                    "operationId": action,
                    "responses": {
                        "200": {"description": "Provider action response"},
                        "400": {"description": "Provider validation error"}
                    }
                }),
            );
        }
    }
    Value::Object(paths)
}

fn insert_primitive(
    index: &mut HashMap<String, String>,
    kind: &str,
    name: &str,
) -> Result<(), ProviderValidationError> {
    if index.insert(name.to_owned(), kind.to_owned()).is_some() {
        return Err(ProviderValidationError::new(
            "duplicate_mcp_primitive",
            format!("duplicate MCP primitive `{name}`"),
        ));
    }
    Ok(())
}

fn fingerprint_catalogs(catalogs: &[ProviderCatalog]) -> String {
    let canonical = serde_json::to_vec(catalogs).expect("catalogs serialize");
    let digest = Sha256::digest(canonical);
    format!("sha256:{digest:x}")
}
