use soma_contracts::providers::ProviderCatalog;

use super::{apply_directory_wide_checks, DirectoryNamespace};
use crate::providers::filesystem::{ProviderFileInspection, ProviderFileInspectionStatus};

fn tool_catalog(provider_name: &str, tool_name: &str) -> ProviderCatalog {
    serde_json::from_str(&format!(
        r#"{{
          "schema_version": 1,
          "provider": {{ "name": "{provider_name}", "kind": "static-rust", "version": "0.1.0" }},
          "tools": [
            {{
              "name": "{tool_name}",
              "description": "probe",
              "input_schema": {{ "type": "object", "properties": {{}}, "additionalProperties": false }}
            }}
          ]
        }}"#
    ))
    .expect("valid catalog fixture")
}

#[test]
fn namespace_register_then_find_conflict_detects_duplicate_provider_name() {
    let mut namespace = DirectoryNamespace::default();
    namespace.register(&tool_catalog("shared", "action_one"), "a.json");

    let conflict = namespace.find_conflict(&tool_catalog("shared", "action_two"));
    assert!(conflict.unwrap().contains("duplicate provider"));
}

#[test]
fn namespace_register_then_find_conflict_detects_duplicate_action() {
    let mut namespace = DirectoryNamespace::default();
    namespace.register(&tool_catalog("provider-a", "shared_action"), "a.json");

    let conflict = namespace.find_conflict(&tool_catalog("provider-b", "shared_action"));
    assert!(conflict.unwrap().contains("duplicate action"));
}

#[test]
fn namespace_find_conflict_is_none_for_disjoint_catalogs() {
    let mut namespace = DirectoryNamespace::default();
    namespace.register(&tool_catalog("provider-a", "action_a"), "a.json");

    assert!(namespace
        .find_conflict(&tool_catalog("provider-b", "action_b"))
        .is_none());
}

#[test]
fn apply_directory_wide_checks_handles_an_empty_directory_without_panicking() {
    // No drop-in catalogs at all — the built-in seed alone must not panic or
    // spuriously flag anything (there's nothing to flag).
    let mut files: Vec<ProviderFileInspection> = Vec::new();
    let catalogs: Vec<Option<ProviderCatalog>> = Vec::new();

    apply_directory_wide_checks(&mut files, &catalogs);

    assert!(files.is_empty());
}

#[test]
fn apply_directory_wide_checks_leaves_the_first_file_loaded_and_invalidates_the_second() {
    let mut files = vec![
        ProviderFileInspection {
            path: "a.json".into(),
            file_name: "a.json".to_owned(),
            status: ProviderFileInspectionStatus::Loaded,
            provider_id: Some("provider-a".to_owned()),
            provider_kind: Some("static-rust".to_owned()),
            actions: vec!["shared_action".to_owned()],
            error: None,
        },
        ProviderFileInspection {
            path: "b.json".into(),
            file_name: "b.json".to_owned(),
            status: ProviderFileInspectionStatus::Loaded,
            provider_id: Some("provider-b".to_owned()),
            provider_kind: Some("static-rust".to_owned()),
            actions: vec!["shared_action".to_owned()],
            error: None,
        },
    ];
    let catalogs = vec![
        Some(tool_catalog("provider-a", "shared_action")),
        Some(tool_catalog("provider-b", "shared_action")),
    ];

    apply_directory_wide_checks(&mut files, &catalogs);

    assert_eq!(files[0].status, ProviderFileInspectionStatus::Loaded);
    assert_eq!(files[1].status, ProviderFileInspectionStatus::Invalid);
    assert!(files[1]
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("a.json"));
}
