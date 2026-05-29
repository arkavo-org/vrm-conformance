//! cross-variant-diff subcommand: passes (exit 0) when the two renders differ,
//! fails (exit 1) when they are identical. max_ssim is read from the plan.

use std::process::Command;

fn write_plan(dir: &std::path::Path) -> std::path::PathBuf {
    // Build the real plan via the generator's public builder, then serialize
    // it — avoids hand-authoring YAML (ConformanceStatus is an internally
    // tagged enum, and several fields are serde-default). vrm-asset-generator
    // is already a dev-dependency of vrm-runner; serde_yml is a regular dep.
    use vrm_asset_generator::params::MToonParams;
    use vrm_asset_generator::sidecar::build_doublesided_quad_test_plan;
    let params = MToonParams::defaults("doublesided_quad_false");
    let plan = build_doublesided_quad_test_plan(
        &params,
        "doublesided_quad_false.vrm",
        Some("doublesided_quad_true"),
    );
    let yaml = serde_yml::to_string(&plan).unwrap();
    let p = dir.join("plan.test.yaml");
    std::fs::write(&p, yaml).unwrap();
    p
}

fn write_png(path: &std::path::Path, with_square: bool) {
    use image::{Rgb, RgbImage};
    let mut img = RgbImage::new(16, 16);
    for px in img.pixels_mut() {
        *px = Rgb([255, 0, 255]);
    }
    if with_square {
        for y in 4..12 {
            for x in 4..12 {
                img.put_pixel(x, y, Rgb([128, 128, 128]));
            }
        }
    }
    img.save(path).unwrap();
}

#[test]
fn cross_variant_diff_passes_when_renders_differ() {
    let tmp = tempfile::tempdir().unwrap();
    let plan = write_plan(tmp.path());
    let f = tmp.path().join("false.png");
    let t = tmp.path().join("true.png");
    write_png(&f, false); // culled → background
    write_png(&t, true); // shown → quad

    let status = Command::new(env!("CARGO_BIN_EXE_vrm-runner"))
        .args([
            "cross-variant-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-false",
            f.to_str().unwrap(),
            "--render-true",
            t.to_str().unwrap(),
            "--json",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "differing renders should exit 0");
}

#[test]
fn cross_variant_diff_fails_when_renders_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let plan = write_plan(tmp.path());
    let f = tmp.path().join("false.png");
    let t = tmp.path().join("true.png");
    write_png(&f, false);
    write_png(&t, false); // identical → must-differ assertion fails

    let status = Command::new(env!("CARGO_BIN_EXE_vrm-runner"))
        .args([
            "cross-variant-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-false",
            f.to_str().unwrap(),
            "--render-true",
            t.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(!status.success(), "identical renders should exit non-zero");
}

#[test]
fn cross_variant_diff_errors_when_plan_lacks_cross_variant() {
    use vrm_asset_generator::params::MToonParams;
    use vrm_asset_generator::sidecar::build_default_test_plan;

    let tmp = tempfile::tempdir().unwrap();
    // A plain default plan carries NO cross_variant block.
    let params = MToonParams::defaults("plain_no_cv");
    let plan = build_default_test_plan(&params, "plain_no_cv.vrm");
    let plan_path = tmp.path().join("plain.test.yaml");
    std::fs::write(&plan_path, serde_yml::to_string(&plan).unwrap()).unwrap();

    let f = tmp.path().join("false.png");
    let t = tmp.path().join("true.png");
    write_png(&f, false);
    write_png(&t, true);

    let status = Command::new(env!("CARGO_BIN_EXE_vrm-runner"))
        .args([
            "cross-variant-diff",
            "--plan",
            plan_path.to_str().unwrap(),
            "--render-false",
            f.to_str().unwrap(),
            "--render-true",
            t.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "a plan without a cross_variant block must exit non-zero"
    );
}
