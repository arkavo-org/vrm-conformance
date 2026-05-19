//! Sequence-mode `vrm-runner diff` smoke test. Uses solid-color PNG fixtures
//! to exercise the temporal_diff dispatch path end-to-end through the CLI.

use camino::Utf8PathBuf;
use std::process::Command;

fn make_solid_color_dir(
    dir: &std::path::Path,
    prefix: &str,
    n: usize,
    rgb: [u8; 3],
) -> Vec<Utf8PathBuf> {
    (0..n)
        .map(|i| {
            let p = dir.join(format!("{prefix}_{i:04}.png"));
            let img = image::RgbImage::from_fn(64, 64, |_, _| image::Rgb(rgb));
            img.save(&p).unwrap();
            Utf8PathBuf::try_from(p).unwrap()
        })
        .collect()
}

fn write_minimal_plan(dir: &std::path::Path) -> std::path::PathBuf {
    let yaml = r#"
id: smoke_seq_cli
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

fn vrm_runner_bin() -> Utf8PathBuf {
    // Use the test binary lookup convention from CARGO_BIN_EXE_*
    Utf8PathBuf::from(env!("CARGO_BIN_EXE_vrm-runner"))
}

#[test]
fn diff_sequence_mode_passes_on_identical_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let cand_dir = dir.path().join("cand");
    let refr_dir = dir.path().join("refr");
    std::fs::create_dir_all(&cand_dir).unwrap();
    std::fs::create_dir_all(&refr_dir).unwrap();
    make_solid_color_dir(&cand_dir, "f", 3, [128, 128, 128]);
    make_solid_color_dir(&refr_dir, "f", 3, [128, 128, 128]);
    let plan = write_minimal_plan(dir.path());

    let out = Command::new(vrm_runner_bin().as_std_path())
        .args([
            "diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-frames",
            cand_dir.to_str().unwrap(),
            "--reference-frames",
            refr_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("vrm-runner diff failed to spawn");

    assert!(
        out.status.success(),
        "diff failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("not json");
    assert_eq!(parsed["test_id"], "smoke_seq_cli");
    assert_eq!(parsed["temporal_diff"]["passed"], true);
    assert_eq!(parsed["temporal_diff"]["frame_count_match"], true);
}

#[test]
fn diff_sequence_mode_fails_on_length_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let cand_dir = dir.path().join("cand");
    let refr_dir = dir.path().join("refr");
    std::fs::create_dir_all(&cand_dir).unwrap();
    std::fs::create_dir_all(&refr_dir).unwrap();
    make_solid_color_dir(&cand_dir, "f", 2, [128, 128, 128]);
    make_solid_color_dir(&refr_dir, "f", 5, [128, 128, 128]);
    let plan = write_minimal_plan(dir.path());

    let out = Command::new(vrm_runner_bin().as_std_path())
        .args([
            "diff",
            "--plan",
            plan.to_str().unwrap(),
            "--render-frames",
            cand_dir.to_str().unwrap(),
            "--reference-frames",
            refr_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("vrm-runner diff failed to spawn");

    // Length mismatch => runner exits non-zero
    assert!(!out.status.success(), "diff should fail on length mismatch");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("not json");
    assert_eq!(parsed["temporal_diff"]["frame_count_match"], false);
    assert_eq!(parsed["temporal_diff"]["passed"], false);
}
