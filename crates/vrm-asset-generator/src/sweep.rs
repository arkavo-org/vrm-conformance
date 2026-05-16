//! MToon basic parameter sweep: ~50 assets, one per axis-value pair, all
//! other parameters held at `MToonParams::defaults()`.

use crate::params::{MToonParams, OutlineWidthMode};
use crate::spring_bone::{
    ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape, SpringBoneParams,
    SpringBoneSceneParams,
};

pub fn mtoon_basic_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Baseline.
    out.push(MToonParams::defaults("mtoon_default"));

    // shadingShiftFactor: -1.0 .. 1.0
    for v in [-1.0, -0.8, -0.5, -0.2, 0.0, 0.2, 0.5, 0.8, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_shadingShift_{}", fmt_num(v)));
        p.shading_shift_factor = v;
        out.push(p);
    }

    // shadingToonyFactor: 0.0 .. 1.0
    for v in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_shadingToony_{}", fmt_num(v)));
        p.shading_toony_factor = v;
        out.push(p);
    }

    // giEqualizationFactor
    for v in [0.0, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_giEqualization_{}", fmt_num(v)));
        p.gi_equalization_factor = v;
        out.push(p);
    }

    // rimLightingMixFactor (the three-vrm v3.5.0 regression source)
    for v in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_rimLightingMix_{}", fmt_num(v)));
        p.rim_lighting_mix_factor = v;
        // Pair with a non-zero rim color so the parameter actually matters
        // visually.
        p.parametric_rim_color_factor = [1.0, 0.5, 0.0];
        p.parametric_rim_fresnel_power_factor = 5.0;
        out.push(p);
    }

    // Outline mode × width
    for &mode in &[
        OutlineWidthMode::None,
        OutlineWidthMode::WorldCoordinates,
        OutlineWidthMode::ScreenCoordinates,
    ] {
        let mode_str = match mode {
            OutlineWidthMode::None => "none",
            OutlineWidthMode::WorldCoordinates => "world",
            OutlineWidthMode::ScreenCoordinates => "screen",
        };
        if matches!(mode, OutlineWidthMode::None) {
            // Single None baseline; width is meaningless when outlines are off.
            let mut p = MToonParams::defaults("mtoon_outline_none");
            p.outline_width_mode = mode;
            out.push(p);
            continue;
        }
        for &w in &[0.01_f32, 0.03, 0.05, 0.10] {
            let mut p =
                MToonParams::defaults(format!("mtoon_outline_{mode_str}_{w}", w = fmt_num(w)));
            p.outline_width_mode = mode;
            p.outline_width_factor = w;
            p.outline_color_factor = [0.0, 0.0, 0.0];
            out.push(p);
        }
    }

    // renderQueueOffsetNumber
    for v in [-9_i32, 0, 9] {
        let mut p = MToonParams::defaults(format!("mtoon_renderQueueOffset_{v}"));
        p.render_queue_offset_number = v;
        out.push(p);
    }

    // doubleSided
    for v in [false, true] {
        let mut p = MToonParams::defaults(format!("mtoon_doubleSided_{v}"));
        p.double_sided = v;
        out.push(p);
    }

    out
}

fn fmt_num<T: std::fmt::Display + Copy + PartialOrd + Default>(v: T) -> String
where
    f64: From<T>,
{
    let f = f64::from(v);
    let s = format!("{f:.3}").replace('.', "p").replace('-', "neg");
    s.trim_end_matches('0').trim_end_matches('p').to_string()
}

fn fmt_signed(v: f32) -> String {
    if v < 0.0 {
        format!("neg{}", fmt_num(-v))
    } else {
        fmt_num(v)
    }
}

/// 4-variant one-axis sweep over `gravity_dir`, holding all other
/// `SpringBoneParams` at defaults. Directions: -Y (default), +Y (anti-gravity),
/// +X (sideways), and a 45° oblique (+0.7, -0.7, 0). Each variant changes only
/// the gravity direction so adapter regressions on this axis are unconfounded.
pub fn spring_bone_gravity_dir_sweep() -> Vec<SpringBoneParams> {
    let directions = [
        ("default", [0.0_f32, -1.0, 0.0]),
        ("anti", [0.0, 1.0, 0.0]),
        ("sideways", [1.0, 0.0, 0.0]),
        ("oblique", [0.7, -0.7, 0.0]),
    ];

    directions
        .iter()
        .map(|(name, dir)| {
            let mut p = SpringBoneParams::defaults(format!("springbone_gravity_dir_{name}"));
            p.gravity_dir = *dir;
            p
        })
        .collect()
}

