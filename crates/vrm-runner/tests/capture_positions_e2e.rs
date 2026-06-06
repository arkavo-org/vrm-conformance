//! End-to-end tests for the `capture_positions` pipeline: TestPlan with
//! `render_sequence.capture_positions = true` → runner threads the flag
//! into `RenderSequenceParams` → mock emits `spring_positions` per frame →
//! runner persists `<output_dir>/<plan_id>_<renderer>_positions.json`.
//!
//! Also verifies the inverse: `capture_positions = false` (or absent) → no
//! positions JSON is written.

use camino::Utf8PathBuf;
use vrm_asset_generator::{params::MToonParams, sidecar::build_default_test_plan};
use vrm_runner::execute::{execute_plan, ExecuteOptions, FramePositionsEntry, SequenceStatus};
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat};

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

fn make_plan_with_capture(
    id: &str,
    capture: bool,
    asset_dir: &Utf8PathBuf,
) -> vrm_test_plan::TestPlan {
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

    let mut plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count: 3,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: None,
        temporal_ssim_threshold: None,
        capture_positions: capture,
        capture_synthetic_colliders: false,
    });
    plan
}

/// When `capture_positions = true`, the runner must write
/// `<output_dir>/<plan_id>_<renderer>_positions.json` containing a non-empty
/// array of `FramePositionsEntry` values where each frame has at least one
/// spring with joint_positions.
#[test]
fn capture_positions_true_writes_positions_json() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "ccp_on";
    let renderer = "mock-ccp";
    let plan = make_plan_with_capture(id, true, &asset_dir);

    let opts = ExecuteOptions {
        adapter_bin: mock_bin(),
        adapter_args: Vec::new(),
        asset_dir: asset_dir.clone(),
        output_dir: output_dir.clone(),
        renderer_name: renderer.into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
        augment_colliders: None,
    };

    let result =
        execute_plan(&plan, &opts).expect("execute_plan must succeed with capture_positions=true");

    // Sequence succeeded.
    let seq = result.sequence.expect("sequence field populated");
    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "mock must implement render_sequence"
    );
    assert!(seq.result.as_ref().map(|r| r.frames.len()).unwrap_or(0) > 0);

    // Positions JSON written.
    let pos_path = output_dir.join(format!("{id}_{renderer}_positions.json"));
    assert!(
        pos_path.exists(),
        "positions JSON must be written at {pos_path}"
    );

    let content = std::fs::read_to_string(&pos_path).expect("positions JSON must be readable");
    let entries: Vec<FramePositionsEntry> = serde_json::from_str(&content)
        .expect("positions JSON must parse as Vec<FramePositionsEntry>");

    assert!(
        !entries.is_empty(),
        "positions JSON must contain at least one frame entry"
    );

    for entry in &entries {
        assert!(
            !entry.springs.is_empty(),
            "frame {} must have at least one spring chain",
            entry.frame_index
        );
        for chain in &entry.springs {
            assert!(
                !chain.joint_positions.is_empty(),
                "frame {} chain '{}' must have at least one joint position",
                entry.frame_index,
                chain.name
            );
        }
    }

    // All three frames should have positions from the mock.
    assert_eq!(
        entries.len(),
        3,
        "all 3 frames must have positions in the JSON"
    );
    // Frames are in order.
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry.frame_index, i as u32,
            "frame_index must be sequential"
        );
    }
}

/// When `capture_positions = false` (or absent), no positions JSON is written.
#[test]
fn capture_positions_false_writes_no_json() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "ccp_off";
    let renderer = "mock-ccp-off";
    let plan = make_plan_with_capture(id, false, &asset_dir);

    let opts = ExecuteOptions {
        adapter_bin: mock_bin(),
        adapter_args: Vec::new(),
        asset_dir: asset_dir.clone(),
        output_dir: output_dir.clone(),
        renderer_name: renderer.into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
        augment_colliders: None,
    };

    let result =
        execute_plan(&plan, &opts).expect("execute_plan must succeed with capture_positions=false");

    // Sequence still succeeds.
    let seq = result.sequence.expect("sequence field populated");
    assert_eq!(seq.status, SequenceStatus::Ok);

    // No positions JSON written.
    let pos_path = output_dir.join(format!("{id}_{renderer}_positions.json"));
    assert!(
        !pos_path.exists(),
        "positions JSON must NOT be written when capture_positions=false; found {pos_path}"
    );
}
