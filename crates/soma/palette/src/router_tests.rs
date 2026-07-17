use super::router;

#[test]
fn router_registers_the_four_palette_routes() {
    // Building the router only registers routes; it does not require a live
    // `PaletteState` (that's only needed once `.with_state()` is called by
    // the composing app). This is a smoke test that route registration
    // itself does not panic and produces a router generic over
    // `PaletteState`, matching what apps/soma will mount.
    let _: axum::Router<crate::state::PaletteState> = router();
}
