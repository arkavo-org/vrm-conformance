use vrm_diff_engine::ssim::ssim_pngs;

fn make_solid_color(w: u32, h: u32, rgb: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |_, _| image::Rgb(rgb))
}

#[test]
fn identical_images_score_one() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_solid_color(64, 64, [128, 64, 200]).save(&a).unwrap();
    make_solid_color(64, 64, [128, 64, 200]).save(&b).unwrap();

    let score = ssim_pngs(
        camino::Utf8Path::from_path(&a).unwrap(),
        camino::Utf8Path::from_path(&b).unwrap(),
    )
    .unwrap();
    assert!(
        (score - 1.0).abs() < 1e-6,
        "identical → SSIM ~ 1, got {score}"
    );
}

#[test]
fn very_different_images_score_low() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_solid_color(64, 64, [0, 0, 0]).save(&a).unwrap();
    make_solid_color(64, 64, [255, 255, 255]).save(&b).unwrap();

    let score = ssim_pngs(
        camino::Utf8Path::from_path(&a).unwrap(),
        camino::Utf8Path::from_path(&b).unwrap(),
    )
    .unwrap();
    assert!(
        score < 0.5,
        "black vs white → SSIM should be low, got {score}"
    );
}

#[test]
fn dimension_mismatch_errors() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_solid_color(32, 32, [0, 0, 0]).save(&a).unwrap();
    make_solid_color(64, 64, [0, 0, 0]).save(&b).unwrap();

    let err = ssim_pngs(
        camino::Utf8Path::from_path(&a).unwrap(),
        camino::Utf8Path::from_path(&b).unwrap(),
    )
    .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("dimension"));
}
