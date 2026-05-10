use camino::Utf8PathBuf;
use vrm_asset_generator::{
    emit::emit_vrm_with_spring_bone, params::MToonParams, spring_bone::SpringBoneParams,
};
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
fn emits_validator_clean_vrm_with_default_spring_bone() {
    let Some(cfg) = config_or_skip() else {
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("default_spring.vrm")).unwrap();

    let mtoon = MToonParams::defaults("default_spring");
    let spring = SpringBoneParams::defaults("default_spring");
    emit_vrm_with_spring_bone(&mtoon, &spring, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "spring-bone-bearing VRM should have zero validator errors. report: {:#?}",
        report.issues.messages
    );
    assert_eq!(report.mime_type.as_deref(), Some("model/gltf-binary"));
}

#[test]
fn emits_validator_clean_vrm_with_stiff_spring_bone() {
    let Some(cfg) = config_or_skip() else {
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("stiff_spring.vrm")).unwrap();

    let mtoon = MToonParams::defaults("stiff_spring");
    let mut spring = SpringBoneParams::defaults("stiff_spring");
    spring.stiffness = 1.0;
    spring.drag_force = 0.9;
    spring.gravity_power = 0.0;
    spring.joint_count = 6;

    emit_vrm_with_spring_bone(&mtoon, &spring, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "stiff-variant VRM should validate clean. report: {:#?}",
        report.issues.messages
    );
}