/// 24-variant Cartesian sweep: 2 shapes × 4 offset_y values × 3 radii.
///
/// Unlike one-axis-at-a-time sweeps, the collider sweep is Cartesian because
/// collision response is not separable on a single axis at this scale —
/// varying shape, offset, and radius together exercises different contact
/// geometries.
///
/// Chain hangs from head (y ≈ 1.36 m downward). offset_y values in local
/// head space: negative = below head = into the chain path.
pub fn spring_bone_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(24);

    let offsets = [-0.08_f32, -0.04, 0.0, 0.04];
    let radii = [0.03_f32, 0.05, 0.10];

    for shape_kind in ["sphere", "capsule"].iter() {
        for &off_y in offsets.iter() {
            for &radius in radii.iter() {
                let id = format!(
                    "springbone_collider_{}_y{}_r{}",
                    shape_kind,
                    fmt_signed(off_y),
                    fmt_num(radius),
                );
                let shape = match *shape_kind {
                    "sphere" => ColliderShape::Sphere { radius },
                    "capsule" => ColliderShape::Capsule {
                        radius,
                        tail_offset: [0.0, -0.05, 0.0],
                    },
                    _ => unreachable!(),
                };
                let collider = ColliderParams {
                    shape,
                    offset: [0.0, off_y, 0.0],
                    attach: ColliderAttach::Head,
                };
                let scene = SpringBoneSceneParams {
                    springs: vec![SpringBoneParams::defaults(&id)],
                    colliders: vec![collider],
                    collider_groups: vec![ColliderGroupParams {
                        name: "head_g".into(),
                        collider_indices: vec![0],
                    }],
                    spring_collider_groups: vec![vec![0]],
                };
                let mtoon = MToonParams::defaults(&id);
                out.push((mtoon, scene));
            }
        }
    }

    out
}

/// Build a (ColliderShape, offset) pair for the extended collider at the
/// given shape name and placement index (0=tight, 1=medium, 2=loose).
fn make_extended_shape_with_placement(shape_name: &str, p_idx: usize) -> (ColliderShape, [f32; 3]) {
    match shape_name {
        "plane" => {
            // Plane normal stays +Y; vary the Y offset to place at different depths.
            let offsets = [-0.04_f32, -0.08, -0.15];
            (
                ColliderShape::Plane {
                    normal: [0.0, 1.0, 0.0],
                },
                [0.0, offsets[p_idx], 0.0],
            )
        }
        "isphere" => {
            // Inside sphere: vary radius (tight=small, loose=large).
            let radii = [0.10_f32, 0.20, 0.40];
            (
                ColliderShape::InsideSphere {
                    radius: radii[p_idx],
                },
                [0.0, -0.10, 0.0],
            )
        }
        "icaps" => {
            // Inside capsule: vary radius.
            let radii = [0.10_f32, 0.20, 0.40];
            (
                ColliderShape::InsideCapsule {
                    radius: radii[p_idx],
                    tail_offset: [0.0, 0.30, 0.0],
                },
                [0.0, -0.10, 0.0],
            )
        }
        _ => unreachable!(),
    }
}

fn build_extended_scene(
    id: &str,
    shape: ColliderShape,
    offset: [f32; 3],
    angle_limit: Option<f32>,
) -> SpringBoneSceneParams {
    let mut spring = SpringBoneParams::defaults(id);
    spring.joint_angle_limit_deg = angle_limit;
    let collider = ColliderParams {
        shape,
        offset,
        attach: ColliderAttach::Head,
    };
    SpringBoneSceneParams {
        springs: vec![spring],
        colliders: vec![collider],
        collider_groups: vec![ColliderGroupParams {
            name: "ext_g".into(),
            collider_indices: vec![0],
        }],
        spring_collider_groups: vec![vec![0]],
    }
}

/// 18-variant extended collider sweep:
/// - First 9: 3 shapes (plane, isphere, icaps) × 3 placements (tight, med, loose), no angle limit.
/// - Second 9: 3 shapes × 3 angle limits (30°, 60°, 90°) at medium placement.
pub fn spring_bone_extended_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(18);
    let shape_names = ["plane", "isphere", "icaps"];
    let placement_keys = ["tight", "med", "loose"];

    // First 9: shape × placement, no angle limit.
    for shape_name in shape_names.iter() {
        for (p_idx, p_key) in placement_keys.iter().enumerate() {
            let id = format!("springbone_extended_{shape_name}_p{p_key}");
            let (shape, offset) = make_extended_shape_with_placement(shape_name, p_idx);
            let scene = build_extended_scene(&id, shape, offset, None);
            out.push((MToonParams::defaults(&id), scene));
        }
    }

    // Second 9: shape × angle limit (30, 60, 90), medium placement.
    for shape_name in shape_names.iter() {
        for &deg in [30.0_f32, 60.0, 90.0].iter() {
            let id = format!("springbone_extended_{shape_name}_anglelimit_{}", deg as i32);
            let (shape, offset) = make_extended_shape_with_placement(shape_name, 1);
            let scene = build_extended_scene(&id, shape, offset, Some(deg));
            out.push((MToonParams::defaults(&id), scene));
        }
    }

    out
}

