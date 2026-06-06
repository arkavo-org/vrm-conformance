//! End-to-end smoke for phase 1 dump_bone_positions infrastructure:
//!   - spawn mock renderer
//!   - execute a minimal plan with --reference-positions pointed at a
//!     1-chain reference JSON matching the mock's synthetic output
//!   - assert overall_passed = true, position_diff = Some(passed: true)
//!
//! The mock renderer returns a deterministic synthetic chain ("mock_hair",
//! 4 joints). The reference fixture mirrors that chain exactly, so drift
//! is 0.0 on every metric. This is a wiring test, not a math test.

use camino::Utf8PathBuf;
use std::fs;
use vrm_runner::execute::{execute_plan, load_plan, ExecuteOptions};

/// Resolve the workspace target dir, then the mock binary. Builds the
/// mock-renderer if it isn't already present — `cargo test --workspace`
/// builds it ahead of time, but `cargo test -p vrm-runner` doesn't.
fn mock_bin() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let target_root = std::env::var("CARGO_TARGET_DIR")
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));
    let exe_name = if cfg!(windows) {
        "vrm-mock-renderer.exe"
    } else {
        "vrm-mock-renderer"
    };
    let bin = target_root.join("debug").join(exe_name);
    if !bin.exists() {
        let status = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "--bin", "vrm-mock-renderer"])
            .status()
            .expect("invoke cargo build for vrm-mock-renderer");
        assert!(
            status.success(),
            "cargo build vrm-mock-renderer must succeed"
        );
    }
    bin
}

#[test]
fn execute_plan_with_reference_positions_against_mock_passes() {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("crate dir has parent")
        .parent()
        .expect("crates dir has parent")
        .to_path_buf();
    let plan_path = repo_root.join("assets/generated/smoke_default.test.yaml");
    let asset_dir = repo_root.join("assets/generated");
    let plan = load_plan(&plan_path).expect("load smoke_default plan");

    let tmp = tempfile::tempdir().expect("tempdir");
    let ref_path =
        Utf8PathBuf::from_path_buf(tmp.path().join("ref_positions.json")).expect("utf8 path");
    // Reference mirrors the mock's deterministic synthetic chain exactly,
    // so mock-vs-reference drift is 0.0 on all metrics.
    fs::write(
        &ref_path,
        r#"{"springs":[{"name":"mock_hair","joint_positions":[[0.0,1.50,0.0],[0.0,1.45,0.0],[0.0,1.40,0.0],[0.0,1.35,0.0]]}]}"#,
    )
    .expect("write reference positions");

    let output_dir = Utf8PathBuf::from_path_buf(tmp.path().join("out")).expect("utf8 path");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let opts = ExecuteOptions {
        adapter_bin: mock_bin(),
        adapter_args: vec![],
        asset_dir,
        output_dir,
        renderer_name: "mock".into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: Some(ref_path),
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
        augment_colliders: None,
    };

    let result = execute_plan(&plan, &opts).expect("execute_plan should succeed");

    let pd = result
        .position_diff
        .expect("position_diff should be present when reference_positions is set");
    assert!(
        pd.passed,
        "mock-vs-identical-reference should structurally pass; got {pd:?}"
    );
    assert_eq!(
        pd.per_joint_max_drift_m, 0.0,
        "identical reference means zero per-joint drift"
    );
    assert_eq!(
        pd.chain_summed_drift_m, 0.0,
        "identical reference means zero summed drift"
    );
}
