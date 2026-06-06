//! End-to-end test: the VMK (VRMMetalKit) adapter dumps per-frame world-space
//! synthetic colliders when `render_sequence.capture_synthetic_colliders` is
//! set, and the colliders move across frames (bone-attached, not static).
//!
//! Also verifies that when `augment_colliders = false` the colliders JSON is
//! ABSENT (no synthetic colliders were generated, so nothing is written).
//!
//! Ignored by default — requires Xcode 26 + macOS 26 + a Metal GPU and
//! `swift build`. Run:
//! `cargo test -p vrm-runner --test capture_synthetic_colliders_vmk -- --ignored --nocapture`

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm_with_spring_bone;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_spring_bone_swing_sequence_test_plan;
use vrm_asset_generator::spring_bone::SpringBoneParams;
use vrm_runner::execute::{execute_plan, ExecuteOptions, FrameCollidersEntry, SequenceStatus};

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

/// augment_colliders=true: colliders JSON exists, is non-empty, and the
/// collider geometry moves across frames (real bone-attached world-space capture).
#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]
fn vmk_render_sequence_captures_moving_synthetic_colliders() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "csc_vmk_on";
    let renderer = "vmk-csc-on";
    let mtoon = MToonParams::defaults(id);
    let spring = SpringBoneParams::defaults(id);
    emit_vrm_with_spring_bone(&mtoon, &spring, &asset_dir.join(format!("{id}.vrm")))
        .expect("emit spring-bone vrm");

    let mut plan = build_spring_bone_swing_sequence_test_plan(&mtoon, &format!("{id}.vrm"));
    let rs = plan.render_sequence.as_mut().unwrap();
    rs.frame_count = 6;
    rs.capture_positions = true;
    rs.capture_synthetic_colliders = true;
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
        augment_colliders: Some(true),
    };

    let result = execute_plan(&plan, &opts).expect("execute_plan against VMK");
    let seq = result.sequence.expect("sequence populated");
    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "err={:?}",
        seq.error_message
    );

    let colliders_path = output_dir.join(format!("{id}_{renderer}_colliders.json"));

    // If augmentation did not fire on this generated asset, the colliders JSON
    // will be absent. This is a meaningful empirical finding, not a test bug.
    // We assert existence here so the test surfaces the result clearly.
    assert!(
        colliders_path.exists(),
        "VMK must persist colliders JSON at {colliders_path} — \
         if augmentation did not fire on the generated humanoid, \
         this test should be updated to NEEDS_CONTEXT status"
    );

    let entries: Vec<FrameCollidersEntry> =
        serde_json::from_str(&std::fs::read_to_string(&colliders_path).unwrap())
            .expect("colliders JSON parses");

    assert!(!entries.is_empty(), "colliders JSON must be non-empty");
    assert_eq!(entries.len(), 6, "one entry per frame");

    for e in &entries {
        assert!(
            !e.colliders.is_empty(),
            "frame {} must have at least one collider",
            e.frame_index
        );
    }

    // Prove the colliders are bone-attached (move across frames).
    // Compare first vs last frame: at least one collider's position must differ.
    let first = &entries.first().unwrap().colliders;
    let last = &entries.last().unwrap().colliders;
    assert_eq!(
        first.len(),
        last.len(),
        "collider count must be stable across frames"
    );

    use vrm_ops::tools::SequenceCollider;
    let first_center = match &first[0] {
        SequenceCollider::Sphere { center, .. } => *center,
        SequenceCollider::Capsule { a, .. } => *a,
    };
    let last_center = match &last[0] {
        SequenceCollider::Sphere { center, .. } => *center,
        SequenceCollider::Capsule { a, .. } => *a,
    };
    assert_ne!(
        first_center, last_center,
        "synthetic collider positions must move across frames — \
         bone-attached colliders should follow the animated root transform"
    );

    println!(
        "[EMPIRICAL] augment=ON: {} colliders/frame, first[0]={:?}, last[0]={:?}",
        first.len(),
        first_center,
        last_center
    );
}

/// augment_colliders=false: the colliders JSON must be ABSENT (no synthetic
/// colliders generated, so the runner writes nothing).
#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]
fn vmk_render_sequence_no_colliders_json_when_augment_off() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    // Use a distinct id/renderer so output files don't collide with the ON test.
    let id = "csc_vmk_off";
    let renderer = "vmk-csc-off";
    let mtoon = MToonParams::defaults(id);
    let spring = SpringBoneParams::defaults(id);
    emit_vrm_with_spring_bone(&mtoon, &spring, &asset_dir.join(format!("{id}.vrm")))
        .expect("emit spring-bone vrm");

    let mut plan = build_spring_bone_swing_sequence_test_plan(&mtoon, &format!("{id}.vrm"));
    let rs = plan.render_sequence.as_mut().unwrap();
    rs.frame_count = 6;
    rs.capture_positions = true;
    rs.capture_synthetic_colliders = true;
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
        augment_colliders: Some(false),
    };

    let result = execute_plan(&plan, &opts).expect("execute_plan against VMK (augment=off)");
    let seq = result.sequence.expect("sequence populated");
    assert_eq!(
        seq.status,
        SequenceStatus::Ok,
        "err={:?}",
        seq.error_message
    );

    let colliders_path = output_dir.join(format!("{id}_{renderer}_colliders.json"));
    assert!(
        !colliders_path.exists(),
        "colliders JSON must be ABSENT when augment_colliders=false; \
         no synthetic colliders → runner writes nothing"
    );

    println!("[EMPIRICAL] augment=OFF: colliders JSON correctly absent");
}
