#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FindingLevel {
    Ok,
    Warn,
    Fail,
}

struct PatternFinding {
    level: FindingLevel,
    check: &'static str,
    message: String,
}

#[derive(Default)]
pub(super) struct PatternReporter {
    findings: Vec<PatternFinding>,
}

impl PatternReporter {
    pub(super) fn ok(&mut self, check: &'static str, message: impl Into<String>) {
        self.findings.push(PatternFinding {
            level: FindingLevel::Ok,
            check,
            message: message.into(),
        });
    }

    pub(super) fn warn(&mut self, check: &'static str, message: impl Into<String>) {
        self.findings.push(PatternFinding {
            level: FindingLevel::Warn,
            check,
            message: message.into(),
        });
    }

    pub(super) fn fail(&mut self, check: &'static str, message: impl Into<String>) {
        self.findings.push(PatternFinding {
            level: FindingLevel::Fail,
            check,
            message: message.into(),
        });
    }

    pub(super) fn print(&self) {
        for finding in &self.findings {
            let level = match finding.level {
                FindingLevel::Ok => "OK",
                FindingLevel::Warn => "WARN",
                FindingLevel::Fail => "FAIL",
            };
            println!("{level}: {}: {}", finding.check, finding.message);
        }
    }

    pub(super) fn has_failures(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.level == FindingLevel::Fail)
    }

    pub(super) fn has_warnings(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.level == FindingLevel::Warn)
    }
}
