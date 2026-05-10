use camino::Utf8PathBuf;
use vrm_asset_generator::{emit::emit_with_sidecars, params::MToonParams};

#[test]
fn emit_with_sidecars_produces_three_files() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("test_asset")).unwrap();

    emit_with_sidecars(&MToonParams::defaults("test_asset"), &stem).unwrap();

    assert!(stem.with_extension("vrm").exists());
    assert!(stem.with_extension("meta.json").exists());
    assert!(stem.with_extension("test.yaml").exists());
}

#[test]
fn meta_json_contains_parameter_values() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("a")).unwrap();
    let mut params = MToonParams::defaults("a");
    params.shading_shift_factor = -0.5;
    emit_with_sidecars(&params, &stem).unwrap();

    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(stem.with_extension("meta.json")).unwrap())
            .unwrap();
    assert_eq!(meta["params"]["shading_shift_factor"], -0.5);
    assert_eq!(meta["license"], "CC0-1.0");
    assert!(meta["blake3"].as_str().unwrap().starts_with("blake3:"));
}

#[test]
fn test_yaml_round_trips_into_test_plan() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("a")).unwrap();
    emit_with_sidecars(&MToonParams::defaults("a"), &stem).unwrap();

    let yaml = std::fs::read_to_string(stem.with_extension("test.yaml")).unwrap();
    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(plan.id, "a");
    assert!(matches!(
        plan.post_processing.tone_mapping,
        vrm_test_plan::ToneMapping::None
    ));
    assert!(
        !plan.lighting.cast_shadows,
        "MToon math tests must run shadows-off"
    );
}
