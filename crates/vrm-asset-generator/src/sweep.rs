//! MToon basic parameter sweep: ~50 assets, one per axis-value pair, all
//! other parameters held at `MToonParams::defaults()`.

use crate::params::{AlphaMode, MToonParams, OutlineWidthMode};
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

    // alphaMode × alphaCutoff (MASK) and transparentWithZWrite (BLEND).
    // Exercises VMK#264 (MToon shader discard_fragment defeats A2C on MASK)
    // by routing the renderer through its MASK pipeline; cross-renderer SSIM
    // against UniVRM/three-vrm/godot-vrm surfaces any divergence. The default
    // `mtoon_default` already covers the OPAQUE baseline.
    for cutoff in [0.25_f32, 0.5, 0.75] {
        let mut p = MToonParams::defaults(format!("mtoon_alpha_mask_cutoff_{}", fmt_num(cutoff)));
        p.alpha_mode = AlphaMode::Mask;
        p.alpha_cutoff = cutoff;
        // A semi-transparent base color so MASK has something to clip.
        p.base_color_factor = [1.0, 1.0, 1.0, cutoff];
        out.push(p);
    }
    for z in [false, true] {
        let mut p = MToonParams::defaults(format!("mtoon_alpha_blend_zwrite_{z}"));
        p.alpha_mode = AlphaMode::Blend;
        p.transparent_with_z_write = z;
        // Half-transparent base color for BLEND so the path actually composites.
        p.base_color_factor = [1.0, 1.0, 1.0, 0.5];
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

/// 7-variant per-joint taper sweep: 4 stiffness shapes + 3 drag shapes.
/// All variants use joint_count=4 and set exactly one per-joint vector so
/// regression regressions are unconfounded on the taper axis.
pub fn spring_bone_taper_sweep() -> Vec<SpringBoneParams> {
    let mut out = Vec::with_capacity(7);
    let n: u32 = 4; // joint_count used throughout this sweep

    // Stiffness tapers (4 shapes):
    let stiffness_shapes: [(&str, Vec<f32>); 4] = [
        ("stiffness_flat", vec![0.5, 0.5, 0.5, 0.5]),
        ("stiffness_high_to_low", vec![1.0, 0.7, 0.4, 0.1]),
        ("stiffness_low_to_high", vec![0.1, 0.4, 0.7, 1.0]),
        ("stiffness_expdecay", vec![1.0, 0.5, 0.25, 0.125]),
    ];
    for (suffix, vec) in stiffness_shapes {
        let mut p = SpringBoneParams::defaults(format!("springbone_taper_{suffix}"));
        p.joint_count = n;
        p.stiffness_per_joint = Some(vec);
        out.push(p);
    }

    // Drag tapers (3 shapes — flat omitted since it would duplicate the stiffness_flat baseline):
    let drag_shapes: [(&str, Vec<f32>); 3] = [
        ("drag_flat", vec![0.5, 0.5, 0.5, 0.5]),
        ("drag_high_to_low", vec![1.0, 0.7, 0.4, 0.1]),
        ("drag_expdecay", vec![1.0, 0.5, 0.25, 0.125]),
    ];
    for (suffix, vec) in drag_shapes {
        let mut p = SpringBoneParams::defaults(format!("springbone_taper_{suffix}"));
        p.joint_count = n;
        p.drag_force_per_joint = Some(vec);
        out.push(p);
    }

    out
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

/// 24-variant Cartesian sweep: 2 shapes × 4 offset_x values × 3 radii.
///
/// **Why lateral X offsets, not Y offsets.** An earlier draft of this sweep
/// varied `offset: [0, y, 0]` along the chain axis. With the chain anchored
/// at the head and hanging straight down through (x=0, z=0), a collider
/// placed on that same vertical axis sits at distance zero from every chain
/// joint center — there is no off-axis component, so the spring system has
/// no lateral force to apply during settle. Every variant produced
/// byte-identical PNGs (chain falls straight through collider center).
///
/// Lateral X offsets break the symmetry: the chain's straight-down pose
/// either contacts the collider sphere (small `|x|`, large radius) and
/// must bend around it, or misses it entirely (large `|x|`, small radius).
/// The 4 offsets here span both regimes so the sweep covers contact and
/// no-contact cases per radius.
///
/// `offset_y` is held at -0.10 m (middle of a 0.20 m chain's vertical span)
/// so the collider intersects the chain's vertical reach.
pub fn spring_bone_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(24);

    let offsets_x = [-0.05_f32, -0.02, 0.02, 0.05];
    let radii = [0.03_f32, 0.05, 0.10];
    let fixed_y = -0.10_f32;

    for shape_kind in ["sphere", "capsule"].iter() {
        for &off_x in offsets_x.iter() {
            for &radius in radii.iter() {
                let id = format!(
                    "springbone_collider_{}_x{}_r{}",
                    shape_kind,
                    fmt_signed(off_x),
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
                    offset: [off_x, fixed_y, 0.0],
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

/// 18-variant multi-chain sweep: 3 chain counts × 2 spacings × 3 sharing modes.
///
/// - Chain counts: 2, 3, 5
/// - Spacing: 0.02 m, 0.05 m (encoded in the ID; emit.rs uses a fixed 0.05 m radial
///   spacing at emit time — see docs/findings.md phase-6 limitation note)
/// - Sharing modes: share_all (single collider group referenced by all chains),
///   share_none (per-chain group, each pointing at the same collider), share_alt
///   (two groups, chains alternate between them)
///
/// Each variant carries one trivial sphere collider so the sharing semantics are
/// non-vacuous (the groups reference a real collider even if it is minimal).
pub fn spring_bone_multichain_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(18);

    let chain_counts: [u32; 3] = [2, 3, 5];
    let spacings_m: [f32; 2] = [0.02, 0.05];
    let sharing_modes = ["share_all", "share_none", "share_alt"];

    for &cc in chain_counts.iter() {
        for &sp in spacings_m.iter() {
            for &mode in sharing_modes.iter() {
                let sp_str = fmt_num(sp as f64);
                let id = format!("springbone_multichain_n{cc}_sp{sp_str}_{mode}");
                let scene = build_multichain_scene(&id, cc, mode);
                out.push((MToonParams::defaults(&id), scene));
            }
        }
    }
    out
}

fn build_multichain_scene(id: &str, chain_count: u32, sharing_mode: &str) -> SpringBoneSceneParams {
    let springs: Vec<SpringBoneParams> = (0..chain_count)
        .map(|i| SpringBoneParams::defaults(format!("{id}_chain_{i}")))
        .collect();

    // One sphere collider lateral to the chain axis so the group-sharing axis
    // is non-vacuous AND produces visual signal (chain bends around it). An
    // earlier draft used offset=[0,0,0] which placed the collider on every
    // chain's vertical axis; with no off-axis component, settle pose was
    // identical regardless of sharing mode. Lateral offset at y=-0.10 puts the
    // collider in the chain's lateral path.
    let trivial_collider = ColliderParams {
        shape: ColliderShape::Sphere { radius: 0.04 },
        offset: [0.03, -0.10, 0.0],
        attach: ColliderAttach::Head,
    };

    let (collider_groups, spring_collider_groups) = match sharing_mode {
        "share_all" => {
            // Single group; every chain references it.
            let groups = vec![ColliderGroupParams {
                name: "shared".into(),
                collider_indices: vec![0],
            }];
            let sgs = (0..chain_count).map(|_| vec![0_usize]).collect();
            (groups, sgs)
        }
        "share_none" => {
            // N groups, each pointed at the same single collider; one group per chain.
            let groups = (0..chain_count)
                .map(|i| ColliderGroupParams {
                    name: format!("g{i}"),
                    collider_indices: vec![0],
                })
                .collect();
            let sgs = (0..chain_count).map(|i| vec![i as usize]).collect();
            (groups, sgs)
        }
        "share_alt" => {
            // 2 groups; chains alternate which group they reference.
            let groups = vec![
                ColliderGroupParams {
                    name: "even".into(),
                    collider_indices: vec![0],
                },
                ColliderGroupParams {
                    name: "odd".into(),
                    collider_indices: vec![0],
                },
            ];
            let sgs = (0..chain_count).map(|i| vec![(i as usize) % 2]).collect();
            (groups, sgs)
        }
        _ => unreachable!("unknown sharing mode {sharing_mode}"),
    };

    SpringBoneSceneParams {
        springs,
        colliders: vec![trivial_collider],
        collider_groups,
        spring_collider_groups,
    }
}

pub fn vrma_expression_sweep() -> Vec<crate::vrma_params::VrmaExpressionParams> {
    use crate::vrma_params::VrmaExpressionParams;

    let presets = [
        "happy", "angry", "sad", "relaxed", "surprised",
        "aa", "ih", "ou", "ee", "blink",
    ];
    let custom = ["smug", "drowsy"];

    presets
        .iter()
        .map(|n| VrmaExpressionParams {
            id: format!("vrma_expression_preset_{n}"),
            expression_name: (*n).into(),
            is_preset: true,
            duration_s: 1.0,
        })
        .chain(custom.iter().map(|n| VrmaExpressionParams {
            id: format!("vrma_expression_custom_{n}"),
            expression_name: (*n).into(),
            is_preset: false,
            duration_s: 1.0,
        }))
        .collect()
}

pub fn vrma_humanoid_sweep() -> Vec<crate::vrma_params::VrmaHumanoidParams> {
    use crate::vrma_params::{RotationAxis, VrmaHumanoidParams};

    let entries: Vec<(&str, &str, RotationAxis, f32)> = vec![
        ("vrma_humanoid_hips_yaw_15", "hips", RotationAxis::Y, 15.0),
        ("vrma_humanoid_spine_yaw_30", "spine", RotationAxis::Y, 30.0),
        ("vrma_humanoid_head_yaw_45", "head", RotationAxis::Y, 45.0),
        ("vrma_humanoid_head_pitch_20", "head", RotationAxis::X, 20.0),
        ("vrma_humanoid_head_roll_15", "head", RotationAxis::Z, 15.0),
        (
            "vrma_humanoid_l_upperarm_yaw",
            "leftUpperArm",
            RotationAxis::Y,
            60.0,
        ),
        (
            "vrma_humanoid_l_upperarm_pitch",
            "leftUpperArm",
            RotationAxis::X,
            60.0,
        ),
        (
            "vrma_humanoid_l_upperarm_roll",
            "leftUpperArm",
            RotationAxis::Z,
            30.0,
        ),
        (
            "vrma_humanoid_r_upperarm_yaw",
            "rightUpperArm",
            RotationAxis::Y,
            -60.0,
        ),
        (
            "vrma_humanoid_r_upperarm_pitch",
            "rightUpperArm",
            RotationAxis::X,
            60.0,
        ),
        (
            "vrma_humanoid_r_upperarm_roll",
            "rightUpperArm",
            RotationAxis::Z,
            30.0,
        ),
        (
            "vrma_humanoid_l_upperleg_pitch",
            "leftUpperLeg",
            RotationAxis::X,
            45.0,
        ),
        (
            "vrma_humanoid_l_upperleg_yaw",
            "leftUpperLeg",
            RotationAxis::Y,
            15.0,
        ),
        (
            "vrma_humanoid_l_lowerleg_pitch",
            "leftLowerLeg",
            RotationAxis::X,
            60.0,
        ),
        (
            "vrma_humanoid_neck_yaw_30",
            "neck",
            RotationAxis::Y,
            30.0,
        ),
    ];

    entries
        .into_iter()
        .map(|(id, bone, axis, angle)| VrmaHumanoidParams {
            id: id.into(),
            bone_name: bone.into(),
            axis,
            angle_deg: angle,
            duration_s: 1.0,
        })
        .collect()
}

#[cfg(test)]
mod vrma_expression_sweep_tests {
    use super::*;

    #[test]
    fn expression_sweep_produces_12_variants() {
        let variants = vrma_expression_sweep();
        assert_eq!(variants.len(), 12, "10 presets + 2 custom = 12");
    }

    #[test]
    fn expression_sweep_unique_ids() {
        let variants = vrma_expression_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 12);
    }

    #[test]
    fn expression_sweep_preset_flag_correct() {
        let variants = vrma_expression_sweep();
        for v in &variants {
            if v.id.contains("_preset_") {
                assert!(v.is_preset, "{} should be preset", v.id);
            } else {
                assert!(!v.is_preset, "{} should be custom", v.id);
            }
        }
    }

    #[test]
    fn expression_sweep_all_duration_1s() {
        let variants = vrma_expression_sweep();
        for v in &variants {
            assert!((v.duration_s - 1.0).abs() < 1e-6, "{}: duration should be 1.0", v.id);
        }
    }
}

#[cfg(test)]
mod taper_sweep_tests {
    use super::*;

    #[test]
    fn taper_sweep_produces_7_variants() {
        // 4 stiffness shapes + 3 drag shapes = 7.
        let variants = spring_bone_taper_sweep();
        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn taper_sweep_each_variant_has_a_per_joint_vector_set() {
        let variants = spring_bone_taper_sweep();
        for p in &variants {
            let has_per_joint = p.stiffness_per_joint.is_some()
                || p.drag_force_per_joint.is_some()
                || p.gravity_power_per_joint.is_some()
                || p.hit_radius_per_joint.is_some();
            assert!(
                has_per_joint,
                "{}: must have at least one per-joint vector",
                p.id
            );
        }
    }

    #[test]
    fn taper_sweep_vector_lengths_match_joint_count() {
        let variants = spring_bone_taper_sweep();
        for p in &variants {
            if let Some(v) = &p.stiffness_per_joint {
                assert_eq!(
                    v.len() as u32,
                    p.joint_count,
                    "{}: stiffness vector len mismatch",
                    p.id
                );
            }
            if let Some(v) = &p.drag_force_per_joint {
                assert_eq!(
                    v.len() as u32,
                    p.joint_count,
                    "{}: drag vector len mismatch",
                    p.id
                );
            }
        }
    }

    #[test]
    fn taper_sweep_unique_ids() {
        let variants = spring_bone_taper_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 7);
    }
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

#[cfg(test)]
mod multichain_sweep_tests {
    use super::*;

    #[test]
    fn multichain_sweep_produces_18_variants() {
        // 3 chain_count × 2 spacing × 3 sharing = 18.
        let variants = spring_bone_multichain_sweep();
        assert_eq!(variants.len(), 18);
    }

    #[test]
    fn multichain_sweep_each_variant_has_multiple_chains() {
        let variants = spring_bone_multichain_sweep();
        for (_, scene) in &variants {
            assert!(scene.springs.len() >= 2);
            assert!(scene.springs.len() <= 5);
        }
    }

    #[test]
    fn multichain_sweep_unique_ids() {
        let variants = spring_bone_multichain_sweep();
        let ids: std::collections::HashSet<_> =
            variants.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(ids.len(), 18);
    }

    #[test]
    fn multichain_sweep_sharing_modes_actually_share() {
        let variants = spring_bone_multichain_sweep();
        // For sharing="all", every chain references the same collider_group index.
        let all_share: Vec<_> = variants
            .iter()
            .filter(|(m, _)| m.id.contains("share_all"))
            .collect();
        for (_, scene) in all_share {
            let first = &scene.spring_collider_groups[0];
            for sg in &scene.spring_collider_groups {
                assert_eq!(
                    sg, first,
                    "share_all variants must point all chains at the same group"
                );
            }
        }
    }
}
