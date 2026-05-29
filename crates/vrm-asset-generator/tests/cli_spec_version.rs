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

#[test]
fn springbone_sweep_v0_emits_secondary_animation() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sb0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-springbone-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run");
    assert!(status.success());
    let any_sa = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "vrm"))
        .any(|e| {
            String::from_utf8_lossy(&std::fs::read(e.path()).unwrap())
                .contains("secondaryAnimation")
        });
    assert!(
        any_sa,
        "0.x spring-bone sweep assets must carry secondaryAnimation"
    );
}

#[test]
fn springbone_swing_sweep_v0_emits_secondary_animation_and_animation_block() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sbswing0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-springbone-swing-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run");
    assert!(status.success());
    let entries: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    let any_sa = entries
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "vrm"))
        .any(|p| {
            String::from_utf8_lossy(&std::fs::read(p).unwrap()).contains("secondaryAnimation")
        });
    assert!(any_sa, "0.x swing sweep .vrm must carry secondaryAnimation");
    let any_anim = entries
        .iter()
        .filter(|p| p.to_string_lossy().ends_with(".test.yaml"))
        .any(|p| std::fs::read_to_string(p).unwrap().contains("animation"));
    assert!(
        any_anim,
        "0.x swing sweep .test.yaml must carry animation block"
    );
}

#[test]
fn extended_collider_sweep_rejects_v0() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ext0x");
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-springbone-extended-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !o.status.success(),
        "extended-collider sweep must reject 0.x"
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("ExtendedCollidersV1Only"));
}

#[test]
fn collider_sweep_v0_emits_only_sphere_cells() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("coll0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-springbone-collider-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    // No emitted asset id should contain "capsule".
    let any_capsule = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().contains("capsule"));
    assert!(!any_capsule, "0.x collider sweep must skip capsule cells");
    // And at least one sphere cell WAS emitted with a secondaryAnimation.
    let any_sa = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "vrm"))
        .any(|e| {
            String::from_utf8_lossy(&std::fs::read(e.path()).unwrap())
                .contains("secondaryAnimation")
        });
    assert!(
        any_sa,
        "0.x collider sweep must emit at least one sphere cell with secondaryAnimation"
    );
}

#[test]
fn gravity_dir_and_coupling_sweeps_emit_v0() {
    for sub in [
        "emit-springbone-gravity-dir-sweep",
        "emit-springbone-coupling-sweep",
    ] {
        let tmp = tempfile::tempdir().unwrap();
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
            .unwrap();
        assert!(status.success(), "{sub} 0.x must exit 0");
        let any_sa = std::fs::read_dir(&out)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "vrm"))
            .any(|e| {
                String::from_utf8_lossy(&std::fs::read(e.path()).unwrap())
                    .contains("secondaryAnimation")
            });
        assert!(any_sa, "{sub} 0.x must emit secondaryAnimation");
    }
}

#[test]
fn taper_sweep_rejects_v0() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("taper0x");
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-springbone-taper-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("PerJointStiffnessV1Only"));
}

#[test]
#[ignore = "requires .tools/vrm-validator-cli"]
fn springbone_multichain_v0_passes_validator() {
    use camino::Utf8PathBuf;
    use vrm_asset_generator::emit::emit_with_sidecars_spring_bone_multichain_v0;
    use vrm_asset_generator::sweep::spring_bone_multichain_sweep;
    use vrm_validator_wrap::{validate, ValidatorConfig};

    let cfg = match ValidatorConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "SKIP: validator shim not reachable ({e}). Set VRM_VALIDATOR_BIN to an absolute path \
                 or run scripts/install-validator.sh from the workspace root."
            );
            return;
        }
    };

    // Use the first variant of the multichain sweep.
    let variants = spring_bone_multichain_sweep();
    let (mtoon, scene) = &variants[0];

    let tmp = tempfile::tempdir().expect("tempdir");
    let stem = Utf8PathBuf::from_path_buf(tmp.path().join("mc_v0_val")).expect("utf8 path");
    emit_with_sidecars_spring_bone_multichain_v0(mtoon, scene, &stem)
        .expect("emission must succeed");

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
            "VRM 0.x multichain asset has {} validator errors: {summary}",
            report.issues.num_errors
        );
    }
    eprintln!(
        "springbone_multichain_v0_passes_validator: 0 errors, {} warnings",
        report.issues.num_warnings
    );
}

#[test]
fn springbone_multichain_sweep_v0_exits_0_and_emits_secondary_animation() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("mc0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-springbone-multichain-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run emit-springbone-multichain-sweep");
    assert!(
        status.success(),
        "emit-springbone-multichain-sweep --spec-version 0.x must exit 0"
    );
    // At least one .vrm must exist and contain "secondaryAnimation".
    let any_sa = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "vrm"))
        .any(|e| {
            String::from_utf8_lossy(&std::fs::read(e.path()).unwrap())
                .contains("secondaryAnimation")
        });
    assert!(
        any_sa,
        "0.x multichain sweep must emit at least one .vrm with secondaryAnimation"
    );
}

#[test]
fn sequence_sweep_v0_emits_render_sequence_block() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("seq0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-sequence-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let any_seq = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".test.yaml"))
        .any(|e| {
            std::fs::read_to_string(e.path())
                .unwrap()
                .contains("render_sequence")
        });
    assert!(
        any_seq,
        "0.x sequence sweep .test.yaml must carry render_sequence block"
    );
}
