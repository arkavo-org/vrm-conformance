use vrm_asset_generator::spring_bone::{spring_bone_basic_sweep, SpringBoneParams};

#[test]
fn sweep_has_expected_size_and_unique_ids() {
    let assets = spring_bone_basic_sweep();
    // Axes (per-axis count minus baseline): joints (3) + segment (3) +
    // stiffness (4) + drag (4) + gravity (3) + 1 baseline = 18. Bound the
    // assertion loosely to allow future per-axis expansion without churn.
    assert!(
        assets.len() >= 15 && assets.len() <= 30,
        "sweep should be ~20 assets, got {}",
        assets.len()
    );

    let mut ids: Vec<&str> = assets.iter().map(|p| p.id.as_str()).collect();
    ids.sort();
    let len = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), len, "duplicate IDs in spring-bone sweep");

    assert!(assets.iter().any(|p| p.id == "springbone_default"));
}

#[test]
fn sweep_baseline_uses_default_params() {
    let assets = spring_bone_basic_sweep();
    let baseline = assets
        .iter()
        .find(|p| p.id == "springbone_default")
        .expect("baseline must exist");
    let reference = SpringBoneParams::defaults("springbone_default");
    assert_eq!(baseline.joint_count, reference.joint_count);
    assert_eq!(baseline.segment_length_m, reference.segment_length_m);
    assert_eq!(baseline.stiffness, reference.stiffness);
    assert_eq!(baseline.drag_force, reference.drag_force);
    assert_eq!(baseline.gravity_power, reference.gravity_power);
}

#[test]
fn sweep_cells_carry_expected_param_values() {
    let assets = spring_bone_basic_sweep();
    let find = |id: &str| {
        assets
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("missing asset: {id}"))
    };

    assert_eq!(find("springbone_joints_2").joint_count, 2);
    assert_eq!(find("springbone_joints_16").joint_count, 16);
    assert!((find("springbone_segment_0p02").segment_length_m - 0.02).abs() < 1e-6);
    assert!((find("springbone_segment_0p2").segment_length_m - 0.20).abs() < 1e-6);
    assert_eq!(find("springbone_stiffness_0").stiffness, 0.0);
    assert_eq!(find("springbone_stiffness_1").stiffness, 1.0);
    assert_eq!(find("springbone_drag_0").drag_force, 0.0);
    assert_eq!(find("springbone_drag_1").drag_force, 1.0);
    // Gravity magnitude sweep retuned for post-VMK-0.15.1 spec-correct
    // gravity scale (~12× stronger than 0.14.0). New range: {0.0, 0.02,
    // 0.05, 0.10, 0.20}.
    assert_eq!(find("springbone_gravity_0").gravity_power, 0.0);
    assert!((find("springbone_gravity_0p2").gravity_power - 0.20).abs() < 1e-6);
}

#[test]
fn sweep_variants_change_exactly_one_axis_from_baseline() {
    let assets = spring_bone_basic_sweep();
    let baseline = SpringBoneParams::defaults("springbone_default");

    for p in &assets {
        if p.id == "springbone_default" {
            continue;
        }
        let diffs = [
            (p.joint_count != baseline.joint_count) as u8,
            ((p.segment_length_m - baseline.segment_length_m).abs() > 1e-6) as u8,
            ((p.stiffness - baseline.stiffness).abs() > 1e-6) as u8,
            ((p.drag_force - baseline.drag_force).abs() > 1e-6) as u8,
            ((p.gravity_power - baseline.gravity_power).abs() > 1e-6) as u8,
        ];
        let n_changed: u8 = diffs.iter().sum();
        assert_eq!(
            n_changed, 1,
            "variant {} should change exactly one axis from baseline; diff vector: {:?}",
            p.id, diffs
        );
    }
}
