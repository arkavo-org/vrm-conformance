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

use camino::Utf8PathBuf;
use vrm_runner::execute_batch::build_manifest;
use vrm_test_plan::{
    AmbientLight, Camera, ColorSpace, Diff, DiffMode, DirectionalLight, Lighting, Output,
    PostProcessing, SpecVersion, TestPlan, ToneMapping,
};

fn synthetic_plan(id: &str) -> TestPlan {
    TestPlan {
        id: id.into(),
        spec_version: SpecVersion::V1,
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
            conformance_status: Default::default(),
            pose_tolerance: None,
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
        physics: None,
        animation: None,
        render_sequence: None,
        cross_variant: None,
        ccd_colliders: None,
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
            (
                synthetic_plan("a"),
                Utf8PathBuf::from_path_buf(vrm_a.clone()).unwrap(),
            ),
            (
                synthetic_plan("b"),
                Utf8PathBuf::from_path_buf(vrm_b.clone()).unwrap(),
            ),
        ],
        Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        "univrm".into(),
    );

    assert_eq!(manifest.manifest_version, 2);
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
        &[(
            synthetic_plan("x"),
            Utf8PathBuf::from_path_buf(vrm).unwrap(),
        )],
        Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        "univrm".into(),
    );

    let json = serde_json::to_value(&manifest).expect("serialize");
    assert_eq!(json["manifest_version"], 2);
    assert_eq!(json["renderer_name"], "univrm");
    assert_eq!(json["tests"][0]["test_id"], "x");
    assert_eq!(json["tests"][0]["camera"]["position"][2], 1.5);
    assert_eq!(json["tests"][0]["output"]["color_space"], "srgb");
}

use vrm_runner::execute_batch::{parse_results_ndjson, BatchResults, ResultStatus};

#[test]
fn parse_meta_envelope_and_two_entries() {
    let input = r#"{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"v0.131.2","unity_version":"2022.3.50f1","render_pipeline":"Built-in RP","total_tests":2}
{"test_id":"a","status":"ok","output_path":"/tmp/a.png","blake3":"blake3:aaa","actual_color_space":"Srgb","render_seconds":0.18}
{"test_id":"b","status":"error","error":{"code":-32000,"message":"Unimplemented","data":{"phase":"L3"}}}
"#;
    let parsed: BatchResults = parse_results_ndjson(input).expect("parse");
    assert_eq!(parsed.meta.manifest_version, 1);
    assert_eq!(parsed.meta.renderer_name, "univrm");
    assert_eq!(parsed.meta.total_tests, 2);
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(parsed.entries[0].test_id, "a");
    assert!(matches!(parsed.entries[0].status, ResultStatus::Ok));
    assert!(matches!(parsed.entries[1].status, ResultStatus::Error));
    assert_eq!(parsed.entries[1].error.as_ref().unwrap().code, -32000);
}

#[test]
fn parse_rejects_missing_meta_envelope() {
    // First line is a test result, not a `_meta` envelope. Parser
    // must reject — the runner needs the meta line to validate the
    // batch before ingesting entries.
    let input = r#"{"test_id":"a","status":"ok","output_path":"/tmp/a.png"}
"#;
    let err = parse_results_ndjson(input).expect_err("must reject");
    assert!(
        err.to_string().contains("_meta"),
        "error should mention _meta, got: {err}"
    );
}

#[test]
fn parse_tolerates_partial_output_below_total_tests() {
    // _meta says total_tests=3 but only 1 entry follows (Unity crashed
    // mid-batch). Parser succeeds; the caller is responsible for
    // detecting the count mismatch.
    let input = r#"{"_meta":true,"manifest_version":1,"renderer_name":"univrm","renderer_version":"v0.131.2","unity_version":"2022.3.50f1","render_pipeline":"Built-in RP","total_tests":3}
{"test_id":"a","status":"ok","output_path":"/tmp/a.png","blake3":"blake3:aaa","actual_color_space":"Srgb","render_seconds":0.18}
"#;
    let parsed = parse_results_ndjson(input).expect("parse");
    assert_eq!(parsed.meta.total_tests, 3);
    assert_eq!(parsed.entries.len(), 1);
}

#[test]
fn parse_skips_blank_lines() {
    let input = "{\"_meta\":true,\"manifest_version\":1,\"renderer_name\":\"univrm\",\"renderer_version\":\"v0.131.2\",\"unity_version\":\"2022.3.50f1\",\"render_pipeline\":\"Built-in RP\",\"total_tests\":1}\n\n{\"test_id\":\"a\",\"status\":\"ok\",\"output_path\":\"/tmp/a.png\",\"blake3\":\"blake3:aaa\",\"actual_color_space\":\"Srgb\",\"render_seconds\":0.18}\n\n";
    let parsed = parse_results_ndjson(input).expect("parse");
    assert_eq!(parsed.entries.len(), 1);
}

use std::fs;
use vrm_runner::execute_batch::{write_local_manifest, LocalManifestEntry};

// =====================================================================
// Task 6 — top-level run() helpers
// =====================================================================

