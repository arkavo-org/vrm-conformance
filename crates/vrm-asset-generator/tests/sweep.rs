use vrm_asset_generator::sweep::mtoon_basic_sweep;

#[test]
fn basic_sweep_yields_expected_count() {
    let assets = mtoon_basic_sweep();
    // Spec axes (handover §5.1): shading_shift (7), shading_toony (6),
    // gi_equalization (4), rim_lighting_mix (5), outline width × mode
    // (3 modes × 4 widths) = 12, render queue offset (3), double_sided (2).
    // Plus the all-defaults baseline. Total target ≈ 40-50.
    assert!(
        assets.len() >= 40 && assets.len() <= 60,
        "sweep should be ~50 assets, got {}",
        assets.len()
    );

    // IDs must be unique.
    let mut ids: Vec<&str> = assets.iter().map(|p| p.id.as_str()).collect();
    ids.sort();
    let len = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), len, "duplicate IDs in sweep");

    // Default baseline is included.
    assert!(assets.iter().any(|p| p.id == "mtoon_default"));
}
