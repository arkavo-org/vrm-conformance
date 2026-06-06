//! End-to-end sequence dispatch test against the godot-vrm adapter.
//!
//! Ignored by default because it requires:
//!   - Godot 4 on PATH
//!   - `vrm-godot-shim` binary built (gets autobuilt by cargo)
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test render_sequence_e2e_godot_vrm -- --ignored
//!
//! This test makes godot-vrm the third real renderer that drives the
//! render_sequence path end-to-end through the runner.

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_runner::execute::{execute_plan, ExecuteOptions, SequenceStatus};
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat, SequenceRootTransformAnimation};

fn godot_shim_bin() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let target_root = std::env::var("CARGO_TARGET_DIR")
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));
    let bin = target_root.join("debug/vrm-godot-shim");
    if !bin.exists() {
        let status = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "--bin", "vrm-godot-shim"])
            .status()
            .expect("cargo build vrm-godot-shim must succeed");
        assert!(status.success());
    }
    // Verify godot is on PATH so a missing toolchain fails loud.
    let probe = std::process::Command::new("godot")
        .arg("--version")
        .output();
    assert!(
        probe.is_ok() && probe.unwrap().status.success(),
        "godot 4 must be on PATH for this test"
    );
    bin
}

#[test]
#[ignore = "requires godot 4 on PATH"]
fn godot_vrm_render_sequence_with_animate_root_transform_produces_frames() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "seq_e2e_godot_vrm";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));

    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    let mut plan = build_default_test_plan(&mtoon, &format!("{id}.vrm"));
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count: 2,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.1, 0.0, 0.0],
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,
        capture_positions: false,
        capture_synthetic_colliders: false,
    });
    plan.animation = None;

    let opts = ExecuteOptions {
        adapter_bin: godot_shim_bin(),
        adapter_args: Vec::new(),
        asset_dir,
        output_dir,
        renderer_name: "godot-vrm-seq".into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
        augment_colliders: None,
    };

    let result = execute_plan(&plan, &opts)
        .expect("execute_plan against godot-vrm in sequence mode must succeed");

    let seq = result
        .sequence
        .expect("sequence mode should populate result.sequence");

    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "godot-vrm should return Ok now; got {:?}, error_message={:?}, phase={:?}",
        seq.status,
        seq.error_message,
        seq.unimplemented_phase
    );

    let seq_result = seq
        .result
        .expect("RenderSequenceResult should be populated");
    assert_eq!(seq_result.frames.len(), 2);

    let zero_sentinel = format!("blake3:{}", "0".repeat(64));

    for (i, frame) in seq_result.frames.iter().enumerate() {
        assert_eq!(frame.index, i as u32);
        assert!(
            std::path::Path::new(&frame.path).exists(),
            "frame {i} not on disk: {}",
            frame.path
        );
        let bytes = std::fs::read(&frame.path).expect("frame PNG readable");
        assert!(
            bytes.len() > 100,
            "frame {i} PNG too small ({} bytes)",
            bytes.len()
        );
        assert!(frame.blake3.starts_with("blake3:"));
        assert_eq!(frame.blake3.len(), "blake3:".len() + 64);
        assert_ne!(
            frame.blake3, zero_sentinel,
            "frame {i} blake3 is still the sentinel — runner re-hash didn't run"
        );
    }
}
