use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::upstream::{PromptDescriptor, ResourceDescriptor, ToolDescriptor, UpstreamHealth};

use super::{GatewayManager, GatewayManagerError};

const UPSTREAM_RESOURCE_PREFIX: &str = "soma://upstream/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayToolRoute {
    pub name: String,
    pub upstream: String,
    pub native_name: String,
    pub descriptor: ToolDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayResourceRoute {
    pub uri: String,
    pub upstream: String,
    pub native_uri: String,
    pub descriptor: ResourceDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayPromptRoute {
    pub name: String,
    pub upstream: String,
    pub native_name: String,
    pub descriptor: PromptDescriptor,
}

impl GatewayManager {
    pub async fn tool_routes(&self) -> Result<Vec<GatewayToolRoute>, GatewayManagerError> {
        self.tool_routes_for_subject(None).await
    }

    pub async fn tool_routes_for_subject(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<GatewayToolRoute>, GatewayManagerError> {
        let pool = self.ready_pool()?;
        let snapshots = discover_snapshots(&pool, subject).await?;
        let mut candidates = Vec::new();
        for snapshot in snapshots {
            if !matches!(snapshot.health, UpstreamHealth::Connected) {
                continue;
            }
            for descriptor in exposed_tools(&pool, &snapshot.name, subject).await? {
                candidates.push((snapshot.name.clone(), descriptor));
            }
        }
        Ok(tool_routes_from_candidates(candidates))
    }

    pub async fn call_mcp_tool(
        &self,
        name: &str,
        params: Value,
    ) -> Result<Option<Value>, GatewayManagerError> {
        self.call_mcp_tool_for_subject(name, params, None).await
    }

    pub async fn call_mcp_tool_for_subject(
        &self,
        name: &str,
        params: Value,
        subject: Option<&str>,
    ) -> Result<Option<Value>, GatewayManagerError> {
        let Some(route) = self
            .tool_routes_for_subject(subject)
            .await?
            .into_iter()
            .find(|route| route.name == name)
        else {
            return Ok(None);
        };
        let pool = self.ready_pool()?;
        call_tool(&pool, route, params, subject)
            .await
            .map(Some)
            .map_err(Into::into)
    }

    pub async fn resource_routes(&self) -> Result<Vec<GatewayResourceRoute>, GatewayManagerError> {
        self.resource_routes_for_subject(None).await
    }

    pub async fn resource_routes_for_subject(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<GatewayResourceRoute>, GatewayManagerError> {
        let pool = self.ready_pool()?;
        let snapshots = discover_snapshots(&pool, subject).await?;
        let mut routes = Vec::new();
        for snapshot in snapshots {
            if !matches!(snapshot.health, UpstreamHealth::Connected) {
                continue;
            }
            for descriptor in list_resources(&pool, &snapshot.name, subject).await? {
                routes.push(GatewayResourceRoute {
                    uri: upstream_resource_uri(&snapshot.name, &descriptor.uri),
                    upstream: snapshot.name.clone(),
                    native_uri: descriptor.uri.clone(),
                    descriptor,
                });
            }
        }
        Ok(routes)
    }

    pub async fn read_mcp_resource(&self, uri: &str) -> Result<Option<Value>, GatewayManagerError> {
        self.read_mcp_resource_for_subject(uri, None).await
    }

    pub async fn read_mcp_resource_for_subject(
        &self,
        uri: &str,
        subject: Option<&str>,
    ) -> Result<Option<Value>, GatewayManagerError> {
        let Some((upstream, native_uri)) = parse_upstream_resource_uri(uri) else {
            return Ok(None);
        };
        let pool = self.ready_pool()?;
        read_resource(&pool, &upstream, &native_uri, subject)
            .await
            .map(Some)
            .map_err(Into::into)
    }

    pub async fn prompt_routes(&self) -> Result<Vec<GatewayPromptRoute>, GatewayManagerError> {
        self.prompt_routes_for_subject(None).await
    }

    pub async fn prompt_routes_for_subject(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<GatewayPromptRoute>, GatewayManagerError> {
        let pool = self.ready_pool()?;
        let snapshots = discover_snapshots(&pool, subject).await?;
        let mut candidates = Vec::new();
        for snapshot in snapshots {
            if !matches!(snapshot.health, UpstreamHealth::Connected) {
                continue;
            }
            for descriptor in list_prompts(&pool, &snapshot.name, subject).await? {
                candidates.push((snapshot.name.clone(), descriptor));
            }
        }
        Ok(prompt_routes_from_candidates(candidates))
    }

    pub async fn get_mcp_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, Value>>,
    ) -> Result<Option<Value>, GatewayManagerError> {
        self.get_mcp_prompt_for_subject(name, arguments, None).await
    }

    pub async fn get_mcp_prompt_for_subject(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, Value>>,
        subject: Option<&str>,
    ) -> Result<Option<Value>, GatewayManagerError> {
        let Some(route) = self
            .prompt_routes_for_subject(subject)
            .await?
            .into_iter()
            .find(|route| route.name == name)
        else {
            return Ok(None);
        };
        let pool = self.ready_pool()?;
        get_prompt(
            &pool,
            &route.upstream,
            &route.native_name,
            arguments,
            subject,
        )
        .await
        .map(Some)
        .map_err(Into::into)
    }

    fn ready_pool(
        &self,
    ) -> Result<std::sync::Arc<crate::upstream::pool::UpstreamPool>, GatewayManagerError> {
        self.ensure_ready()?;
        Ok(self.pool.read().expect("gateway pool poisoned").clone())
    }
}

async fn discover_snapshots(
    pool: &crate::upstream::pool::UpstreamPool,
    subject: Option<&str>,
) -> Result<Vec<crate::upstream::UpstreamSnapshot>, crate::upstream::UpstreamError> {
    let _ = subject;
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool.discover_for_subject(subject).await;
    }
    pool.discover().await
}

async fn exposed_tools(
    pool: &crate::upstream::pool::UpstreamPool,
    upstream: &str,
    subject: Option<&str>,
) -> Result<Vec<ToolDescriptor>, crate::upstream::UpstreamError> {
    let _ = subject;
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool.exposed_tools_for_subject(upstream, subject).await;
    }
    pool.exposed_tools(upstream)
}

async fn call_tool(
    pool: &crate::upstream::pool::UpstreamPool,
    route: GatewayToolRoute,
    params: Value,
    subject: Option<&str>,
) -> Result<Value, crate::upstream::UpstreamError> {
    let _ = subject;
    let call = crate::upstream::pool::ToolCall {
        upstream: route.upstream,
        tool: route.native_name,
        params,
    };
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool.call_tool_for_subject(call, subject).await;
    }
    pool.call_tool(call).await
}

