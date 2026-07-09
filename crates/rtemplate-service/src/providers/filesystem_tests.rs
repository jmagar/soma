use std::fs;

use tempfile::tempdir;

use super::filesystem::{
    FileProviderSource, ProviderDirectoryInspection, ProviderFileInspection,
    ProviderFileInspectionStatus,
};

const HELLO_PROVIDER_JSON: &str = r#"{
  "schema_version": 1,
  "provider": { "name": "hello", "kind": "static-rust", "version": "0.1.0" },
  "tools": [
    {
      "name": "hello",
      "description": "Hello probe",
      "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
      "output_schema": { "type": "object", "properties": {}, "additionalProperties": true }
    }
  ]
}"#;

fn file_named<'a>(
    report: &'a ProviderDirectoryInspection,
    name: &str,
) -> &'a ProviderFileInspection {
    report
        .files
        .iter()
        .find(|file| file.file_name == name)
        .unwrap_or_else(|| panic!("expected provider file `{name}` in inspection report"))
}

#[test]
fn inspect_reports_loaded_disabled_and_invalid_files_without_executing_handlers() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path();

    fs::write(providers.join("hello.json"), HELLO_PROVIDER_JSON).expect("write provider");

    fs::write(
        providers.join("disabled.json"),
        r#"{
          "schema_version": 1,
          "provider": { "name": "disabled", "kind": "static-rust", "enabled": false },
          "tools": []
        }"#,
    )
    .expect("write disabled provider");

    fs::write(providers.join("broken.json"), "{").expect("write invalid provider");
    fs::write(providers.join("notes.txt"), "ignored").expect("write ignored file");

    let report = FileProviderSource::new(providers)
        .inspect()
        .expect("inspect providers");

    assert_eq!(report.root, providers);
    assert!(report.exists);
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.providers_loaded(), 1);
    assert_eq!(report.providers_disabled(), 1);
    assert_eq!(report.providers_invalid(), 1);

    let hello = file_named(&report, "hello.json");
    assert_eq!(hello.status, ProviderFileInspectionStatus::Loaded);
    assert_eq!(hello.provider_id.as_deref(), Some("hello"));
    assert_eq!(hello.actions, vec!["hello"]);

    let disabled = file_named(&report, "disabled.json");
    assert_eq!(disabled.status, ProviderFileInspectionStatus::Disabled);
    assert_eq!(disabled.provider_id.as_deref(), Some("disabled"));

    let broken = file_named(&report, "broken.json");
    assert_eq!(broken.status, ProviderFileInspectionStatus::Invalid);
    assert!(broken
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("broken.json"));
}

#[test]
fn inspect_missing_directory_is_a_valid_empty_report() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("providers");

    let report = FileProviderSource::new(&missing)
        .inspect()
        .expect("inspect missing dir");

    assert_eq!(report.root, missing);
    assert!(!report.exists);
    assert!(report.files.is_empty());
    assert_eq!(report.providers_loaded(), 0);
    assert_eq!(report.providers_disabled(), 0);
    assert_eq!(report.providers_invalid(), 0);
}

#[test]
fn inspect_marks_semantically_invalid_provider_manifests_invalid() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path();

    fs::write(
        providers.join("duplicate-tools.json"),
        r#"{
          "schema_version": 1,
          "provider": { "name": "duplicate-tools", "kind": "static-rust", "version": "0.1.0" },
          "tools": [
            {
              "name": "dup",
              "description": "Duplicate probe one",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
              "name": "dup",
              "description": "Duplicate probe two",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false }
            }
          ]
        }"#,
    )
    .expect("write semantically invalid provider");

    let report = FileProviderSource::new(providers)
        .inspect()
        .expect("inspect providers");

    assert_eq!(report.providers_loaded(), 0);
    assert_eq!(report.providers_invalid(), 1);

    let duplicate = file_named(&report, "duplicate-tools.json");
    assert_eq!(duplicate.status, ProviderFileInspectionStatus::Invalid);
    assert!(duplicate
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("duplicate_tool_name"));
}

#[test]
fn inspect_marks_invalid_runtime_provider_config_invalid() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path();

    fs::write(
        providers.join("missing-openapi-base.json"),
        r#"{
          "schema_version": 1,
          "provider": { "name": "missing-openapi-base", "kind": "openapi", "version": "0.1.0" },
          "tools": [
            {
              "name": "call_missing_base",
              "description": "Missing OpenAPI base URL",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
              "rest": { "enabled": true, "method": "POST", "path": "/call" }
            }
          ]
        }"#,
    )
    .expect("write runtime-invalid provider");

    let report = FileProviderSource::new(providers)
        .inspect()
        .expect("inspect providers");

    assert_eq!(report.providers_loaded(), 0);
    assert_eq!(report.providers_invalid(), 1);

    let provider = file_named(&report, "missing-openapi-base.json");
    assert_eq!(provider.status, ProviderFileInspectionStatus::Invalid);
    assert!(provider
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("missing_openapi_base_url"));
}
