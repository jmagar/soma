use std::path::PathBuf;

use anyhow::{anyhow, Result};
use rtemplate_service::providers::filesystem::{
    FileProviderSource, ProviderDirectoryInspection, ProviderFileInspectionStatus,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvidersCommand {
    List { dir: Option<PathBuf>, json: bool },
    Validate { dir: Option<PathBuf>, json: bool },
    Status { dir: Option<PathBuf>, json: bool },
}

impl ProvidersCommand {
    fn dir(&self) -> &Option<PathBuf> {
        match self {
            Self::List { dir, .. } | Self::Validate { dir, .. } | Self::Status { dir, .. } => dir,
        }
    }

    fn json(&self) -> bool {
        match self {
            Self::List { json, .. } | Self::Validate { json, .. } | Self::Status { json, .. } => {
                *json
            }
        }
    }

    fn validates(&self) -> bool {
        matches!(self, Self::Validate { .. })
    }
}

pub fn parse_providers_command(args: &[String]) -> Result<ProvidersCommand> {
    let Some(subcommand) = args.first() else {
        return Err(anyhow!(
            "missing providers subcommand: expected list, validate, or status"
        ));
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

pub fn run_providers_command(command: ProvidersCommand) -> Result<()> {
    let report = inspect_for_command(&command)?;
    if command.json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&provider_report_json(&report))?
        );
    } else {
        println!("{}", provider_report_text(&report));
    }

    if command.validates() && report.providers_invalid > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
pub fn build_provider_report_json(command: &ProvidersCommand) -> Result<Value> {
    Ok(provider_report_json(&inspect_for_command(command)?))
}

#[cfg(test)]
pub fn build_provider_report_text(command: &ProvidersCommand) -> Result<String> {
    Ok(provider_report_text(&inspect_for_command(command)?))
}

fn inspect_for_command(command: &ProvidersCommand) -> Result<ProviderDirectoryInspection> {
    let dir = command
        .dir()
        .clone()
        .or_else(|| std::env::var_os("RTEMPLATE_PROVIDER_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("providers"));

    Ok(FileProviderSource::new(dir).inspect()?)
}

fn provider_report_json(report: &ProviderDirectoryInspection) -> Value {
    json!({
        "provider_dir": report.root.display().to_string(),
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
                "path": file.path.display().to_string(),
                "file_name": file.file_name,
                "status": status_label(file.status),
                "provider_id": file.provider_id,
                "provider_kind": file.provider_kind,
                "actions": file.actions,
                "error": file.error,
            })
        }).collect::<Vec<_>>(),
    })
}

fn provider_report_text(report: &ProviderDirectoryInspection) -> String {
    let mut output = String::new();

    output.push_str(&format!("Provider directory: {}\n", report.root.display()));
    output.push_str(&format!("Exists: {}\n", report.exists));
    output.push_str(&format!(
        "Summary: {} loaded, {} disabled, {} invalid\n",
        report.providers_loaded, report.providers_disabled, report.providers_invalid
    ));

    if report.files.is_empty() {
        output.push_str("Files: none\n");
        return output;
    }

    output.push_str("Files:\n");
    for file in &report.files {
        let provider = file.provider_id.as_deref().unwrap_or("-");
        let kind = file.provider_kind.as_deref().unwrap_or("-");
        let actions = if file.actions.is_empty() {
            "-".to_owned()
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

        if let Some(error) = &file.error {
            output.push_str(&format!("    error: {error}\n"));
        }
    }

    output
}

fn status_label(status: ProviderFileInspectionStatus) -> &'static str {
    match status {
        ProviderFileInspectionStatus::Loaded => "loaded",
        ProviderFileInspectionStatus::Disabled => "disabled",
        ProviderFileInspectionStatus::Invalid => "invalid",
    }
}
