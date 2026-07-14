//! Directory-wide uniqueness checks for non-executing provider inspection.
//!
//! Complements the per-file checks in `filesystem.rs`
//! (`validate_provider_manifest` + schema compilation): a manifest can pass
//! every per-file check individually and still collide with another
//! provider — or with the built-in `static-rust` provider — once loaded
//! together, which the live registry
//! (`provider_registry::{provider_map, build_snapshot}`) rejects.

use std::collections::HashMap;

use soma_contracts::providers::ProviderCatalog;

use super::{ProviderFileInspection, ProviderFileInspectionStatus};

/// Mirrors the cross-provider uniqueness checks
/// `provider_registry::{provider_map, build_snapshot}` run over the whole
/// *loaded* provider set: duplicate provider name, action/tool name, REST
/// route, CLI command/alias, and MCP primitive name. Two files can each pass
/// per-file validation individually and still collide once loaded together,
/// which the live registry (and `soma providers validate`) rejects — a
/// non-executing `lint` has to catch the same thing or it gives false
/// confidence. Only files still `Loaded` after the per-file pass participate;
/// `Disabled`/`Invalid`/`Skipped` files are excluded, matching `load()`,
/// which never registers a disabled provider in the first place.
///
/// The namespace is seeded with the built-in `static-rust` catalog first —
/// `dynamic_provider_registry_from_dir()` always loads it alongside drop-in
/// files (see `crate::dynamic_provider_registry_from_dir`), so a drop-in file
/// reusing a built-in provider name or action (e.g. `status`) collides at
/// real registry construction even though no *other drop-in file* is
/// involved. Lint has to reserve those names too.
///
/// On the first file to reuse an already-claimed name, that file (not the
/// original owner) is marked `Invalid`, mirroring the live registry's
/// insert-into-a-map-fails-on-second-entry semantics.
pub(super) fn apply_directory_wide_checks(
    files: &mut [ProviderFileInspection],
    loaded_catalogs: &[Option<ProviderCatalog>],
) {
    let mut namespace = DirectoryNamespace::default();
    namespace.register(
        &crate::providers::static_rust::StaticRustProvider::catalog_static(),
        BUILTIN_PROVIDER_LABEL,
    );

    for index in 0..files.len() {
        let Some(catalog) = &loaded_catalogs[index] else {
            continue;
        };

        if let Some(message) = namespace.find_conflict(catalog) {
            files[index].status = ProviderFileInspectionStatus::Invalid;
            files[index].actions = Vec::new();
            files[index].error = Some(message);
            continue;
        }

        namespace.register(catalog, &files[index].file_name);
    }
}

const BUILTIN_PROVIDER_LABEL: &str = "the built-in `static-rust` provider";

/// Tracks provider/action/REST-route/CLI-command/MCP-primitive names already
/// claimed in this directory (plus the built-in catalog), each mapped to a
/// human-readable label identifying the owner — a file name, or
/// [`BUILTIN_PROVIDER_LABEL`].
#[derive(Default)]
struct DirectoryNamespace {
    provider_names: HashMap<String, String>,
    action_names: HashMap<String, String>,
    rest_routes: HashMap<(String, String), String>,
    cli_commands: HashMap<String, String>,
    primitives: HashMap<String, String>,
}

impl DirectoryNamespace {
    /// Returns an error message for the first collision found, checking in
    /// the same order `provider_registry::{provider_map, build_snapshot}`
    /// does. Does not mutate — call `register` separately once the caller
    /// has decided this catalog is conflict-free.
    fn find_conflict(&self, catalog: &ProviderCatalog) -> Option<String> {
        if let Some(other) = self.provider_names.get(&catalog.provider.name) {
            return Some(conflict_message("provider", &catalog.provider.name, other));
        }
        for tool in &catalog.tools {
            if let Some(other) = self.action_names.get(&tool.name) {
                return Some(conflict_message("action", &tool.name, other));
            }
            if let Some(rest) = &tool.rest {
                if rest.enabled {
                    let key = rest_route_key(tool.name.as_str(), rest);
                    if let Some(other) = self.rest_routes.get(&key) {
                        let label = format!("{} {}", key.0, key.1);
                        return Some(conflict_message("REST route", &label, other));
                    }
                }
            }
            if let Some(cli) = &tool.cli {
                if cli.enabled {
                    let command = cli_command(tool.name.as_str(), cli);
                    if let Some(other) = self.cli_commands.get(&command) {
                        return Some(conflict_message("CLI command", &command, other));
                    }
                    for alias in &cli.aliases {
                        if let Some(other) = self.cli_commands.get(alias) {
                            return Some(conflict_message("CLI alias", alias, other));
                        }
                    }
                }
            }
        }
        for name in primitive_names(catalog) {
            if let Some(other) = self.primitives.get(&name) {
                return Some(conflict_message("MCP primitive", &name, other));
            }
        }
        None
    }

    fn register(&mut self, catalog: &ProviderCatalog, owner: &str) {
        self.provider_names
            .insert(catalog.provider.name.clone(), owner.to_owned());
        for tool in &catalog.tools {
            self.action_names
                .insert(tool.name.clone(), owner.to_owned());
            if let Some(rest) = &tool.rest {
                if rest.enabled {
                    self.rest_routes
                        .insert(rest_route_key(tool.name.as_str(), rest), owner.to_owned());
                }
            }
            if let Some(cli) = &tool.cli {
                if cli.enabled {
                    self.cli_commands
                        .insert(cli_command(tool.name.as_str(), cli), owner.to_owned());
                    for alias in &cli.aliases {
                        self.cli_commands.insert(alias.clone(), owner.to_owned());
                    }
                }
            }
        }
        for name in primitive_names(catalog) {
            self.primitives.insert(name, owner.to_owned());
        }
    }
}

fn conflict_message(kind: &str, name: &str, owner: &str) -> String {
    format!("duplicate {kind} `{name}` (already claimed by {owner})")
}

fn rest_route_key(
    tool_name: &str,
    rest: &soma_contracts::providers::RestOverlay,
) -> (String, String) {
    let method = rest.method.clone().unwrap_or_else(|| "POST".to_owned());
    let path = rest
        .path
        .clone()
        .unwrap_or_else(|| format!("/v1/{tool_name}"));
    (method, path)
}

fn cli_command(tool_name: &str, cli: &soma_contracts::providers::CliOverlay) -> String {
    cli.command.clone().unwrap_or_else(|| tool_name.to_owned())
}

fn primitive_names(catalog: &ProviderCatalog) -> Vec<String> {
    catalog
        .prompts
        .iter()
        .map(|prompt| prompt.name.clone())
        .chain(
            catalog
                .resources
                .iter()
                .map(|resource| resource.name.clone()),
        )
        .chain(catalog.tasks.iter().map(|task| task.name.clone()))
        .chain(
            catalog
                .elicitation
                .iter()
                .map(|elicitation| elicitation.name.clone()),
        )
        .collect()
}
