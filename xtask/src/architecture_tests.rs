use super::*;
use serde_json::json;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from("/repo")
}

fn pkg(name: &str, rel: &str, layer: &str, deps: Vec<Value>) -> Value {
    json!({
        "id": name,
        "name": name,
        "source": null,
        "manifest_path": format!("/repo/{rel}/Cargo.toml"),
        "metadata": {
            "soma-architecture": {
                "layer": layer
            }
        },
        "dependencies": deps
    })
}

fn dep(name: &str, rel: &str) -> Value {
    dep_with(name, rel, "normal", false)
}

fn dep_with(name: &str, rel: &str, kind: &str, optional: bool) -> Value {
    json!({
        "name": name,
        "path": format!("/repo/{rel}"),
        "kind": if kind == "normal" { Value::Null } else { Value::String(kind.to_owned()) },
        "optional": optional
    })
}

fn graph(packages: Vec<Value>) -> Graph {
    let metadata = json!({ "packages": packages });
    Graph::from_metadata(&root(), &metadata).expect("synthetic metadata graph")
}

fn failures(packages: Vec<Value>) -> Vec<String> {
    check_graph(&graph(packages))
}

#[test]
fn valid_shared_only_graph_passes() {
    let failures = failures(vec![
        pkg("soma-auth", "crates/shared/auth", "shared", vec![]),
        pkg(
            "soma-gateway",
            "crates/shared/mcp/gateway",
            "shared",
            vec![dep("soma-auth", "crates/shared/auth")],
        ),
        pkg("soma-api", "crates/soma/api", "product-surface", vec![]),
        pkg(
            "soma",
            "apps/soma",
            "app",
            vec![dep("soma-api", "crates/soma/api")],
        ),
    ]);

    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn shared_optional_dependency_on_product_fails() {
    let failures = failures(vec![
        pkg(
            "soma-gateway",
            "crates/shared/mcp/gateway",
            "shared",
            vec![dep_with(
                "soma-contracts",
                "crates/soma/contracts",
                "normal",
                true,
            )],
        ),
        pkg("soma-contracts", "crates/soma/contracts", "legacy", vec![]),
    ]);

    let report = failures.join("\n");
    assert!(report.contains("shared package soma-gateway"));
    assert!(report.contains("optional normal"));
}

#[test]
fn shared_transitive_graph_must_stay_shared_only() {
    let failures = failures(vec![
        pkg(
            "soma-gateway",
            "crates/shared/mcp/gateway",
            "shared",
            vec![dep("soma-codemode", "crates/shared/codemode")],
        ),
        pkg(
            "soma-codemode",
            "crates/shared/codemode",
            "shared",
            vec![dep("soma-service", "crates/soma/service")],
        ),
        pkg("soma-service", "crates/soma/service", "legacy", vec![]),
    ]);

    assert!(failures
        .join("\n")
        .contains("shared all-features graph for soma-gateway"));
}

#[test]
fn metadata_layer_must_match_physical_path() {
    let failures = failures(vec![pkg(
        "soma-auth",
        "crates/shared/auth",
        "product-surface",
        vec![],
    )]);

    assert!(failures
        .join("\n")
        .contains("declares architecture layer \"product-surface\""));
}

#[test]
fn surfaces_must_not_depend_on_one_another() {
    let failures = failures(vec![
        pkg(
            "soma-api",
            "crates/soma/api",
            "product-surface",
            vec![dep("soma-mcp", "crates/soma/mcp")],
        ),
        pkg("soma-mcp", "crates/soma/mcp", "product-surface", vec![]),
    ]);

    assert!(failures
        .join("\n")
        .contains("surface packages soma-api, soma-cli, and soma-mcp"));
}

#[test]
fn only_app_or_integration_may_bridge_application_ports_to_engines() {
    let failures = failures(vec![
        pkg(
            "soma-api",
            "crates/soma/api",
            "product-surface",
            vec![
                dep("soma-service", "crates/soma/service"),
                dep("soma-gateway", "crates/shared/mcp/gateway"),
            ],
        ),
        pkg("soma-service", "crates/soma/service", "legacy", vec![]),
        pkg(
            "soma-gateway",
            "crates/shared/mcp/gateway",
            "shared",
            vec![],
        ),
    ]);

    assert!(failures
        .join("\n")
        .contains("depends on both product application ports and concrete shared engines"));
}

#[test]
fn internal_dependency_cycles_fail() {
    let failures = failures(vec![
        pkg(
            "soma-api",
            "crates/soma/api",
            "product-surface",
            vec![dep("soma-runtime", "crates/soma/runtime")],
        ),
        pkg(
            "soma-runtime",
            "crates/soma/runtime",
            "product-runtime",
            vec![dep("soma-api", "crates/soma/api")],
        ),
    ]);

    assert!(failures
        .join("\n")
        .contains("internal dependency cycle detected"));
}
