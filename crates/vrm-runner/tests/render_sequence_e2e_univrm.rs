//! End-to-end sequence dispatch test against the UniVRM PlayMode adapter.
//!
//! Ignored by default because it requires:
//!   - macOS (UniVRM adapter is macOS-only per the launcher)
//!   - Unity 6 (6000.4.6f1) installed via Unity Hub (or UNITY_BIN env override)
//!   - PlayMode is the launcher default; L3_EDITMODE=1 would force EditMode
//!     which rejects sequence plans loudly
//!
//! Run locally with:
//!   cargo test -p vrm-runner --test render_sequence_e2e_univrm -- --ignored
//!
//! UniVRM is the consortium reference for VRM 1.0 rendering. This test
//! makes it the fourth real renderer that drives render_sequence through
//! the runner, completing the Phase 7 coverage.

use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm;
use vrm_asset_generator::params::MToonParams;
use vrm_asset_generator::sidecar::build_default_test_plan;
use vrm_runner::execute_batch::{run, RunOptions};
use vrm_test_plan::{RenderSequenceBlock, SequenceFormat, SequenceRootTransformAnimation};

fn univrm_launcher() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    let launcher = workspace_root.join("adapters/univrm/launcher.sh");
    assert!(
        launcher.exists(),
        "univrm launcher.sh missing at {launcher}"
    );
    let unity_bin = std::env::var("UNITY_BIN").unwrap_or_else(|_| {
        "/Applications/Unity/Hub/Editor/6000.4.6f1/Unity.app/Contents/MacOS/Unity".into()
    });
    assert!(
        std::path::Path::new(&unity_bin).exists(),
        "Unity 6000.4.6f1 must be installed (set UNITY_BIN env or install via Hub); \
         checked path: {unity_bin}"
    );
    launcher
}

#[test]
#[ignore = "requires Unity 6 + macOS + UniVRM PlayMode launcher"]
fn univrm_render_sequence_with_animate_root_transform_produces_frames() {
    let dir = tempfile::tempdir().unwrap();
    let plans_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = plans_dir.join("out");
    std::fs::create_dir_all(output_dir.as_std_path()).unwrap();

    // Emit a real .vrm + .meta.json + .test.yaml triplet. UniVRM's
    // launcher discovers *.test.yaml in the plans_dir and pairs with the
    // sibling .vrm by stem.
    let id = "seq_e2e_univrm";
    let vrm_path = plans_dir.join(format!("{id}.vrm"));
    let mtoon = MToonParams::defaults(id);
    emit_vrm(&mtoon, &vrm_path).expect("emit_vrm must succeed");

    // Build a default plan + inject render_sequence. Write as .test.yaml
    // for the discover-pair step.
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
    });
    plan.animation = None;

    let plan_path = plans_dir.join(format!("{id}.test.yaml"));
    let yaml = serde_yml::to_string(&plan).expect("serialize plan");
    std::fs::write(plan_path.as_std_path(), yaml).unwrap();

    // Also write a minimal meta.json sidecar — emit_vrm doesn't include
    // it, but UniVRM's batch reads it via the test plan's `asset` field.
    // The runner-side discover_plan_vrm_pairs pairs by stem, not by
    // meta.json, so an empty meta isn't required — skip writing it.

    let opts = RunOptions {
        plans_dir: plans_dir.clone(),
        adapter_bin: univrm_launcher(),
        output_dir: output_dir.clone(),
        renderer_name: "univrm".into(),
    };

    let summary = run(&opts).expect("run() against UniVRM PlayMode must succeed");

    assert!(
        summary.ok_count >= 1,
        "expected at least 1 ok result; got summary={summary:?}"
    );

    // Verify the per-frame PNGs landed in the expected directory layout:
    //   <output_dir>/<test_id>_<renderer>_frames/<NNNN>.png
    let frames_dir = output_dir.join(format!("{id}_univrm_frames"));
    assert!(frames_dir.exists(), "frames dir not created: {frames_dir}");
    for i in 0..2u32 {
        let frame_path = frames_dir.join(format!("{i:04}.png"));
        assert!(frame_path.exists(), "frame {i} not on disk: {frame_path}");
        let bytes = std::fs::metadata(frame_path.as_std_path())
            .map(|m| m.len())
            .unwrap_or(0);
        assert!(
            bytes > 100,
            "frame {i} PNG too small ({bytes} bytes) — render likely empty"
        );
    }
}
