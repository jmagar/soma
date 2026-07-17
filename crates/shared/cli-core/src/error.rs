//! Reusable CLI error presentation.
//!
//! Formats a message plus an optional remediation hint for either a human
//! terminal or a `--json` consumer. This is presentation only — it does not
//! define error *codes*, retryability, or product error taxonomy; those are
//! product/application concerns.

use serde::Serialize;

/// A presentable CLI error: a message and an optional hint for how to fix
/// it.
#[derive(Debug, Clone, Serialize)]
pub struct CliErrorReport {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl CliErrorReport {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Render as a human-readable line, e.g. `"error: bad thing (try: fix
    /// it)"`.
    pub fn render_human(&self) -> String {
        match &self.hint {
            Some(hint) => format!("error: {} (try: {hint})", self.message),
            None => format!("error: {}", self.message),
        }
    }

    /// Render as pretty-printed JSON.
    pub fn render_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_render_includes_hint_when_present() {
        let report = CliErrorReport::new("bad thing").with_hint("fix it");
        assert_eq!(report.render_human(), "error: bad thing (try: fix it)");
    }

    #[test]
    fn human_render_omits_hint_when_absent() {
        let report = CliErrorReport::new("bad thing");
        assert_eq!(report.render_human(), "error: bad thing");
    }

    #[test]
    fn json_render_skips_missing_hint() {
        let report = CliErrorReport::new("bad thing");
        let json = report.render_json().unwrap();
        assert!(!json.contains("hint"));
    }
}
