//! End-to-end test for the render_sequence dispatch path.
//!
//! Spawns the mock-renderer binary with a plan that has `render_sequence` set.
//! The mock returns -32000 Unimplemented with phase "v1.x-sequence" — which is
//! exactly the current Phase 1 behaviour for all real adapters too. The test
//! verifies that the runner:
//!   1. Does not propagate the Unimplemented error as an anyhow::Error.
//!   2. Populates `result.sequence` with `status == Unimplemented`.
//!   3. Extracts the phase label from the error envelope.

use camino::Utf8PathBuf;
use vrm_asset_generator::{params::MToonParams, sidecar::build_default_test_plan};
use vrm_runner::execute::{execute_plan, ExecuteOptions, SequenceStatus};
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat};

/// Resolve the workspace target dir, then the mock binary. Builds the
/// mock-renderer if it isn't already present.
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
fn render_sequence_against_mock_returns_unimplemented() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "seq_unimpl";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));
    let meta_path = asset_dir.join(format!("{id}.meta.json"));
    // Minimal valid-enough VRM header so mock's load_vrm succeeds.
    std::fs::write(&vrm_path, b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00").unwrap();
    let meta = serde_json::json!({
        "id": id,
        "license": "CC0-1.0",
        "params": MToonParams::defaults(id),
    });
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();

    // Build a default plan, then inject the render_sequence block.
    let mut plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count: 2,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: None,
        temporal_ssim_threshold: None,
    });

    let renderer_name = "mock-seq";
    let opts = ExecuteOptions {
        adapter_bin: mock_bin(),
        adapter_args: Vec::new(),
        asset_dir,
        output_dir,
        renderer_name: renderer_name.into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
    };

    // The runner must not propagate Unimplemented as an error.
    let result = execute_plan(&plan, &opts).expect(
        "execute_plan must succeed even when adapter returns Unimplemented for render_sequence",
    );

    // sequence field must be populated.
    let seq = result
        .sequence
        .expect("sequence mode should populate result.sequence");

    assert_eq!(
        seq.status,
        SequenceStatus::Unimplemented,
        "mock returns -32000 Unimplemented; status should be Unimplemented"
    );
    assert_eq!(
        seq.unimplemented_phase.as_deref(),
        Some("v1.x-sequence"),
        "phase label must be extracted from error envelope"
    );
    assert!(
        seq.result.is_none(),
        "no RenderSequenceResult when Unimplemented"
    );
    assert!(
        seq.error_message.is_none(),
        "error_message should be None for Unimplemented (not a crash)"
    );

    // Diff should be skipped in sequence mode.
    assert!(
        result.diff.is_none(),
        "single-frame diff must be skipped in sequence mode"
    );
}
