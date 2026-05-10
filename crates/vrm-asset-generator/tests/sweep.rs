use vrm_asset_generator::params::OutlineWidthMode;
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

#[test]
fn sweep_assets_have_correct_param_values() {
    let assets = mtoon_basic_sweep();
    let find = |id: &str| {
        assets
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("missing asset: {id}"))
    };

    assert_eq!(find("mtoon_default").shading_shift_factor, 0.0);
    assert_eq!(find("mtoon_shadingShift_neg0p5").shading_shift_factor, -0.5);
    assert_eq!(find("mtoon_shadingShift_1").shading_shift_factor, 1.0);
    assert_eq!(find("mtoon_shadingToony_0p25").shading_toony_factor, 0.25);
    assert_eq!(find("mtoon_giEqualization_0p5").gi_equalization_factor, 0.5);
    assert_eq!(
        find("mtoon_rimLightingMix_0p5").rim_lighting_mix_factor,
        0.5
    );
    assert_eq!(
        find("mtoon_outline_none").outline_width_mode,
        OutlineWidthMode::None
    );
    assert_eq!(find("mtoon_outline_world_0p05").outline_width_factor, 0.05);
    assert_eq!(
        find("mtoon_renderQueueOffset_-9").render_queue_offset_number,
        -9
    );
    assert!(find("mtoon_doubleSided_true").double_sided);
}
