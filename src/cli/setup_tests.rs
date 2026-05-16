use super::{SetupCommand, SetupReport};

// ── SetupReport state machine ─────────────────────────────────────────────────

#[test]
fn new_report_has_no_failures() {
    let report = SetupReport::new(false);
    assert!(report.blocking_failures.is_empty());
    assert!(report.advisory_failures.is_empty());
    assert!(!report.ran_repair);
}

#[test]
fn finish_sets_success_when_no_failures() {
    let report = SetupReport::new(false).finish();
    assert_eq!(report.exit_policy, "success");
}

#[test]
fn finish_sets_blocking_failure_when_blocking_present() {
    let mut report = SetupReport::new(false);
    report.blocking_failures.push(super::SetupFailure {
        code: "test_code",
        message: "test message".into(),
    });
    let report = report.finish();
    assert_eq!(report.exit_policy, "blocking_failure");
}

#[test]
fn finish_sets_advisory_failure_when_only_advisory_present() {
    let mut report = SetupReport::new(false);
    report.advisory_failures.push(super::SetupFailure {
        code: "test_advisory",
        message: "advisory message".into(),
    });
    let report = report.finish();
    assert_eq!(report.exit_policy, "advisory_failure");
}

#[test]
fn finish_prefers_blocking_over_advisory() {
    let mut report = SetupReport::new(false);
    report.blocking_failures.push(super::SetupFailure {
        code: "b",
        message: "blocking".into(),
    });
    report.advisory_failures.push(super::SetupFailure {
        code: "a",
        message: "advisory".into(),
    });
    let report = report.finish();
    assert_eq!(report.exit_policy, "blocking_failure");
}

// ── SetupCommand enum ─────────────────────────────────────────────────────────

#[test]
fn setup_command_copy() {
    let cmd = SetupCommand::Check;
    let _copy = cmd;
    let _again = cmd;
}

#[test]
fn all_variants_are_distinct() {
    assert_ne!(SetupCommand::Check, SetupCommand::Repair);
    assert_ne!(
        SetupCommand::Check,
        SetupCommand::PluginHook { no_repair: false }
    );
    assert_ne!(
        SetupCommand::Repair,
        SetupCommand::PluginHook { no_repair: false }
    );
    assert_ne!(
        SetupCommand::PluginHook { no_repair: false },
        SetupCommand::PluginHook { no_repair: true }
    );
}
