use vrm_asset_generator::params::{MToonParams, OutlineWidthMode};
use vrm_mock_renderer::render::synthesize_png;

#[test]
fn identical_params_produce_identical_pixels() {
    let p = MToonParams::defaults("a");
    let a = synthesize_png(&p, 64, 64);
    let b = synthesize_png(&p, 64, 64);
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "identical params must yield byte-identical pixels"
    );
}

#[test]
fn different_base_color_changes_avatar_pixels() {
    let mut a = MToonParams::defaults("a");
    a.base_color_factor = [1.0, 0.0, 0.0, 1.0];
    let mut b = MToonParams::defaults("b");
    b.base_color_factor = [0.0, 1.0, 0.0, 1.0];

    let img_a = synthesize_png(&a, 64, 64);
    let img_b = synthesize_png(&b, 64, 64);
    assert_ne!(
        img_a.as_raw(),
        img_b.as_raw(),
        "different base_color should produce different pixels"
    );
}

#[test]
fn background_is_magenta_sentinel() {
    let p = MToonParams::defaults("sentinel");
    let img = synthesize_png(&p, 64, 64);
    let corner = img.get_pixel(0, 0);
    assert_eq!(
        corner.0,
        [255, 0, 255],
        "background must be magenta sentinel for diff-engine bbox detection"
    );
}

#[test]
fn avatar_bbox_is_centered_50_percent() {
    let p = MToonParams::defaults("bbox");
    let img = synthesize_png(&p, 64, 64);
    let center = img.get_pixel(32, 32);
    assert_ne!(
        center.0,
        [255, 0, 255],
        "image center should be inside the avatar (non-magenta)"
    );
    let corner = img.get_pixel(2, 2);
    assert_eq!(corner.0, [255, 0, 255]);
}

#[test]
fn outline_mode_none_omits_ring() {
    let mut p = MToonParams::defaults("no_outline");
    p.outline_width_mode = OutlineWidthMode::None;
    p.outline_color_factor = [1.0, 1.0, 1.0];
    let img = synthesize_png(&p, 64, 64);
    // Avatar bbox runs [16..48] × [16..48] for 64×64 image (50% centered).
    // (16, 32) is the left bbox edge — would be a white outline pixel if drawn.
    let edge = img.get_pixel(16, 32);
    assert_ne!(
        edge.0,
        [255, 255, 255],
        "outline_width_mode::None must not draw a white ring"
    );
}

#[test]
fn outline_mode_world_draws_ring() {
    let mut p = MToonParams::defaults("with_outline");
    p.outline_width_mode = OutlineWidthMode::WorldCoordinates;
    p.outline_color_factor = [1.0, 1.0, 1.0];
    let img = synthesize_png(&p, 64, 64);
    let edge = img.get_pixel(16, 32);
    assert_eq!(
        edge.0,
        [255, 255, 255],
        "outline_width_mode::WorldCoordinates must draw a white ring"
    );
}
