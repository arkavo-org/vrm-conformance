//! RFC 0007: a plan that sets `material_time_seconds` must never render at
//! the adapter's own clock. The mock declares `set_material_time` but
//! returns Unimplemented, so the runner must fail the plan with a message
//! naming the op instead of producing a PNG.

use camino::Utf8PathBuf;
use vrm_asset_generator::{params::MToonParams, sidecar::build_default_test_plan};
use vrm_runner::execute::{execute_plan, ExecuteOptions};
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
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "--quiet", "-p", "vrm-mock-renderer"])
        .status()
        .expect("invoke cargo build for vrm-mock-renderer");
    assert!(
        status.success(),
        "cargo build vrm-mock-renderer must succeed"
    );
    bin
}

fn setup(id: &str) -> (tempfile::TempDir, ExecuteOptions) {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    std::fs::write(
        asset_dir.join(format!("{id}.vrm")),
        b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00",
    )
    .unwrap();
    let meta = serde_json::json!({
        "id": id,
        "license": "CC0-1.0",
        "params": MToonParams::defaults(id),
    });
    std::fs::write(
        asset_dir.join(format!("{id}.meta.json")),
        serde_json::to_vec(&meta).unwrap(),
    )
    .unwrap();
    let opts = ExecuteOptions {
        adapter_bin: mock_bin(),
        adapter_args: Vec::new(),
        asset_dir: asset_dir.clone(),
        output_dir: asset_dir,
        renderer_name: "mock".into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: None,
        vrma_path: None,
        apply_at_time: 0.0,
        reference_pose_json: None,
        augment_colliders: None,
    };
    (dir, opts)
}

#[test]
fn timed_plan_fails_loudly_on_an_adapter_without_set_material_time() {
    let id = "uvanim_unsupported";
    let (_dir, opts) = setup(id);
    let mut plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    plan.material_time_seconds = Some(1.0);

    let err = execute_plan(&plan, &opts).expect_err("a timed plan must not render on the mock");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("set_material_time unsupported by mock"),
        "error must name the op and renderer, got: {msg}"
    );
    assert!(
        !opts.output_dir.join(format!("{id}_mock.png")).exists(),
        "no PNG may be written for a timed plan the adapter cannot honour"
    );
}

#[test]
fn untimed_plan_never_calls_set_material_time() {
    let id = "uvanim_untimed";
    let (_dir, opts) = setup(id);
    let plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    assert!(plan.material_time_seconds.is_none());
    execute_plan(&plan, &opts).expect("untimed plans are unaffected by RFC 0007");
}

#[test]
fn timed_sequence_plan_is_rejected_before_spawning() {
    let id = "uvanim_seq";
    let (_dir, opts) = setup(id);
    let mut plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    plan.material_time_seconds = Some(0.5);
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
    let err = execute_plan(&plan, &opts).expect_err("time + sequence is invalid");
    assert!(format!("{err:#}").contains("render_sequence"), "{err:#}");
}
