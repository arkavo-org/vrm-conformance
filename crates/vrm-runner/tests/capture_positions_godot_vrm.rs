//! End-to-end test: the godot-vrm adapter reports per-frame spring-bone
//! positions when `render_sequence.capture_positions = true`.
//!
//! This is the FIRST real adapter (non-mock) to populate
//! `SequenceFrame.spring_positions` from an actual spring-bone solver,
//! unblocking `penetration-diff` against a real renderer.
//!
//! Ignored by default — requires:
//!   - Godot 4 on PATH
//!   - `vrm-godot-shim` binary (autobuilt by cargo)
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test capture_positions_godot_vrm -- --ignored

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm_with_spring_bone;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_spring_bone_swing_sequence_test_plan;
use vrm_asset_generator::spring_bone::SpringBoneParams;
use vrm_runner::execute::{execute_plan, ExecuteOptions, FramePositionsEntry, SequenceStatus};

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
    let probe = std::process::Command::new("godot")
        .arg("--version")
        .output();
    assert!(
        probe.is_ok() && probe.unwrap().status.success(),
        "godot 4 must be on PATH for this test"
    );
    bin
}

/// With `capture_positions = true`, a render_sequence run through godot-vrm
/// must persist a positions JSON whose per-frame spring chains carry real
/// joint coordinates — and, under `animate_root_transform`, those positions
/// must CHANGE across frames (the property that distinguishes a real solver
/// from the mock's static synthetic chain).
#[test]
#[ignore = "requires godot 4 on PATH"]
fn godot_vrm_render_sequence_captures_moving_spring_positions() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "ccp_godot";
    let renderer = "godot-vrm-ccp";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));

    let mtoon = MToonParams::defaults(id);
    let spring = SpringBoneParams::defaults(id);
    emit_vrm_with_spring_bone(&mtoon, &spring, &vrm_path)
        .expect("emit_vrm_with_spring_bone must succeed");

    let mut plan = build_spring_bone_swing_sequence_test_plan(&mtoon, &format!("{id}.vrm"));
    // Keep the run short (godot renders every frame) but still long enough
    // for the swing to move the chain.
    let rs = plan
        .render_sequence
        .as_mut()
        .expect("swing-sequence plan has a render_sequence block");
    rs.frame_count = 6;
    rs.capture_positions = true;
    plan.animation = None;

    let opts = ExecuteOptions {
        adapter_bin: godot_shim_bin(),
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

    let result = execute_plan(&plan, &opts).expect("execute_plan against godot-vrm must succeed");

    let seq = result.sequence.expect("sequence field populated");
    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "godot must run the sequence; error={:?}",
        seq.error_message
    );

    // Positions JSON must exist — the runner only writes it when the adapter
    // returns per-frame spring_positions.
    let pos_path = output_dir.join(format!("{id}_{renderer}_positions.json"));
    assert!(
        pos_path.exists(),
        "positions JSON must be written at {pos_path} — godot must return spring_positions"
    );

    let content = std::fs::read_to_string(&pos_path).expect("positions JSON readable");
    let entries: Vec<FramePositionsEntry> =
        serde_json::from_str(&content).expect("positions JSON parses as Vec<FramePositionsEntry>");

    assert_eq!(entries.len(), 6, "one positions entry per frame");

    for entry in &entries {
        assert!(
            !entry.springs.is_empty(),
            "frame {} must carry at least one spring chain",
            entry.frame_index
        );
        for chain in &entry.springs {
            assert!(
                !chain.joint_positions.is_empty(),
                "frame {} chain '{}' must have joint positions",
                entry.frame_index,
                chain.name
            );
        }
    }

    // Real solver under animation: the chain must move. Compare first vs last
    // frame's first spring's joint positions — they must differ. (The mock's
    // static chain would be byte-identical and fail this.)
    let first = &entries.first().unwrap().springs[0].joint_positions;
    let last = &entries.last().unwrap().springs[0].joint_positions;
    assert_ne!(
        first, last,
        "spring positions must change across frames under animate_root_transform \
         (a static chain means physics didn't run)"
    );
}