async fn list_resources(
    pool: &crate::upstream::pool::UpstreamPool,
    upstream: &str,
    subject: Option<&str>,
) -> Result<Vec<ResourceDescriptor>, crate::upstream::UpstreamError> {
    let _ = subject;
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool.list_resources_for_subject(upstream, subject).await;
    }
    pool.list_resources(upstream).await
}

async fn read_resource(
    pool: &crate::upstream::pool::UpstreamPool,
    upstream: &str,
    uri: &str,
    subject: Option<&str>,
) -> Result<Value, crate::upstream::UpstreamError> {
    let _ = subject;
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool.read_resource_for_subject(upstream, uri, subject).await;
    }
    pool.read_resource(upstream, uri).await
}

async fn list_prompts(
    pool: &crate::upstream::pool::UpstreamPool,
    upstream: &str,
    subject: Option<&str>,
) -> Result<Vec<PromptDescriptor>, crate::upstream::UpstreamError> {
    let _ = subject;
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool.list_prompts_for_subject(upstream, subject).await;
    }
    pool.list_prompts(upstream).await
}

async fn get_prompt(
    pool: &crate::upstream::pool::UpstreamPool,
    upstream: &str,
    name: &str,
    arguments: Option<serde_json::Map<String, Value>>,
    subject: Option<&str>,
) -> Result<Value, crate::upstream::UpstreamError> {
    let _ = subject;
    #[cfg(feature = "oauth")]
    if subject.is_some() {
        return pool
            .get_prompt_for_subject(upstream, name, arguments, subject)
            .await;
    }
    pool.get_prompt(upstream, name, arguments).await
}

fn tool_routes_from_candidates(candidates: Vec<(String, ToolDescriptor)>) -> Vec<GatewayToolRoute> {
    let counts = name_counts(candidates.iter().map(|(_, descriptor)| &descriptor.name));
    let mut used = BTreeSet::new();
    candidates
        .into_iter()
        .map(|(upstream, descriptor)| {
            let native_name = descriptor.name.clone();
            let preferred = if counts.get(native_name.as_str()) == Some(&1) && native_name != "soma"
            {
                native_name.clone()
            } else {
                format!(
                    "{}__{}",
                    route_segment(&upstream),
                    route_segment(&native_name)
                )
            };
            GatewayToolRoute {
                name: unique_route_name(preferred, &mut used),
                upstream,
                native_name,
                descriptor,
            }
        })
        .collect()
}

fn prompt_routes_from_candidates(
    candidates: Vec<(String, PromptDescriptor)>,
) -> Vec<GatewayPromptRoute> {
    let counts = name_counts(candidates.iter().map(|(_, descriptor)| &descriptor.name));
    let mut used = BTreeSet::new();
    candidates
        .into_iter()
        .map(|(upstream, descriptor)| {
            let native_name = descriptor.name.clone();
            let preferred = if counts.get(native_name.as_str()) == Some(&1) {
                native_name.clone()
            } else {
                format!(
                    "{}__{}",
                    route_segment(&upstream),
                    route_segment(&native_name)
                )
            };
            GatewayPromptRoute {
                name: unique_route_name(preferred, &mut used),
                upstream,
                native_name,
                descriptor,
            }
        })
        .collect()
}

fn name_counts<'a>(names: impl Iterator<Item = &'a String>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for name in names {
        *counts.entry(name.clone()).or_insert(0) += 1;
    }
    counts
}

fn route_segment(value: &str) -> String {
    let routed: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if routed.is_empty() {
        "route".to_owned()
    } else {
        routed
    }
}

fn unique_route_name(preferred: String, used: &mut BTreeSet<String>) -> String {
    if used.insert(preferred.clone()) {
        return preferred;
    }
    for index in 2usize.. {
        let candidate = format!("{preferred}_{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("unbounded route suffix loop should always return")
}

pub fn upstream_resource_uri(upstream: &str, native_uri: &str) -> String {
    format!(
        "{UPSTREAM_RESOURCE_PREFIX}{upstream}/{}",
        percent_encode(native_uri.as_bytes())
    )
}

pub fn parse_upstream_resource_uri(uri: &str) -> Option<(String, String)> {
    let rest = uri.strip_prefix(UPSTREAM_RESOURCE_PREFIX)?;
    let (upstream, encoded) = rest.split_once('/')?;
    let native = percent_decode(encoded).ok()?;
    Some((upstream.to_owned(), native))
}

fn percent_decode(value: &str) -> Result<String, ()> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hi = bytes.get(index + 1).copied().ok_or(())?;
            let lo = bytes.get(index + 2).copied().ok_or(())?;
            decoded.push(from_hex(hi)? << 4 | from_hex(lo)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| ())
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut encoded = String::new();
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn from_hex(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
#[path = "mcp_routes_tests.rs"]
mod tests;
