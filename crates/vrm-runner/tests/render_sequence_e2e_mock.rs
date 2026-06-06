//! End-to-end test for the render_sequence dispatch path.
//!
//! The mock is the canonical reference renderer; this test verifies the full
//! success path: TestPlan → plan_to_ops → execute_plan → render_sequence
//! dispatch → SequenceExecuteResult → N PNG frames on disk.
//!
//! Phase 3 Task 1 made the mock a real implementer of `render_sequence`,
//! replacing the old Phase 2 Unimplemented behaviour. The assertions below
//! verify that all N frames land on disk with valid BLAKE3 hashes.

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
fn render_sequence_against_mock_produces_frames() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "seq_e2e";
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
        capture_positions: false,
        capture_synthetic_colliders: false,
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

    // Sequence-mode execution must succeed (no anyhow propagation).
    let result = execute_plan(&plan, &opts)
        .expect("execute_plan must succeed against the mock in sequence mode");

    // Sequence field must be populated.
    let seq = result
        .sequence
        .expect("sequence mode should populate result.sequence");

    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "mock is a real implementer; status should be Ok"
    );
    assert!(
        seq.unimplemented_phase.is_none(),
        "no Unimplemented when status is Ok"
    );
    assert!(
        seq.error_message.is_none(),
        "no error_message when status is Ok"
    );

    let seq_result = seq
        .result
        .expect("RenderSequenceResult should be populated");
    assert_eq!(seq_result.frames.len(), 2, "frame_count was 2");

    // Each frame on disk + BLAKE3 prefix.
    for (i, frame) in seq_result.frames.iter().enumerate() {
        assert_eq!(frame.index, i as u32);
        assert!(
            std::path::Path::new(&frame.path).exists(),
            "frame {} not on disk: {}",
            i,
            frame.path
        );
        assert!(
            frame.blake3.starts_with("blake3:"),
            "frame {} blake3 missing prefix: {}",
            i,
            frame.blake3
        );
        assert_eq!(
            frame.blake3.len(),
            "blake3:".len() + 64,
            "frame {i} blake3 wrong length"
        );
    }

    // Diff should still be skipped in sequence mode (Phase 2 contract).
    assert!(
        result.diff.is_none(),
        "single-frame diff must be skipped in sequence mode"
    );
}
