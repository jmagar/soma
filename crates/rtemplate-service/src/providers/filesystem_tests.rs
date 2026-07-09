use std::fs;

use tempfile::tempdir;

use super::filesystem::{FileProviderSource, ProviderFileInspectionStatus};

#[test]
fn inspect_reports_loaded_disabled_and_invalid_files_without_executing_handlers() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path();

    fs::write(
        providers.join("hello.json"),
        r#"{
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
        }"#,
    )
    .expect("write provider");

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
    assert_eq!(report.providers_loaded, 1);
    assert_eq!(report.providers_disabled, 1);
    assert_eq!(report.providers_invalid, 1);

    let hello = report
        .files
        .iter()
        .find(|file| file.file_name == "hello.json")
        .unwrap();
    assert_eq!(hello.status, ProviderFileInspectionStatus::Loaded);
    assert_eq!(hello.provider_id.as_deref(), Some("hello"));
    assert_eq!(hello.actions, vec!["hello"]);

    let disabled = report
        .files
        .iter()
        .find(|file| file.file_name == "disabled.json")
        .unwrap();
    assert_eq!(disabled.status, ProviderFileInspectionStatus::Disabled);
    assert_eq!(disabled.provider_id.as_deref(), Some("disabled"));

    let broken = report
        .files
        .iter()
        .find(|file| file.file_name == "broken.json")
        .unwrap();
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
    assert_eq!(report.providers_loaded, 0);
    assert_eq!(report.providers_disabled, 0);
    assert_eq!(report.providers_invalid, 0);
}
