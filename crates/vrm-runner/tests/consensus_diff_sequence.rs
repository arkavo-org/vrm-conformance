//! Integration test for `vrm-runner consensus-diff --render-frames`.
//! Uses solid-color and checkerboard PNG fixtures to exercise the sequence
//! consensus path end-to-end through the CLI binary.

use camino::Utf8PathBuf;
use std::process::Command;

fn vrm_runner_bin() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_BIN_EXE_vrm-runner"))
}

fn make_solid_color_dir(dir: &std::path::Path, n: usize, rgb: [u8; 3]) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    for i in 0..n {
        let p = dir.join(format!("f_{i:04}.png"));
        image::RgbImage::from_fn(64, 64, |_, _| image::Rgb(rgb))
            .save(&p)
            .unwrap();
    }
    dir.to_path_buf()
}

fn make_checkerboard_dir(dir: &std::path::Path, n: usize) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    for i in 0..n {
        let p = dir.join(format!("f_{i:04}.png"));
        image::RgbImage::from_fn(64, 64, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgb([0u8, 0, 0])
            } else {
                image::Rgb([255u8, 255, 255])
            }
        })
        .save(&p)
        .unwrap();
    }
    dir.to_path_buf()
}

fn write_minimal_plan(dir: &std::path::Path) -> std::path::PathBuf {
    let yaml = r#"
id: smoke_consensus_seq
spec_section: x
asset: x.vrm
camera:
  position: [0.0, 1.2, 1.5]
  target: [0.0, 1.0, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.3
output:
  width: 64
  height: 64
  color_space: linear
  msaa: 1
diff:
  mode: ssim
  threshold: 0.5
  reference_renderer: vrm-metal-kit
render_sequence:
  frame_count: 3
  frame_hz: 30.0
  physics_dt_seconds: 0.01666
  output_format: png_sequence
  temporal_ssim_threshold: 0.5
"#;
    let p = dir.join("plan.test.yaml");
    std::fs::write(&p, yaml).unwrap();
    p
}

#[test]
fn consensus_diff_sequence_three_identical_passes() {
    let dir = tempfile::tempdir().unwrap();
    let a_dir = make_solid_color_dir(&dir.path().join("a"), 3, [100, 100, 100]);
    let b_dir = make_solid_color_dir(&dir.path().join("b"), 3, [100, 100, 100]);
    let c_dir = make_solid_color_dir(&dir.path().join("c"), 3, [100, 100, 100]);
    let plan = write_minimal_plan(dir.path());

    let out = Command::new(vrm_runner_bin().as_std_path())
        .args([
            "consensus-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-frames",
            &format!("a={}", a_dir.display()),
            "--render-frames",
            &format!("b={}", b_dir.display()),
            "--render-frames",
            &format!("c={}", c_dir.display()),
            "--json",
        ])
        .output()
        .expect("vrm-runner consensus-diff failed to spawn");

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout not valid JSON");
    assert_eq!(parsed["consensus_passed"], true);
    assert_eq!(parsed["renderers"].as_array().unwrap().len(), 3);
    assert_eq!(parsed["frame_counts"], serde_json::json!([3, 3, 3]));
    assert!(parsed["outliers"].as_array().unwrap().is_empty());
}

#[test]
fn consensus_diff_sequence_outlier_detected_fails() {
    // a and b are structurally identical (solid grey). c is a checkerboard
    // (SSIM vs solid is well below 0.50 threshold from the plan).
    let dir = tempfile::tempdir().unwrap();
    let a_dir = make_solid_color_dir(&dir.path().join("a"), 3, [100, 100, 100]);
    let b_dir = make_solid_color_dir(&dir.path().join("b"), 3, [100, 100, 100]);
    let c_dir = make_checkerboard_dir(&dir.path().join("c"), 3);
    let plan = write_minimal_plan(dir.path());

    let out = Command::new(vrm_runner_bin().as_std_path())
        .args([
            "consensus-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-frames",
            &format!("a={}", a_dir.display()),
            "--render-frames",
            &format!("b={}", b_dir.display()),
            "--render-frames",
            &format!("c={}", c_dir.display()),
            "--json",
        ])
        .output()
        .expect("vrm-runner consensus-diff failed to spawn");

    // Outlier detected → exit non-zero
    assert!(
        !out.status.success(),
        "expected non-zero exit for outlier, got success"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout not valid JSON");
    assert_eq!(parsed["consensus_passed"], false);
    let outliers = parsed["outliers"].as_array().unwrap();
    assert!(
        outliers.iter().any(|v| v.as_str() == Some("c")),
        "expected 'c' in outliers, got: {outliers:?}"
    );
}

#[test]
fn consensus_diff_sequence_length_mismatch_fails() {
    let dir = tempfile::tempdir().unwrap();
    let a_dir = make_solid_color_dir(&dir.path().join("a"), 3, [100, 100, 100]);
    let b_dir = make_solid_color_dir(&dir.path().join("b"), 3, [100, 100, 100]);
    // c has 5 frames — length mismatch vs a and b (3 frames each).
    let c_dir = make_solid_color_dir(&dir.path().join("c"), 5, [100, 100, 100]);
    let plan = write_minimal_plan(dir.path());

    let out = Command::new(vrm_runner_bin().as_std_path())
        .args([
            "consensus-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-frames",
            &format!("a={}", a_dir.display()),
            "--render-frames",
            &format!("b={}", b_dir.display()),
            "--render-frames",
            &format!("c={}", c_dir.display()),
            "--json",
        ])
        .output()
        .expect("vrm-runner consensus-diff failed to spawn");

    assert!(
        !out.status.success(),
        "expected non-zero exit for length-mismatch outlier"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout not valid JSON");
    assert_eq!(parsed["consensus_passed"], false);
    assert_eq!(parsed["frame_counts"], serde_json::json!([3, 3, 5]));
}

#[test]
fn consensus_diff_sequence_and_render_flags_mutually_exclusive() {
    let dir = tempfile::tempdir().unwrap();
    let img_path = dir.path().join("x.png");
    image::RgbImage::from_fn(64, 64, |_, _| image::Rgb([0u8, 0, 0]))
        .save(&img_path)
        .unwrap();
    let frame_dir = make_solid_color_dir(&dir.path().join("fd"), 2, [0, 0, 0]);
    let plan = write_minimal_plan(dir.path());

    let out = Command::new(vrm_runner_bin().as_std_path())
        .args([
            "consensus-diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render",
            &format!("a={}", img_path.display()),
            "--render-frames",
            &format!("b={}", frame_dir.display()),
        ])
        .output()
        .expect("vrm-runner consensus-diff failed to spawn");

    assert!(
        !out.status.success(),
        "expected failure when both --render and --render-frames are supplied"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("mutually exclusive"),
        "expected 'mutually exclusive' in stderr, got: {stderr}"
    );
}
