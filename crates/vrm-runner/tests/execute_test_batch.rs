//! Contract tests for `vrm-runner execute-test-batch`. Tests use mock
//! shell-script fixtures so they run without Unity installed.

use std::path::PathBuf;
use std::process::Command;

fn runner_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vrm-runner"))
}

#[test]
fn execute_test_batch_subcommand_is_registered() {
    // The subcommand must parse — clap should accept the flag set even
    // if the implementation is a stub. Failing here means the CLI
    // surface doesn't exist yet.
    let out = Command::new(runner_bin())
        .args(["execute-test-batch", "--help"])
        .output()
        .expect("spawn runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "execute-test-batch --help should succeed; stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("--plans"),
        "help must mention --plans flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--adapter-bin"),
        "help must mention --adapter-bin flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--output-dir"),
        "help must mention --output-dir flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--renderer-name"),
        "help must mention --renderer-name flag; got: {stdout}"
    );
    assert!(
        stdout.contains("--json"),
        "help must mention --json flag; got: {stdout}"
    );
}

use vrm_runner::execute_batch::build_manifest;
use vrm_test_plan::{
    AmbientLight, Camera, Diff, DiffMode, DirectionalLight, Lighting, Output, PostProcessing,
    TestPlan, ToneMapping, ColorSpace,
};
use camino::Utf8PathBuf;

fn synthetic_plan(id: &str) -> TestPlan {
    TestPlan {
        id: id.into(),
        spec_section: "VRMC_materials_mtoon".into(),
        asset: format!("{id}.vrm"),
        camera: Camera {
            position: [0.0, 1.4, 1.5],
            target: [0.0, 1.4, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [-0.3, -0.6, -0.7],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false,
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        },
        output: Output {
            width: 1024,
            height: 1024,
            color_space: ColorSpace::Srgb,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold: 0.985,
            reference_renderer: "vrm-metal-kit".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
        physics: None,
        animation: None,
    }
}

#[test]
fn manifest_carries_two_entries_with_absolute_paths() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vrm_a = tmp.path().join("a.vrm");
    let vrm_b = tmp.path().join("b.vrm");
    std::fs::write(&vrm_a, b"fake vrm").unwrap();
    std::fs::write(&vrm_b, b"fake vrm").unwrap();
    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let manifest = build_manifest(
        &[
            (synthetic_plan("a"), Utf8PathBuf::from_path_buf(vrm_a.clone()).unwrap()),
            (synthetic_plan("b"), Utf8PathBuf::from_path_buf(vrm_b.clone()).unwrap()),
        ],
        Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        "univrm".into(),
    );

    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.renderer_name, "univrm");
    assert_eq!(manifest.tests.len(), 2);
    assert_eq!(manifest.tests[0].test_id, "a");
    assert!(
        manifest.tests[0].vrm_path.as_str().starts_with('/'),
        "vrm_path should be absolute, got: {}",
        manifest.tests[0].vrm_path
    );
    assert!(
        manifest.output_dir.as_str().starts_with('/'),
        "output_dir should be absolute, got: {}",
        manifest.output_dir
    );
}

#[test]
fn manifest_serializes_to_expected_json_shape() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vrm = tmp.path().join("x.vrm");
    std::fs::write(&vrm, b"fake").unwrap();
    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let manifest = build_manifest(
        &[(synthetic_plan("x"), Utf8PathBuf::from_path_buf(vrm).unwrap())],
        Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        "univrm".into(),
    );

    let json = serde_json::to_value(&manifest).expect("serialize");
    assert_eq!(json["manifest_version"], 1);
    assert_eq!(json["renderer_name"], "univrm");
    assert_eq!(json["tests"][0]["test_id"], "x");
    assert_eq!(json["tests"][0]["camera"]["position"][2], 1.5);
    assert_eq!(json["tests"][0]["output"]["color_space"], "srgb");
}
