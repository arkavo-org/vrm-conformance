//! End-to-end runner-against-mock test for plan.animation.root_transform.
//!
//! Spawns the mock-renderer binary, executes a TestPlan whose animation
//! block requests a short root-translation excitation, asserts the run
//! succeeds and the rendered PNG lands on disk. Mock acknowledges
//! animate_root_transform as a deterministic no-op (it has no physics
//! state), so this exercises the full Rust-side plumbing without
//! requiring the three-vrm browser stack.

use camino::Utf8PathBuf;
use vrm_asset_generator::{params::MToonParams, sidecar::build_default_test_plan};
use vrm_runner::execute::{execute_plan, ExecuteOptions};
use vrm_test_plan::{AnimationConfig, RootTransformAnimation};

/// Resolve the workspace target dir, then the mock binary. Builds the
/// mock-renderer if it isn't already present — `cargo test --workspace`
/// builds it ahead of time, but `cargo test -p vrm-runner` doesn't.
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
fn execute_plan_with_animation_against_mock_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let asset_dir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let output_dir = asset_dir.clone();

    let id = "anim_e2e";
    let vrm_path = asset_dir.join(format!("{id}.vrm"));
    let meta_path = asset_dir.join(format!("{id}.meta.json"));
    std::fs::write(&vrm_path, b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00").unwrap();
    let meta = serde_json::json!({
        "id": id,
        "license": "CC0-1.0",
        "params": MToonParams::defaults(id),
    });
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();

    let mut plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    plan.animation = Some(AnimationConfig {
        root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.2, 0.0, 0.0],
            duration_seconds: 0.167,
            fps: 60,
        }),
    });

    let renderer_name = "mock-anim";
    let opts = ExecuteOptions {
        adapter_bin: mock_bin(),
        adapter_args: Vec::new(),
        asset_dir,
        output_dir: output_dir.clone(),
        renderer_name: renderer_name.into(),
        emit_progress_ndjson: false,
        reference: None,
    };

    let result = execute_plan(&plan, &opts).expect("execute_plan must succeed");
    assert_eq!(result.test_id, id);
    assert_eq!(result.renderer, renderer_name);
    let expected_png = output_dir.join(format!("{id}_{renderer_name}.png"));
    assert!(
        expected_png.exists(),
        "expected PNG at {expected_png}; result.output_png={}",
        result.output_png
    );
}

#[test]
fn yaml_round_trip_preserves_animation_block() {
    let id = "anim_yaml";
    let mut plan = build_default_test_plan(&MToonParams::defaults(id), &format!("{id}.vrm"));
    plan.animation = Some(AnimationConfig {
        root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.1, 0.0, 0.0],
            duration_seconds: 0.25,
            fps: 60,
        }),
    });

    let yaml = serde_yml::to_string(&plan).unwrap();
    assert!(yaml.contains("animation"), "animation present in YAML");
    assert!(yaml.contains("duration_seconds"), "yaml: {yaml}");

    let parsed: vrm_test_plan::TestPlan = serde_yml::from_str(&yaml).unwrap();
    let root = parsed
        .animation
        .expect("animation deserialized")
        .root_transform
        .expect("root_transform deserialized");
    assert_eq!(root.translation_end, [0.1, 0.0, 0.0]);
    assert_eq!(root.duration_seconds, 0.25);
    assert_eq!(root.fps, 60);
}
