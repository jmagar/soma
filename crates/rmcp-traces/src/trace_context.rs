use std::{error::Error, fmt};

use rmcp::model::Meta;
use serde_json::Value;

pub const TRACEPARENT_KEY: &str = "traceparent";
pub const TRACESTATE_KEY: &str = "tracestate";
pub const BAGGAGE_KEY: &str = "baggage";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceTrust {
    Untrusted,
    Trusted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceLimits {
    pub max_traceparent_len: usize,
    pub max_tracestate_len: usize,
    pub max_baggage_len: usize,
    pub max_baggage_members: usize,
}

impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            max_traceparent_len: 512,
            max_tracestate_len: 512,
            max_baggage_len: 8 * 1024,
            max_baggage_members: 64,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TraceParent {
    raw: String,
    trace_id: String,
    span_id: String,
    sampled: bool,
}

impl TraceParent {
    pub fn parse(value: &str) -> Result<Self, TraceParseError> {
        let limits = TraceLimits::default();
        if value.len() > limits.max_traceparent_len {
            return Err(TraceParseError::ValueTooLong {
                field: TRACEPARENT_KEY,
                actual: value.len(),
                max: limits.max_traceparent_len,
            });
        }
        parse_traceparent(value)
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn span_id(&self) -> &str {
        &self.span_id
    }

    pub fn sampled(&self) -> bool {
        self.sampled
    }

    fn trace_id_short(&self) -> &str {
        &self.trace_id[..8]
    }

    fn span_id_short(&self) -> &str {
        &self.span_id[..8]
    }
}

impl fmt::Debug for TraceParent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceParent")
            .field("trace_id", &self.trace_id_short())
            .field("span_id", &self.span_id_short())
            .field("sampled", &self.sampled)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TraceContext {
    traceparent: TraceParent,
    tracestate: Option<String>,
    baggage: Option<String>,
    trust: TraceTrust,
}

impl TraceContext {
    pub fn from_meta(meta: &Meta, trust: TraceTrust) -> Result<Option<Self>, TraceParseError> {
        Self::from_meta_with_limits(meta, trust, TraceLimits::default())
    }

    pub fn from_meta_with_limits(
        meta: &Meta,
        trust: TraceTrust,
        limits: TraceLimits,
    ) -> Result<Option<Self>, TraceParseError> {
        let Some(traceparent) = optional_meta_str(meta, TRACEPARENT_KEY)? else {
            return Ok(None);
        };
        if traceparent.len() > limits.max_traceparent_len {
            return Err(TraceParseError::ValueTooLong {
                field: TRACEPARENT_KEY,
                actual: traceparent.len(),
                max: limits.max_traceparent_len,
            });
        }
        let traceparent = parse_traceparent(traceparent)?;
        let tracestate =
            bounded_optional_meta_string(meta, TRACESTATE_KEY, limits.max_tracestate_len)?;
        let baggage = bounded_optional_meta_string(meta, BAGGAGE_KEY, limits.max_baggage_len)?;
        validate_baggage_member_count(baggage.as_deref(), limits.max_baggage_members)?;
        Ok(Some(Self {
            traceparent,
            tracestate,
            baggage,
            trust,
        }))
    }

    pub fn traceparent(&self) -> &TraceParent {
        &self.traceparent
    }

    pub fn summary(&self) -> TraceSummary {
        TraceSummary::from_context(self)
    }
}

impl fmt::Debug for TraceContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.summary().fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceSummary {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub sampled: Option<bool>,
    pub trust: TraceTrust,
    pub has_tracestate: bool,
    pub baggage_member_count: usize,
    pub sensitive_baggage_member_count: usize,
    pub invalid: Option<String>,
}

impl TraceSummary {
    pub fn absent() -> Self {
        Self {
            trace_id: None,
            span_id: None,
            sampled: None,
            trust: TraceTrust::Untrusted,
            has_tracestate: false,
            baggage_member_count: 0,
            sensitive_baggage_member_count: 0,
            invalid: None,
        }
    }

    pub fn invalid(error: &TraceParseError) -> Self {
        Self {
            invalid: Some(error.safe_reason()),
            ..Self::absent()
        }
    }

    pub fn from_meta(meta: &Meta, trust: TraceTrust) -> Self {
        Self::from_meta_with_limits(meta, trust, TraceLimits::default())
    }

    pub fn from_meta_with_limits(meta: &Meta, trust: TraceTrust, limits: TraceLimits) -> Self {
        let traceparent = match optional_meta_str(meta, TRACEPARENT_KEY) {
            Ok(Some(value)) => value,
            Ok(None) => return Self::absent(),
            Err(error) => return Self::invalid(&error),
        };
        if traceparent.len() > limits.max_traceparent_len {
            return Self::invalid(&TraceParseError::ValueTooLong {
                field: TRACEPARENT_KEY,
                actual: traceparent.len(),
                max: limits.max_traceparent_len,
            });
        }
        let traceparent = match parse_traceparent(traceparent) {
            Ok(traceparent) => traceparent,
            Err(error) => return Self::invalid(&error),
        };

        let mut summary = Self::from_traceparent(&traceparent, trust);
        match bounded_optional_meta_string(meta, TRACESTATE_KEY, limits.max_tracestate_len) {
            Ok(tracestate) => summary.has_tracestate = tracestate.is_some(),
            Err(error) => {
                summary.invalid.get_or_insert_with(|| error.safe_reason());
            }
        };
        match bounded_optional_meta_string(meta, BAGGAGE_KEY, limits.max_baggage_len) {
            Ok(baggage) => {
                match validate_baggage_member_count(baggage.as_deref(), limits.max_baggage_members)
                {
                    Ok(()) => {
                        let (baggage_member_count, sensitive_baggage_member_count) =
                            summarize_baggage(baggage.as_deref());
                        summary.baggage_member_count = baggage_member_count;
                        summary.sensitive_baggage_member_count = sensitive_baggage_member_count;
                    }
                    Err(error) => {
                        summary.invalid.get_or_insert_with(|| error.safe_reason());
                    }
                }
            }
            Err(error) => {
                summary.invalid.get_or_insert_with(|| error.safe_reason());
            }
        };
        summary
    }

    pub fn from_context(context: &TraceContext) -> Self {
        let (baggage_member_count, sensitive_baggage_member_count) =
            summarize_baggage(context.baggage.as_deref());
        let mut summary = Self::from_traceparent(&context.traceparent, context.trust);
        summary.has_tracestate = context.tracestate.is_some();
        summary.baggage_member_count = baggage_member_count;
        summary.sensitive_baggage_member_count = sensitive_baggage_member_count;
        summary
    }

    fn from_traceparent(traceparent: &TraceParent, trust: TraceTrust) -> Self {
        Self {
            trace_id: Some(traceparent.trace_id_short().to_owned()),
            span_id: Some(traceparent.span_id_short().to_owned()),
            sampled: Some(traceparent.sampled()),
            trust,
            has_tracestate: false,
            baggage_member_count: 0,
            sensitive_baggage_member_count: 0,
            invalid: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceParseError {
    NonStringMeta {
        field: &'static str,
    },
    ValueTooLong {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    TooManyBaggageMembers {
        actual: usize,
        max: usize,
    },
    InvalidTraceParentLength {
        actual: usize,
    },
    InvalidTraceParentFormat,
    UnsupportedVersion,
    InvalidTraceId,
    InvalidSpanId,
    InvalidFlags,
}

impl TraceParseError {
    pub fn safe_reason(&self) -> String {
        match self {
            Self::NonStringMeta { field } => format!("{field} was not a string"),
            Self::ValueTooLong { field, actual, max } => {
                format!("{field} exceeded {max} bytes (actual {actual})")
            }
            Self::TooManyBaggageMembers { actual, max } => {
                format!("baggage exceeded {max} members (actual at least {actual})")
            }
            Self::InvalidTraceParentLength { actual } => {
                format!("traceparent length was {actual}, expected 55")
            }
            Self::InvalidTraceParentFormat => "traceparent format was invalid".to_owned(),
            Self::UnsupportedVersion => "traceparent version was unsupported".to_owned(),
            Self::InvalidTraceId => "traceparent trace id was invalid".to_owned(),
            Self::InvalidSpanId => "traceparent span id was invalid".to_owned(),
            Self::InvalidFlags => "traceparent flags were invalid".to_owned(),
        }
    }
}

impl fmt::Display for TraceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.safe_reason())
    }
}

impl Error for TraceParseError {}

fn optional_meta_str<'a>(
    meta: &'a Meta,
    field: &'static str,
) -> Result<Option<&'a str>, TraceParseError> {
    match meta.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(TraceParseError::NonStringMeta { field }),
    }
}

