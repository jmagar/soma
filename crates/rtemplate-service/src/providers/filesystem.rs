use std::{
    fs,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use rtemplate_contracts::providers::{ProviderCatalog, ProviderKind};
use serde_json::json;
use url::Url;
use wasmtime::{Config, Engine, ExternType, Module};

use crate::{
    provider_errors::ProviderError,
    provider_registry::{
        validate_provider_catalog_for_runtime, Provider, ProviderCall, ProviderOutput,
    },
    providers::{
        ai_sdk::AiSdkProvider, mcp::McpProvider, openapi::OpenApiProvider, wasm::WasmProvider,
    },
};

#[derive(Debug, Clone)]
pub struct FileProviderSource {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDirectoryInspection {
    pub root: PathBuf,
    pub exists: bool,
    pub files: Vec<ProviderFileInspection>,
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

impl ProviderDirectoryInspection {
    pub fn providers_loaded(&self) -> usize {
        self.count_status(ProviderFileInspectionStatus::Loaded)
    }

    pub fn providers_disabled(&self) -> usize {
        self.count_status(ProviderFileInspectionStatus::Disabled)
    }

    pub fn providers_invalid(&self) -> usize {
        self.count_status(ProviderFileInspectionStatus::Invalid)
    }

    fn count_status(&self, status: ProviderFileInspectionStatus) -> usize {
        self.files
            .iter()
            .filter(|file| file.status == status)
            .count()
    }
}

impl FileProviderSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn inspect(&self) -> Result<ProviderDirectoryInspection, FileProviderLoadError> {
        match self.root.try_exists() {
            Ok(false) => {
                return Ok(ProviderDirectoryInspection {
                    root: self.root.clone(),
                    exists: false,
                    files: Vec::new(),
                });
            }
            Ok(true) => {}
            Err(source) => {
                return Err(FileProviderLoadError {
                    path: self.root.clone(),
                    message: format!("failed to inspect provider directory: {source}"),
                });
            }
        }

        let entries = fs::read_dir(&self.root).map_err(|source| FileProviderLoadError {
            path: self.root.clone(),
            message: format!("failed to read provider directory: {source}"),
        })?;
        let mut files = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|source| FileProviderLoadError {
                path: self.root.clone(),
                message: format!("failed to read provider directory entry: {source}"),
            })?;
            let path = entry.path();
            if !path.is_file() || !is_provider_file(&path) {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<unknown>")
                .to_owned();

            match load_catalog(&path) {
                Ok(catalog) => match validate_provider_catalog_for_runtime(&catalog)
                    .map_err(|error| error.to_string())
                    .and_then(|_| validate_runtime_config_for_inspection(&path, &catalog))
                {
                    Ok(()) => {
                        let status = if catalog.provider.enabled == Some(false) {
                            ProviderFileInspectionStatus::Disabled
                        } else {
                            ProviderFileInspectionStatus::Loaded
                        };
                        let actions = catalog
                            .tools
                            .iter()
                            .map(|tool| tool.name.clone())
                            .collect::<Vec<_>>();
                        files.push(ProviderFileInspection {
                            path,
                            file_name,
                            status,
                            provider_id: Some(catalog.provider.name.clone()),
                            provider_kind: Some(catalog.provider.kind.as_str().to_owned()),
                            actions,
                            error: None,
                        });
                    }
                    Err(error) => {
                        let message = format!("{}: {error}", path.display());
                        files.push(invalid_file_inspection(path, file_name, message));
                    }
                },
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
        Ok(ProviderDirectoryInspection {
            root: self.root.clone(),
            exists: true,
            files,
        })
    }

    pub fn load(&self) -> Result<Vec<std::sync::Arc<dyn Provider>>, FileProviderLoadError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut providers = Vec::new();
        let entries = fs::read_dir(&self.root).map_err(|source| FileProviderLoadError {
            path: self.root.clone(),
            message: format!("failed to read provider directory: {source}"),
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| FileProviderLoadError {
                path: self.root.clone(),
                message: format!("failed to read provider directory entry: {source}"),
            })?;
            let path = entry.path();
            if !path.is_file() || !is_provider_file(&path) {
                continue;
            }
            let catalog = load_catalog(&path)?;
            if catalog.provider.enabled == Some(false) {
                continue;
            }
            providers.push(provider_for_catalog(path, catalog));
        }
        Ok(providers)
    }
}

fn invalid_file_inspection(
    path: PathBuf,
    file_name: String,
    error: String,
) -> ProviderFileInspection {
    ProviderFileInspection {
        path,
        file_name,
        status: ProviderFileInspectionStatus::Invalid,
        provider_id: None,
        provider_kind: None,
        actions: Vec::new(),
        error: Some(error),
    }
}

fn validate_runtime_config_for_inspection(
    path: &Path,
    catalog: &ProviderCatalog,
) -> Result<(), String> {
    match catalog.provider.kind {
        ProviderKind::StaticRust => Ok(()),
        ProviderKind::Openapi => validate_openapi_config_for_inspection(catalog),
        ProviderKind::Mcp => validate_mcp_config_for_inspection(catalog),
        ProviderKind::AiSdk => validate_ai_sdk_config_for_inspection(path),
        ProviderKind::Wasm => validate_wasm_config_for_inspection(path),
    }
}

fn validate_openapi_config_for_inspection(catalog: &ProviderCatalog) -> Result<(), String> {
    let base_url = catalog
        .meta
        .get("openapi")
        .and_then(|value| value.get("base_url"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            catalog
                .meta
                .get("base_url")
                .and_then(serde_json::Value::as_str)
        })
        .ok_or_else(|| {
            "missing_openapi_base_url: OpenAPI provider requires meta.openapi.base_url".to_owned()
        })?;
    let base =
        Url::parse(base_url).map_err(|error| format!("invalid_openapi_base_url: {error}"))?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(
            "openapi_scheme_denied: OpenAPI provider base_url must use http or https".to_owned(),
        );
    }
    let host = base.host_str().ok_or_else(|| {
        "openapi_host_required: OpenAPI provider base_url must include a host".to_owned()
    })?;
    if let Some(network) = &catalog.capabilities.network {
        if network.enabled && !network.allowed_hosts.iter().any(|allowed| allowed == host) {
            return Err(format!(
                "openapi_host_not_allowed: OpenAPI provider host `{host}` is not declared in allowed_hosts"
            ));
        }
    }
    for tool in &catalog.tools {
        let operation_meta = tool.meta.get("openapi");
        let path = operation_meta
            .and_then(|value| value.get("path"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| tool.rest.as_ref().and_then(|rest| rest.path.as_deref()))
            .unwrap_or("");
        if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//") {
            return Err(format!(
                "openapi_absolute_operation_url_denied: tool `{}` operation path must be relative",
                tool.name
            ));
        }
    }
    Ok(())
}

fn validate_mcp_config_for_inspection(catalog: &ProviderCatalog) -> Result<(), String> {
    let meta = catalog
        .meta
        .get("mcp")
        .or_else(|| catalog.meta.get("runtime"))
        .ok_or_else(|| {
            "missing_mcp_runtime: MCP provider requires meta.mcp runtime config".to_owned()
        })?;
    let explicit = meta
        .get("transport")
        .and_then(serde_json::Value::as_str)
        .map(str::to_ascii_lowercase);
    let has_url = meta
        .get("url")
        .and_then(serde_json::Value::as_str)
        .is_some()
        || meta
            .get("http")
            .and_then(|http| http.get("url"))
            .and_then(serde_json::Value::as_str)
            .is_some();
    let has_stdio = meta.get("stdio").is_some()
        || meta
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some();

    match explicit.as_deref() {
        Some("http" | "streamable-http" | "streamable_http") if !has_url => {
            return Err("invalid_mcp_transport: transport=http requires url".to_owned());
        }
        Some("stdio") if !has_stdio => {
            return Err(
                "invalid_mcp_transport: transport=stdio requires stdio.command or command"
                    .to_owned(),
            );
        }
        Some("http" | "streamable-http" | "streamable_http" | "stdio") => {}
        Some(other) => {
            return Err(format!(
                "invalid_mcp_transport: unsupported MCP transport `{other}`"
            ));
        }
        None if !has_url && !has_stdio => {
            return Err(
                "missing_mcp_command: MCP provider stdio runtime requires command".to_owned(),
            );
        }
        None => {}
    }

    if has_url {
        let url = meta
            .get("url")
            .or_else(|| meta.get("http").and_then(|http| http.get("url")))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let parsed = Url::parse(url).map_err(|error| format!("invalid_mcp_url: {error}"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(format!(
                "invalid_mcp_url: url scheme `{}` is not supported",
                parsed.scheme()
            ));
        }
        if parsed.host_str().is_none() {
            return Err("invalid_mcp_url: url must include a host".to_owned());
        }
    }
    Ok(())
}

fn validate_ai_sdk_config_for_inspection(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read TypeScript provider: {error}"))?;
    if text.contains("export async function call") || text.contains("export function call") {
        Ok(())
    } else {
        Err("missing_ai_sdk_call_export: AI SDK provider must export a call function".to_owned())
    }
}

fn validate_wasm_config_for_inspection(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read WASM provider: {error}"))?;
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config).map_err(|error| format!("wasm_engine_failed: {error}"))?;
    let module = Module::from_binary(&engine, &bytes)
        .map_err(|error| format!("wasm_module_invalid: {error}"))?;
    let exports = module
        .exports()
        .map(|export| (export.name().to_owned(), export.ty()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for name in [
        "memory",
        "rtemplate_input_alloc",
        "rtemplate_input_ptr",
        "rtemplate_call",
        "rtemplate_output_ptr",
        "rtemplate_output_len",
    ] {
        let ty = exports
            .get(name)
            .ok_or_else(|| format!("wasm_export_missing: WASM provider must export `{name}`"))?;
        match (name, ty) {
            ("memory", ExternType::Memory(_)) => {}
            ("memory", _) => {
                return Err("wasm_export_invalid: `memory` must be a memory export".to_owned());
            }
            (_, ExternType::Func(_)) => {}
            _ => {
                return Err(format!(
                    "wasm_export_invalid: `{name}` must be a function export"
                ))
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct FileProviderLoadError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for FileProviderLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for FileProviderLoadError {}

#[derive(Clone)]
struct FileProvider {
    path: PathBuf,
    catalog: ProviderCatalog,
}

fn provider_for_catalog(path: PathBuf, catalog: ProviderCatalog) -> std::sync::Arc<dyn Provider> {
    match catalog.provider.kind {
        ProviderKind::Openapi => OpenApiProvider::arc(catalog),
        ProviderKind::Mcp => McpProvider::arc(catalog),
        ProviderKind::AiSdk => AiSdkProvider::arc(path, catalog),
        ProviderKind::Wasm => WasmProvider::arc(path, catalog),
        ProviderKind::StaticRust => std::sync::Arc::new(FileProvider { path, catalog }),
    }
}

#[async_trait]
impl Provider for FileProvider {
    fn catalog(&self) -> ProviderCatalog {
        self.catalog.clone()
    }

    async fn call(&self, call: ProviderCall) -> Result<ProviderOutput, ProviderError> {
        let tool = self
            .catalog
            .tools
            .iter()
            .find(|tool| tool.name == call.action)
            .ok_or_else(|| {
                ProviderError::validation(
                    &self.catalog.provider.name,
                    &call.action,
                    "unknown_file_provider_action",
                    format!(
                        "provider file `{}` does not expose this action",
                        self.path.display()
                    ),
                )
            })?;

        if let Some(result) = tool.meta.get("result").cloned() {
            return Ok(ProviderOutput::json(result));
        }

        Ok(ProviderOutput::json(json!({
            "kind": "file_provider_result",
            "schema_version": 1,
            "provider": self.catalog.provider.name,
            "provider_kind": self.catalog.provider.kind.as_str(),
            "action": call.action,
            "params": call.params,
            "source": self.path.display().to_string(),
        })))
    }
}

fn is_provider_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("json" | "ts" | "wasm")
    )
}

fn load_catalog(path: &Path) -> Result<ProviderCatalog, FileProviderLoadError> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    let catalog = match extension {
        Some("json") => {
            serde_json::from_slice(&fs::read(path).map_err(|source| FileProviderLoadError {
                path: path.to_path_buf(),
                message: format!("failed to read provider manifest: {source}"),
            })?)
            .map_err(|source| FileProviderLoadError {
                path: path.to_path_buf(),
                message: format!("invalid provider manifest JSON: {source}"),
            })?
        }
        Some("ts") => load_ts_catalog(path)?,
        Some("wasm") => load_wasm_catalog(path)?,
        _ => {
            return Err(FileProviderLoadError {
                path: path.to_path_buf(),
                message: "unsupported provider file extension".to_owned(),
            });
        }
    };
    ensure_kind_matches(path, &catalog)?;
    Ok(catalog)
}

fn load_ts_catalog(path: &Path) -> Result<ProviderCatalog, FileProviderLoadError> {
    let text = fs::read_to_string(path).map_err(|source| FileProviderLoadError {
        path: path.to_path_buf(),
        message: format!("failed to read TypeScript provider: {source}"),
    })?;
    let json_text = extract_ts_manifest(&text).ok_or_else(|| FileProviderLoadError {
        path: path.to_path_buf(),
        message: "TypeScript provider must contain `export default { ... }` manifest JSON"
            .to_owned(),
    })?;
    serde_json::from_str(json_text).map_err(|source| FileProviderLoadError {
        path: path.to_path_buf(),
        message: format!("invalid TypeScript provider manifest JSON: {source}"),
    })
}

fn extract_ts_manifest(text: &str) -> Option<&str> {
    let marker = "export default";
    let start = text.find(marker)? + marker.len();
    let rest = text[start..].trim_start();
    let open = rest.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in rest[open..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let end = open + offset + ch.len_utf8();
                    return Some(rest[..end].trim());
                }
            }
            _ => {}
        }
    }
    None
}

fn load_wasm_catalog(path: &Path) -> Result<ProviderCatalog, FileProviderLoadError> {
    let bytes = fs::read(path).map_err(|source| FileProviderLoadError {
        path: path.to_path_buf(),
        message: format!("failed to read WASM provider: {source}"),
    })?;
    let payload =
        wasm_custom_section(&bytes, "rtemplate.provider").ok_or_else(|| FileProviderLoadError {
            path: path.to_path_buf(),
            message: "WASM provider must contain a `rtemplate.provider` custom section".to_owned(),
        })?;
    serde_json::from_slice(payload).map_err(|source| FileProviderLoadError {
        path: path.to_path_buf(),
        message: format!("invalid WASM provider manifest JSON: {source}"),
    })
}

fn wasm_custom_section<'a>(bytes: &'a [u8], wanted_name: &str) -> Option<&'a [u8]> {
    if bytes.len() < 8 || &bytes[..4] != b"\0asm" || bytes[4..8] != [1, 0, 0, 0] {
        return None;
    }
    let mut offset = 8;
    while offset < bytes.len() {
        let section_id = *bytes.get(offset)?;
        offset += 1;
        let section_len = read_leb_u32(bytes, &mut offset)? as usize;
        let section_end = offset.checked_add(section_len)?;
        if section_end > bytes.len() {
            return None;
        }
        if section_id == 0 {
            let mut cursor = offset;
            let name_len = read_leb_u32(bytes, &mut cursor)? as usize;
            let name_end = cursor.checked_add(name_len)?;
            if name_end <= section_end && &bytes[cursor..name_end] == wanted_name.as_bytes() {
                return Some(&bytes[name_end..section_end]);
            }
        }
        offset = section_end;
    }
    None
}

fn read_leb_u32(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let mut result = 0u32;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(*offset)?;
        *offset += 1;
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift >= 32 {
            return None;
        }
    }
}

fn ensure_kind_matches(
    path: &Path,
    catalog: &ProviderCatalog,
) -> Result<(), FileProviderLoadError> {
    let expected = match path.extension().and_then(|extension| extension.to_str()) {
        Some("ts") => Some(ProviderKind::AiSdk),
        Some("wasm") => Some(ProviderKind::Wasm),
        _ => None,
    };
    if expected.is_some_and(|expected| catalog.provider.kind != expected) {
        return Err(FileProviderLoadError {
            path: path.to_path_buf(),
            message: format!(
                "provider kind `{}` does not match file extension",
                catalog.provider.kind.as_str()
            ),
        });
    }
    Ok(())
}
