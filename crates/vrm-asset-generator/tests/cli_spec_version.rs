use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_vrm-asset-generator")
}

#[test]
#[ignore = "requires .tools/vrm-validator-cli"]
fn emit_spring_bone_v0_passes_validator() {
    use camino::Utf8PathBuf;
    use vrm_asset_generator::emit::emit_with_sidecars_spring_bone_v0;
    use vrm_asset_generator::{params::MToonParams, spring_bone::SpringBoneParams};
    use vrm_validator_wrap::{validate, ValidatorConfig};

    let cfg = match ValidatorConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "SKIP: validator shim not reachable ({e}). Set VRM_VALIDATOR_BIN to an absolute path \
                 (e.g. VRM_VALIDATOR_BIN=$(git rev-parse --show-toplevel)/.tools/vrm-validator-cli) \
                 or run scripts/install-validator.sh from the workspace root."
            );
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let stem = Utf8PathBuf::from_path_buf(tmp.path().join("sb_v0_val")).expect("utf8 path");
    let mtoon = MToonParams::defaults("sb_v0_val");
    let spring = SpringBoneParams::defaults("sb_v0_val");
    emit_with_sidecars_spring_bone_v0(&mtoon, &spring, &stem).expect("emission must succeed");

    let vrm = stem.with_extension("vrm");
    let report = validate(&cfg, &vrm).expect("validator must run");

    if report.issues.num_errors > 0 {
        let summary = report
            .issues
            .messages
            .iter()
            .filter(|m| m.severity == 0)
            .map(|m| format!("{}: {}", m.code, m.message))
            .collect::<Vec<_>>()
            .join("; ");
        panic!(
            "VRM 0.x spring-bone asset has {} validator errors: {summary}",
            report.issues.num_errors
        );
    }
    eprintln!(
        "emit_spring_bone_v0_passes_validator: 0 errors, {} warnings",
        report.issues.num_warnings
    );
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

#[test]
#[ignore = "requires .tools/vrm-validator-cli"]
fn emit_sweep_v0_assets_pass_validator() {
    use camino::Utf8PathBuf;
    use vrm_validator_wrap::{validate, ValidatorConfig};

    let cfg = match ValidatorConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "SKIP: validator shim not reachable ({e}). Set VRM_VALIDATOR_BIN to an absolute path \
                 (e.g. VRM_VALIDATOR_BIN=$(git rev-parse --show-toplevel)/.tools/vrm-validator-cli) \
                 or run scripts/install-validator.sh from the workspace root."
            );
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sweep0x_val");
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

    let vrm_files: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "vrm"))
        .collect();
    assert!(!vrm_files.is_empty(), "at least one .vrm must be emitted");

    let mut failures = Vec::new();
    for path in &vrm_files {
        let utf8 = Utf8PathBuf::from_path_buf(path.clone()).expect("utf8 path");
        let report = match validate(&cfg, &utf8) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: validator error: {e}", path.display()));
                continue;
            }
        };
        if report.issues.num_errors > 0 {
            let summary = report
                .issues
                .messages
                .iter()
                .filter(|m| m.severity == 0)
                .map(|m| format!("{}: {}", m.code, m.message))
                .collect::<Vec<_>>()
                .join("; ");
            failures.push(format!("{}: {summary}", path.display()));
        } else {
            eprintln!("OK {}", path.display());
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        panic!(
            "{} of {} 0.x assets failed validation",
            failures.len(),
            vrm_files.len()
        );
    }
}

#[test]
fn applicable_texture_sweeps_emit_v0() {
    for sub in [
        "emit-emissive-sweep",
        "emit-shade-multiply-texture-sweep",
        "emit-matcap-texture-sweep",
        "emit-outline-width-multiply-texture-sweep",
        "emit-pbr-textures-sweep",
        "emit-first-person-sweep",
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = tmp.path().join(sub);
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
            .args([
                sub,
                "--spec-version",
                "0.x",
                "--output-dir",
                out.to_str().unwrap(),
            ])
            .status()
            .expect("run sweep");
        assert!(status.success(), "{sub} --spec-version 0.x must exit 0");
        let has_vrm = std::fs::read_dir(&out)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().is_some_and(|x| x == "vrm"));
        assert!(has_vrm, "{sub} must emit at least one .vrm at 0.x");
    }
}

#[test]
fn notapplicable_sweeps_reject_v0() {
    for (sub, reason) in [
        (
            "emit-shading-shift-texture-sweep",
            "ShadingShiftTextureV1Only",
        ),
        (
            "emit-rim-multiply-texture-sweep",
            "RimMultiplyTextureV1Only",
        ),
        ("emit-texture-transform-sweep", "KhrTextureTransformV1Only"),
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = tmp.path().join(sub);
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
            .args([
                sub,
                "--spec-version",
                "0.x",
                "--output-dir",
                out.to_str().unwrap(),
            ])
            .output()
            .expect("run sweep");
        assert!(
            !output.status.success(),
            "{sub} must reject --spec-version 0.x"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(reason),
            "{sub} rejection must name {reason}; got: {stderr}"
        );
    }
}