fn bounded_optional_meta_string(
    meta: &Meta,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, TraceParseError> {
    let Some(value) = optional_meta_str(meta, field)? else {
        return Ok(None);
    };
    if value.len() > max {
        return Err(TraceParseError::ValueTooLong {
            field,
            actual: value.len(),
            max,
        });
    }
    Ok(Some(value.to_owned()))
}

fn parse_traceparent(value: &str) -> Result<TraceParent, TraceParseError> {
    if value.len() < 55 {
        return Err(TraceParseError::InvalidTraceParentLength {
            actual: value.len(),
        });
    }
    let bytes = value.as_bytes();
    if bytes[2] != b'-' || bytes[35] != b'-' || bytes[52] != b'-' {
        return Err(TraceParseError::InvalidTraceParentFormat);
    }
    let version = &value[0..2];
    let trace_id = &value[3..35];
    let span_id = &value[36..52];
    let flags = &value[53..55];
    if !is_lower_hex(version)
        || !is_lower_hex(trace_id)
        || !is_lower_hex(span_id)
        || !is_lower_hex(flags)
    {
        return Err(TraceParseError::InvalidTraceParentFormat);
    }
    if version == "ff" {
        return Err(TraceParseError::UnsupportedVersion);
    }
    if version == "00" && value.len() != 55 {
        return Err(TraceParseError::InvalidTraceParentLength {
            actual: value.len(),
        });
    }
    if version != "00" && value.len() > 55 && bytes[55] != b'-' {
        return Err(TraceParseError::InvalidTraceParentFormat);
    }
    if trace_id.bytes().all(|b| b == b'0') {
        return Err(TraceParseError::InvalidTraceId);
    }
    if span_id.bytes().all(|b| b == b'0') {
        return Err(TraceParseError::InvalidSpanId);
    }
    let flag_byte = u8::from_str_radix(flags, 16).map_err(|_| TraceParseError::InvalidFlags)?;
    Ok(TraceParent {
        raw: value.to_owned(),
        trace_id: trace_id.to_owned(),
        span_id: span_id.to_owned(),
        sampled: flag_byte & 0x01 == 0x01,
    })
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn summarize_baggage(baggage: Option<&str>) -> (usize, usize) {
    let Some(baggage) = baggage else {
        return (0, 0);
    };
    let mut total = 0;
    let mut sensitive = 0;
    for key in baggage_keys(baggage) {
        total += 1;
        if is_sensitive_key(key) {
            sensitive += 1;
        }
    }
    (total, sensitive)
}

fn validate_baggage_member_count(baggage: Option<&str>, max: usize) -> Result<(), TraceParseError> {
    let Some(baggage) = baggage else {
        return Ok(());
    };
    let mut total = 0;
    for _ in baggage_keys(baggage) {
        total += 1;
        if total > max {
            return Err(TraceParseError::TooManyBaggageMembers { actual: total, max });
        }
    }
    Ok(())
}

fn baggage_keys(baggage: &str) -> impl Iterator<Item = &str> {
    baggage.split(',').filter_map(|member| {
        let key = member
            .split_once('=')
            .map(|(key, _)| key)
            .unwrap_or(member)
            .trim();
        (!key.is_empty()).then_some(key)
    })
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "authorization"
            | "cookie"
            | "setcookie"
            | "password"
            | "secret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "apikey"
            | "xapikey"
            | "privatekey"
            | "session"
            | "sessionid"
    )
}
