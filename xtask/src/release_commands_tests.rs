use super::*;

#[test]
fn release_options_default_to_pr_mode_head() {
    let options = ReleaseCommandOptions::parse(&[]).unwrap();
    assert!(options.base.is_none());
    assert_eq!(options.head, "HEAD");
    assert_eq!(options.mode, release_versions::GateMode::Pr);
    assert!(!options.json);
}

#[test]
fn bump_levels_parse_known_values() {
    assert_eq!(
        parse_bump_level("patch").unwrap(),
        release_versions::BumpLevel::Patch
    );
    assert!(parse_bump_level("unknown").is_err());
}