#[cfg(test)]
mod extended_sweep_tests {
    use super::*;

    #[test]
    fn extended_sweep_produces_18_variants() {
        // 3 shapes × 3 placements = 9 (default angle, no limit)
        // 3 shapes × 3 angle_limits (30, 60, 90) at default placement = 9
        // Total: 18 base variants.
        let variants = spring_bone_extended_collider_sweep();
        assert_eq!(variants.len(), 18);
    }

    #[test]
    fn extended_sweep_unique_names() {
        let variants = spring_bone_extended_collider_sweep();
        let names: std::collections::HashSet<_> =
            variants.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(names.len(), 18);
    }

    #[test]
    fn extended_sweep_angle_limit_variants_actually_set_the_limit() {
        let variants = spring_bone_extended_collider_sweep();
        let limited: Vec<_> = variants
            .iter()
            .filter(|(_, s)| s.springs[0].joint_angle_limit_deg.is_some())
            .collect();
        assert_eq!(limited.len(), 9, "9 variants should carry angle limits");
    }
}

#[cfg(test)]
mod gravity_dir_sweep_tests {
    use super::*;

    #[test]
    fn gravity_dir_sweep_produces_4_variants() {
        let variants = spring_bone_gravity_dir_sweep();
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn gravity_dir_sweep_covers_4_distinct_directions() {
        let variants = spring_bone_gravity_dir_sweep();
        let dirs: std::collections::HashSet<[i32; 3]> = variants
            .iter()
            .map(|p| {
                let g = p.gravity_dir;
                // Multiply by 10 to compare with tolerance via integer hashing.
                [
                    (g[0] * 10.0) as i32,
                    (g[1] * 10.0) as i32,
                    (g[2] * 10.0) as i32,
                ]
            })
            .collect();
        assert_eq!(dirs.len(), 4, "all four directions must be distinct");
    }

    #[test]
    fn gravity_dir_sweep_baseline_first() {
        let variants = spring_bone_gravity_dir_sweep();
        assert_eq!(
            variants[0].gravity_dir,
            [0.0, -1.0, 0.0],
            "first variant should be the baseline -Y direction"
        );
    }

    #[test]
    fn gravity_dir_sweep_includes_anti_sideways_oblique() {
        let variants = spring_bone_gravity_dir_sweep();
        let has_antigravity = variants.iter().any(|p| p.gravity_dir == [0.0, 1.0, 0.0]);
        let has_sideways_x = variants.iter().any(|p| p.gravity_dir == [1.0, 0.0, 0.0]);
        let has_oblique = variants.iter().any(|p| {
            (p.gravity_dir[0] - 0.7).abs() < 1e-6 && (p.gravity_dir[1] - (-0.7)).abs() < 1e-6
        });
        assert!(
            has_antigravity && has_sideways_x && has_oblique,
            "must include anti, sideways, and oblique"
        );
    }

    #[test]
    fn gravity_dir_sweep_uses_unique_ids() {
        let variants = spring_bone_gravity_dir_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 4);
    }
}

#[cfg(test)]
mod collider_sweep_tests {
    use super::*;

    #[test]
    fn collider_sweep_produces_24_variants() {
        let variants = spring_bone_collider_sweep();
        assert_eq!(
            variants.len(),
            24,
            "Cartesian: 2 shapes × 4 offsets × 3 radii = 24"
        );
    }

    #[test]
    fn collider_sweep_variants_are_uniquely_named() {
        let variants = spring_bone_collider_sweep();
        let names: std::collections::HashSet<_> = variants
            .iter()
            .map(|(mtoon, _scene)| mtoon.id.clone())
            .collect();
        assert_eq!(names.len(), 24, "all variant IDs must be unique");
    }

    #[test]
    fn collider_sweep_uses_default_mtoon_constant_across_variants() {
        let variants = spring_bone_collider_sweep();
        let baseline_color = variants[0].0.base_color_factor;
        for (m, _) in &variants {
            assert_eq!(
                m.base_color_factor, baseline_color,
                "MToon must be held constant across collider sweep"
            );
        }
    }

    #[test]
    fn collider_sweep_each_variant_has_exactly_one_collider_group_per_spring() {
        let variants = spring_bone_collider_sweep();
        for (id, scene) in variants.iter().map(|(m, s)| (m.id.clone(), s)) {
            assert_eq!(scene.colliders.len(), 1, "{id}: expected 1 collider");
            assert_eq!(scene.collider_groups.len(), 1, "{id}: expected 1 group");
            assert_eq!(
                scene.spring_collider_groups[0],
                vec![0],
                "{id}: spring must reference group 0"
            );
        }
    }
}
