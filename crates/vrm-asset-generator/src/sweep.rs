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
