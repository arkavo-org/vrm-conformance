//! End-to-end test: the UniVRM adapter (PlayMode batch) reports per-frame
//! spring-bone positions when `render_sequence.capture_positions` is set.
//!
//! UniVRM is the golden reference, so this makes the penetration metric
//! measurable against the oracle's real FastSpringBone solver.
//!
//! Ignored by default — requires Unity 6000.4.6f1 (the pinned editor;
//! launcher.sh resolves it) + a Personal license, and is slow (Unity boot +
//! PlayMode batch). Run locally with:
//! `cargo test -p vrm-runner --test capture_positions_univrm -- --ignored`

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm_with_spring_bone;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_spring_bone_swing_sequence_test_plan;
use vrm_asset_generator::spring_bone::SpringBoneParams;
use vrm_runner::execute::FramePositionsEntry;
use vrm_runner::execute_batch::{run as run_batch, RunOptions};

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// The UniVRM launcher; gated on the pinned Unity editor being installed.
fn univrm_launcher_or_skip() -> Option<Utf8PathBuf> {
    let unity = std::path::Path::new(
        "/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity",
    );
    if !unity.exists() {
        eprintln!("skip: Unity 6000.4.6f1 not installed");
        return None;
    }
    Some(Utf8PathBuf::from_path_buf(workspace_root().join("adapters/univrm/launcher.sh")).unwrap())
}

#[test]
#[ignore = "requires Unity 6000.4.6f1 + license; slow PlayMode batch"]
fn univrm_render_sequence_captures_moving_spring_positions() {
    let Some(launcher) = univrm_launcher_or_skip() else {
        return;
    };

    let tmp = tempfile::tempdir().unwrap();
    let plans_dir = Utf8PathBuf::from_path_buf(tmp.path().join("plans")).unwrap();
    let output_dir = Utf8PathBuf::from_path_buf(tmp.path().join("out")).unwrap();
    std::fs::create_dir_all(&plans_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();

    let id = "ccp_univrm";
    let renderer = "univrm";
    let mtoon = MToonParams::defaults(id);
    let spring = SpringBoneParams::defaults(id);
    emit_vrm_with_spring_bone(&mtoon, &spring, &plans_dir.join(format!("{id}.vrm")))
        .expect("emit spring-bone vrm");

    let mut plan = build_spring_bone_swing_sequence_test_plan(&mtoon, &format!("{id}.vrm"));
    let rs = plan.render_sequence.as_mut().unwrap();
    rs.frame_count = 6;
    rs.capture_positions = true;
    plan.animation = None;
    std::fs::write(
        plans_dir.join(format!("{id}.test.yaml")),
        serde_yml::to_string(&plan).unwrap(),
    )
    .unwrap();

    let opts = RunOptions {
        plans_dir,
        adapter_bin: launcher,
        output_dir: output_dir.clone(),
        renderer_name: renderer.into(),
    };
    let summary = run_batch(&opts).expect("run univrm batch");
    assert_eq!(summary.ok_count, 1, "the sequence plan must render ok");

    let pos_path = output_dir.join(format!("{id}_{renderer}_positions.json"));
    assert!(
        pos_path.exists(),
        "UniVRM must persist positions JSON at {pos_path}"
    );

    let entries: Vec<FramePositionsEntry> =
        serde_json::from_str(&std::fs::read_to_string(&pos_path).unwrap())
            .expect("positions JSON parses as Vec<FramePositionsEntry>");
    assert_eq!(entries.len(), 6, "one positions entry per frame");
    for e in &entries {
        assert!(
            !e.springs.is_empty(),
            "frame {} has a spring",
            e.frame_index
        );
        assert!(!e.springs[0].joint_positions.is_empty());
    }
    // Real FastSpringBone under animation: positions must change across frames.
    assert_ne!(
        entries.first().unwrap().springs[0].joint_positions,
        entries.last().unwrap().springs[0].joint_positions,
        "spring positions must change across frames (real solver, not static)"
    );
}
