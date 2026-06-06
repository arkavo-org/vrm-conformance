//! End-to-end test: the VMK (VRMMetalKit) adapter reports per-frame
//! spring-bone positions when `render_sequence.capture_positions` is set.
//!
//! Ignored by default — requires Xcode 26 + macOS 26 + a Metal GPU and
//! `swift build`. Run:
//! `cargo test -p vrm-runner --test capture_positions_vmk -- --ignored`

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm_with_spring_bone;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_spring_bone_swing_sequence_test_plan;
use vrm_asset_generator::spring_bone::SpringBoneParams;
use vrm_runner::execute::{execute_plan, ExecuteOptions, FramePositionsEntry, SequenceStatus};

fn vmk_bin() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let vmk_dir = workspace_root.join("adapters/vrm-metal-kit");
    let bin = vmk_dir.join(".build/debug/vrm-metal-kit-adapter");
    if !bin.exists() {
        let status = std::process::Command::new("swift")
            .arg("build")
            .current_dir(vmk_dir.as_std_path())
            .status()
            .expect("swift build must be available (Xcode 26 + macOS 26)");
        assert!(status.success(), "swift build vrm-metal-kit-adapter failed");
    }
    bin
}

#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]
fn vmk_render_sequence_captures_moving_spring_positions() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "ccp_vmk";
    let renderer = "vmk-ccp";
    let mtoon = MToonParams::defaults(id);
    let spring = SpringBoneParams::defaults(id);
    emit_vrm_with_spring_bone(&mtoon, &spring, &asset_dir.join(format!("{id}.vrm")))
        .expect("emit spring-bone vrm");

    let mut plan = build_spring_bone_swing_sequence_test_plan(&mtoon, &format!("{id}.vrm"));
    let rs = plan.render_sequence.as_mut().unwrap();
    rs.frame_count = 6;
    rs.capture_positions = true;
    plan.animation = None;

    let opts = ExecuteOptions {
        adapter_bin: vmk_bin(),
        adapter_args: Vec::new(),
        asset_dir,
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

    let result = execute_plan(&plan, &opts).expect("execute_plan against VMK");
    let seq = result.sequence.expect("sequence populated");
    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "err={:?}",
        seq.error_message
    );

    let pos_path = output_dir.join(format!("{id}_{renderer}_positions.json"));
    assert!(
        pos_path.exists(),
        "VMK must persist positions JSON at {pos_path}"
    );
    let entries: Vec<FramePositionsEntry> =
        serde_json::from_str(&std::fs::read_to_string(&pos_path).unwrap())
            .expect("positions JSON parses");
    assert_eq!(entries.len(), 6, "one entry per frame");
    for e in &entries {
        assert!(
            !e.springs.is_empty(),
            "frame {} has a spring",
            e.frame_index
        );
        assert!(!e.springs[0].joint_positions.is_empty());
    }
    assert_ne!(
        entries.first().unwrap().springs[0].joint_positions,
        entries.last().unwrap().springs[0].joint_positions,
        "positions must change across frames (real solver, not static)"
    );
}
