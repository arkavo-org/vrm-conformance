use camino::Utf8PathBuf;
use vrm_asset_generator::{emit::emit_vrm, params::MToonParams};
use vrm_validator_wrap::{validate, ValidatorConfig};

fn config_or_skip() -> Option<ValidatorConfig> {
    match ValidatorConfig::from_env() {
        Ok(c) => Some(c),
        Err(_) => {
            eprintln!("SKIP: validator not installed");
            None
        }
    }
}

#[test]
fn emits_validator_clean_vrm_with_default_mtoon() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("default.vrm")).unwrap();

    let params = MToonParams::defaults("default");
    emit_vrm(&params, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "emitted VRM should have zero validator errors. report: {:#?}",
        report.issues.messages
    );

    // mimeType should be GLB.
    assert_eq!(report.mime_type.as_deref(), Some("model/gltf-binary"));
}

#[test]
fn emits_validator_clean_vrm_with_outline() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("outline.vrm")).unwrap();

    let mut params = MToonParams::defaults("outline_world_05cm");
    params.outline_width_mode = vrm_asset_generator::params::OutlineWidthMode::WorldCoordinates;
    params.outline_width_factor = 0.005;
    params.outline_color_factor = [0.0, 0.0, 0.0];

    emit_vrm(&params, &out).unwrap();
    let report = validate(&cfg, &out).unwrap();
    assert_eq!(report.issues.num_errors, 0);
}
