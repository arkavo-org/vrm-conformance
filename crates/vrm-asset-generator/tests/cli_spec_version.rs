use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_vrm-asset-generator")
}

#[test]
fn emit_default_accepts_spec_version_v0() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id",
            "smoke_v0",
            "--spec-version",
            "0.x",
            "--output-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(tmp.path().join("smoke_v0.vrm").exists());
}

#[test]
fn emit_default_accepts_spec_version_v1() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id",
            "smoke_v1",
            "--spec-version",
            "1.0",
            "--output-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(tmp.path().join("smoke_v1.vrm").exists());
}

#[test]
fn emit_default_defaults_to_v1_when_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id",
            "smoke_default",
            "--output-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(tmp.path().join("smoke_default.vrm").exists());
}

#[test]
fn emit_default_rejects_invalid_spec_version() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id",
            "smoke_bad",
            "--spec-version",
            "2.0",
            "--output-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(
        !out.status.success(),
        "expected failure for spec_version=2.0"
    );
}

#[test]
fn emit_sweep_v0_produces_vrm0_assets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sweep0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run emit-sweep");
    assert!(
        status.success(),
        "emit-sweep --spec-version 0.x must exit 0"
    );
    let vrm = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "vrm"))
        .expect("at least one .vrm emitted");
    let bytes = std::fs::read(&vrm).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("\"VRM\""),
        "0.x asset must carry the bare `VRM` extension, not VRMC_*"
    );
}
