use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn describe_outputs_json_schema() {
    let mut cmd = Command::cargo_bin("vrm-asset-generator").unwrap();
    cmd.args(["describe", "--format", "json"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"operations\""))
        .stdout(predicate::str::contains("\"emit-default\""));
}

#[test]
fn emit_default_writes_three_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("vrm-asset-generator").unwrap();
    cmd.args(["emit-default", "--id", "smoke", "--output-dir", out_dir]);
    cmd.assert().success();

    let stem = std::path::Path::new(out_dir).join("smoke");
    assert!(stem.with_extension("vrm").exists());
    assert!(stem.with_extension("meta.json").exists());
    assert!(stem.with_extension("test.yaml").exists());
}