use vrm_runner::execute_batch::{run as run_batch, RunOptions};

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(
        workspace_root()
            .join("crates/vrm-runner/tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn write_synthetic_plan_files(
    dir: &std::path::Path,
    id: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let plan = synthetic_plan(id);
    let plan_path = dir.join(format!("{id}.test.yaml"));
    let vrm_path = dir.join(format!("{id}.vrm"));
    std::fs::write(&plan_path, serde_yml::to_string(&plan).unwrap()).unwrap();
    std::fs::write(&vrm_path, b"fake vrm").unwrap();
    (plan_path, vrm_path)
}

#[test]
fn local_manifest_writes_expected_shape() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output_dir = tmp.path();

    // Synthesize a tiny "PNG" so BLAKE3 has something to hash.
    let png_a = output_dir.join("a.png");
    fs::write(&png_a, b"fake png bytes").unwrap();

    let entries = vec![LocalManifestEntry {
        test_id: "a".into(),
        renderer_name: "univrm".into(),
        renderer_version: "v0.131.2".into(),
        output_path: png_a.to_string_lossy().to_string(),
        blake3: None, // writer fills this from the file on disk
        actual_color_space: "Srgb".into(),
        status: "ok".into(),
        error_message: None,
    }];

    let manifest_path = output_dir.join("local-manifest.json");
    write_local_manifest(&manifest_path, &entries).expect("write");

    let written = fs::read_to_string(&manifest_path).expect("read manifest back");
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(v["entries"][0]["test_id"], "a");
    assert_eq!(v["entries"][0]["renderer_name"], "univrm");
    assert!(
        v["entries"][0]["blake3"]
            .as_str()
            .unwrap()
            .starts_with("blake3:"),
        "blake3 should be prefixed; got: {:?}",
        v["entries"][0]["blake3"]
    );
}

// =====================================================================
// Task 6 — happy-path contract test
// =====================================================================

#[test]
fn happy_path_writes_local_manifest_with_blake3() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "alpha");
    write_synthetic_plan_files(&plans_dir, "bravo");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-ok.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        renderer_name: "univrm".into(),
    };

    let summary = run_batch(&opts).expect("run");
    assert_eq!(summary.total_tests, 2);
    assert_eq!(summary.ok_count, 2);
    assert_eq!(summary.error_count, 0);

    let manifest_path = output_dir.join("local-manifest.json");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let entries = manifest_json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(entry["renderer_name"], "univrm");
        assert!(
            entry["blake3"].as_str().unwrap().starts_with("blake3:"),
            "blake3 should be filled in; got: {:?}",
            entry["blake3"]
        );
    }
}

#[test]
fn partial_output_marks_missing_tests_as_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "a1");
    write_synthetic_plan_files(&plans_dir, "a2");
    write_synthetic_plan_files(&plans_dir, "a3");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-partial.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir.clone()).unwrap(),
        renderer_name: "univrm".into(),
    };

    let summary = run_batch(&opts).expect("run");
    assert_eq!(summary.total_tests, 3, "all 3 declared tests should appear");
    // mock-partial emits ceil(3/2) = 2 ok entries; 1 missing.
    assert_eq!(summary.ok_count, 2);
    assert_eq!(summary.error_count, 1);

    let manifest_path = output_dir.join("local-manifest.json");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let entries = manifest_json["entries"].as_array().unwrap();
    let error_entry = entries.iter().find(|e| e["status"] == "error").unwrap();
    assert!(
        error_entry["error_message"]
            .as_str()
            .unwrap()
            .contains("batch terminated"),
        "missing test should be marked as batch-terminated; got: {error_entry:?}"
    );
}

// =====================================================================
// Task 8 — malformed _meta envelope contract tests
// =====================================================================

#[test]
fn malformed_meta_fails_with_clear_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "z");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-bad-meta.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        renderer_name: "univrm".into(),
    };

    let err = run_batch(&opts).expect_err("should fail on missing total_tests");
    let msg = err.to_string();
    assert!(
        msg.contains("total_tests") || msg.contains("_meta"),
        "error message should mention the missing field or _meta envelope; got: {msg}"
    );
}

#[test]
fn missing_meta_envelope_fails_with_clear_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "y");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: fixture("mock-univrm-missing-meta.sh"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        renderer_name: "univrm".into(),
    };

    let err = run_batch(&opts).expect_err("should fail without _meta envelope");
    assert!(
        err.to_string().contains("_meta"),
        "error message should mention _meta; got: {err}"
    );
}

// =====================================================================
// Spawn-failure contract test
// =====================================================================

#[test]
fn spawn_failure_reports_clear_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "spawn_test");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let opts = RunOptions {
        plans_dir: Utf8PathBuf::from_path_buf(plans_dir).unwrap(),
        adapter_bin: Utf8PathBuf::from("/definitely/not/a/real/binary"),
        output_dir: Utf8PathBuf::from_path_buf(output_dir).unwrap(),
        renderer_name: "univrm".into(),
    };

    let err = run_batch(&opts).expect_err("should fail when adapter binary does not exist");
    let msg = err.to_string();
    assert!(
        msg.contains("spawn")
            || msg.contains("adapter")
            || msg.contains("/definitely/not/a/real/binary"),
        "error should mention spawn failure or the bogus path; got: {msg}"
    );
    assert!(
        !msg.contains("did not produce a readable results file"),
        "error should not misattribute to missing results file; got: {msg}"
    );
}

// =====================================================================
// Task 9 — end-to-end CLI invocation test
// =====================================================================

#[test]
fn cli_invocation_end_to_end_with_mock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plans_dir = tmp.path().join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    write_synthetic_plan_files(&plans_dir, "cli1");

    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&output_dir).unwrap();

    let out = Command::new(runner_bin())
        .args([
            "execute-test-batch",
            "--plans",
            plans_dir.to_str().unwrap(),
            "--adapter-bin",
            fixture("mock-univrm-ok.sh").as_str(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--renderer-name",
            "univrm",
            "--json",
        ])
        .output()
        .expect("spawn runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "runner should succeed end-to-end; stdout={stdout} stderr={stderr}"
    );
    let summary: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("JSON summary on stdout");
    assert_eq!(summary["total_tests"], 1);
    assert_eq!(summary["ok_count"], 1);
    assert_eq!(summary["error_count"], 0);
    assert!(
        output_dir.join("local-manifest.json").exists(),
        "local-manifest.json should be written"
    );
}
