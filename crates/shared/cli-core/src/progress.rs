//! Minimal progress helpers for long-running CLI commands.
//!
//! These format progress as plain text lines. Callers decide how and where
//! to write them (typically stderr) and whether the target stream is
//! interactive — see [`crate::terminal`]. On a non-interactive stream
//! (piped output, log capture) prefer [`ProgressReporter::line`] output
//! over carriage-return redraws, since `\r` overwrites are meaningless once
//! captured to a file.

use std::fmt::Write as _;

/// Format a `current/total` counter with an optional label, e.g.
/// `"[3/10] validating providers"`.
pub fn format_counter(current: usize, total: usize, label: &str) -> String {
    let mut out = String::new();
    let _ = write!(out, "[{current}/{total}]");
    if !label.is_empty() {
        let _ = write!(out, " {label}");
    }
    out
}

/// Reports progress either as redrawing carriage-return lines (interactive)
/// or as one line per update (non-interactive), based on a caller-supplied
/// interactivity flag.
#[derive(Debug, Clone, Copy)]
pub struct ProgressReporter {
    interactive: bool,
}

impl ProgressReporter {
    pub fn new(interactive: bool) -> Self {
        Self { interactive }
    }

    /// Render one progress update. Interactive callers write this with no
    /// trailing newline and a leading `\r` so successive calls redraw the
    /// same line; non-interactive callers get a plain, appendable line.
    pub fn render(&self, message: &str) -> String {
        if self.interactive {
            format!("\r{message}")
        } else {
            message.to_string()
        }
    }

    /// Render the final message for a finished operation. Interactive
    /// callers get a trailing newline to leave the redrawn line in place;
    /// non-interactive callers just get the message.
    pub fn render_done(&self, message: &str) -> String {
        if self.interactive {
            format!("\r{message}\n")
        } else {
            message.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_counter_includes_label() {
        assert_eq!(
            format_counter(3, 10, "validating providers"),
            "[3/10] validating providers"
        );
        assert_eq!(format_counter(1, 1, ""), "[1/1]");
    }

    #[test]
    fn interactive_reporter_uses_carriage_return() {
        let reporter = ProgressReporter::new(true);
        assert_eq!(reporter.render("working"), "\rworking");
        assert_eq!(reporter.render_done("done"), "\rdone\n");
    }

    #[test]
    fn non_interactive_reporter_uses_plain_lines() {
        let reporter = ProgressReporter::new(false);
        assert_eq!(reporter.render("working"), "working");
        assert_eq!(reporter.render_done("done"), "done");
    }
}
