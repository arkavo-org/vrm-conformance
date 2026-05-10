use camino::Utf8PathBuf;
use vrm_asset_generator::{
    emit::emit_with_sidecars_spring_bone_swing,
    params::MToonParams,
    sidecar::{build_spring_bone_swing_test_plan, build_spring_bone_test_plan},
    spring_bone::SpringBoneParams,
};

#[test]
fn swing_plan_carries_both_physics_and_animation() {
    let mtoon = MToonParams::defaults("swing_plan");
    let plan = build_spring_bone_swing_test_plan(&mtoon, "swing_plan.vrm");

    let physics = plan
        .physics
        .expect("swing plan must keep the physics block");
    assert_eq!(physics.settle_steps, 30);

    let animation = plan
        .animation
        .expect("swing plan must carry an animation block");
    let root = animation
        .root_transform
        .expect("animation must include root_transform");
    assert_eq!(root.translation_start, [0.0, 0.0, 0.0]);
    assert_eq!(root.translation_end, [0.15, 0.0, 0.0]);
    assert_eq!(root.duration_seconds, 0.25);
    assert_eq!(root.fps, 60);

    assert!(plan.spec_section.contains("swing"));
}

#[test]
fn settle_only_plan_has_no_animation_block() {
    let mtoon = MToonParams::defaults("settle_only");
    let plan = build_spring_bone_test_plan(&mtoon, "settle_only.vrm");
    assert!(
        plan.animation.is_none(),
        "settle-only plan must not synthesize an animation block"
    );
    assert!(plan.physics.is_some(), "settle plan still carries physics");
}

#[test]
fn swing_emit_writes_three_files() {
    let dir = tempfile::tempdir().unwrap();
    let stem = Utf8PathBuf::from_path_buf(dir.path().join("swing_emit")).unwrap();

    let mtoon = MToonParams::defaults("swing_emit");
    let spring = SpringBoneParams::defaults("swing_emit");
    emit_with_sidecars_spring_bone_swing(&mtoon, &spring, &stem).unwrap();

    assert!(stem.with_extension("vrm").exists());
    assert!(stem.with_extension("meta.json").exists());
    let yaml_path = stem.with_extension("test.yaml");
    assert!(yaml_path.exists());

    let yaml = std::fs::read_to_string(&yaml_path).unwrap();
    assert!(yaml.contains("animation"), "yaml: {yaml}");
    assert!(yaml.contains("root_transform"));
    assert!(yaml.contains("translation_end"));
    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(&yaml).unwrap();
    assert!(plan.physics.is_some() && plan.animation.is_some());
}
