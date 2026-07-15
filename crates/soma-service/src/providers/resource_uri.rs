//! Path-to-URI derivation and matching for structured `providers/resources/`
//! files, per `docs/contracts/drop-in-provider-layout.md`.
//!
//! A file's path relative to `providers/resources/` becomes either a static
//! resource URI or (when it contains a bracket segment) a dynamic resource
//! URI template. Matching a request URI against the set of discovered
//! templates happens in strict precedence order: exact static, exact
//! dynamic (no params), parameterized, catch-all.

use std::collections::BTreeMap;

pub(crate) const RESOURCE_URI_PREFIX: &str = "soma://resources/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathSegment {
    Literal(String),
    Param(String),
    CatchAll(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ResourcePath {
    pub segments: Vec<PathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SegmentShape {
    Literal(String),
    Param,
    CatchAll,
}

impl ResourcePath {
    pub fn is_dynamic(&self) -> bool {
        self.segments
            .iter()
            .any(|segment| !matches!(segment, PathSegment::Literal(_)))
    }

    /// RFC 6570-flavored URI (template) string. Param and catch-all segments
    /// both render as `{name}` — the contract's own examples use plain
    /// `{path}` for a catch-all, not `{+path}` reserved-expansion, so the
    /// string form alone cannot distinguish "one segment" from "the rest of
    /// the path." Matching therefore never re-derives shape from this
    /// string; it always uses the `ResourcePath` that produced it.
    pub fn uri_string(&self) -> String {
        let rendered = self
            .segments
            .iter()
            .map(|segment| match segment {
                PathSegment::Literal(value) => value.clone(),
                PathSegment::Param(name) | PathSegment::CatchAll(name) => format!("{{{name}}}"),
            })
            .collect::<Vec<_>>()
            .join("/");
        format!("{RESOURCE_URI_PREFIX}{rendered}")
    }

    fn shape(&self) -> Vec<SegmentShape> {
        self.segments
            .iter()
            .map(|segment| match segment {
                PathSegment::Literal(value) => SegmentShape::Literal(value.clone()),
                PathSegment::Param(_) => SegmentShape::Param,
                PathSegment::CatchAll(_) => SegmentShape::CatchAll,
            })
            .collect()
    }

    /// Two templates are ambiguous if some request URI could match both —
    /// approximated here as "identical segment shape, ignoring param
    /// names," matching the contract's "ambiguous templates at the same
    /// precedence level MUST make validation fail."
    pub fn is_ambiguous_with(&self, other: &ResourcePath) -> bool {
        self.shape() == other.shape()
    }

    /// Attempts to match `request_segments` (the request URI's path split
    /// on `/`, already stripped of the `soma://resources/` prefix) against
    /// this template, returning captured params on success.
    pub fn match_segments(&self, request_segments: &[&str]) -> Option<BTreeMap<String, String>> {
        let mut params = BTreeMap::new();
        let has_catch_all = matches!(self.segments.last(), Some(PathSegment::CatchAll(_)));

        if has_catch_all {
            let fixed = &self.segments[..self.segments.len() - 1];
            if request_segments.len() < self.segments.len() {
                return None;
            }
            for (segment, actual) in fixed.iter().zip(request_segments.iter()) {
                match segment {
                    PathSegment::Literal(value) if value == actual => {}
                    PathSegment::Literal(_) => return None,
                    PathSegment::Param(name) => {
                        params.insert(name.clone(), (*actual).to_owned());
                    }
                    PathSegment::CatchAll(_) => unreachable!("catch-all is only ever last"),
                }
            }
            let PathSegment::CatchAll(name) = self.segments.last()? else {
                unreachable!("checked above");
            };
            let rest = request_segments[fixed.len()..].join("/");
            params.insert(name.clone(), rest);
            return Some(params);
        }

        if self.segments.len() != request_segments.len() {
            return None;
        }
        for (segment, actual) in self.segments.iter().zip(request_segments.iter()) {
            match segment {
                PathSegment::Literal(value) if value == actual => {}
                PathSegment::Literal(_) => return None,
                PathSegment::Param(name) => {
                    params.insert(name.clone(), (*actual).to_owned());
                }
                PathSegment::CatchAll(_) => unreachable!("checked has_catch_all above"),
            }
        }
        Some(params)
    }
}

/// Parameter names must match `^[A-Za-z_][A-Za-z0-9_]*$`.
fn is_valid_param_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Lowercases, collapses runs of non-alphanumerics to a single hyphen, and
/// trims leading/trailing hyphens. Shared by prompt-name derivation
/// (`filesystem::prompt_name_from_file_stem`) and static literal path
/// segments here — "the same separator rules as prompt names" per the
/// layout contract.
pub(crate) fn slugify(input: &str) -> String {
    let mut output = String::new();
    let mut previous_separator = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
            previous_separator = false;
        } else if !previous_separator && !output.is_empty() {
            output.push('-');
            previous_separator = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    output
}

#[derive(Debug)]
pub(crate) struct SegmentParseError(pub String);

/// Parses one path segment (a directory name, or a file stem with its
/// extension already stripped) into a `PathSegment`. A segment wrapped in
/// `[...]` is dynamic: `[...name]` is a catch-all, `[name]` is a single
/// parameter. Anything else is a literal, slugified the same way a prompt
/// name is.
pub(crate) fn parse_path_segment(raw: &str) -> Result<PathSegment, SegmentParseError> {
    if let Some(inner) = raw.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        if let Some(name) = inner.strip_prefix("...") {
            if !is_valid_param_name(name) {
                return Err(SegmentParseError(format!(
                    "invalid catch-all parameter name `{name}` in `{raw}`"
                )));
            }
            return Ok(PathSegment::CatchAll(name.to_owned()));
        }
        if !is_valid_param_name(inner) {
            return Err(SegmentParseError(format!(
                "invalid parameter name `{inner}` in `{raw}`"
            )));
        }
        return Ok(PathSegment::Param(inner.to_owned()));
    }
    let slug = slugify(raw);
    Ok(PathSegment::Literal(if slug.is_empty() {
        "segment".to_owned()
    } else {
        slug
    }))
}

/// Parses every segment of a resource file's path (relative to
/// `providers/resources/`, extension already stripped from the final
/// segment) into a `ResourcePath`, enforcing that at most one catch-all
/// segment appears and that it is the final segment.
pub(crate) fn parse_resource_path(segments: &[&str]) -> Result<ResourcePath, SegmentParseError> {
    let mut parsed = Vec::with_capacity(segments.len());
    for (index, raw) in segments.iter().enumerate() {
        let segment = parse_path_segment(raw)?;
        if matches!(segment, PathSegment::CatchAll(_)) && index != segments.len() - 1 {
            return Err(SegmentParseError(format!(
                "catch-all segment `{raw}` must be the final path segment"
            )));
        }
        parsed.push(segment);
    }
    Ok(ResourcePath { segments: parsed })
}

/// Splits a request resource URI into path segments for matching, stripping
/// the `soma://resources/` prefix and any `?query` suffix. Returns `None`
/// if the URI doesn't use the resource scheme at all.
pub(crate) fn request_segments(uri: &str) -> Option<Vec<&str>> {
    let rest = uri.strip_prefix(RESOURCE_URI_PREFIX)?;
    let path = rest.split('?').next().unwrap_or(rest);
    if path.is_empty() {
        return Some(Vec::new());
    }
    Some(path.split('/').collect())
}

/// Percent-decoded `?key=value&...` pairs from a resource URI's query
/// string, if any — passed to dynamic resource readers as `input.query`.
pub(crate) fn query_params(uri: &str) -> BTreeMap<String, String> {
    let Some((_, query)) = uri.split_once('?') else {
        return BTreeMap::new();
    };
    url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
}

#[cfg(test)]
#[path = "resource_uri_tests.rs"]
mod tests;
