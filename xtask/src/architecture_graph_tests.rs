use super::*;

#[test]
fn layer_paths_match_architecture_taxonomy() {
    assert_eq!(Layer::from_path("apps/soma"), Some(Layer::App));
    assert_eq!(
        Layer::from_path("crates/shared/mcp/gateway"),
        Some(Layer::Shared)
    );
    assert_eq!(
        Layer::from_path("crates/soma/runtime"),
        Some(Layer::ProductRuntime)
    );
    assert_eq!(
        Layer::from_path("crates/soma/api"),
        Some(Layer::ProductSurface)
    );
    assert_eq!(Layer::from_path("xtask"), Some(Layer::Legacy));
    assert_eq!(Layer::from_path("apps/web"), None);
}
