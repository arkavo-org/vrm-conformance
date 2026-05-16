//! Parameter dictionary for VRMC_springBone scenario generation.
//!
//! v0.1 emits a single named spring with N uniform joints. Per-joint
//! variation (the stiffness / drag / gravity sweeps from handover §5.1)
//! is supported by emitting separate assets each with different uniform
//! values — collider variants and multi-chain assets are deferred to 2D-c.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringBoneParams {
    pub id: String,

    /// Name attached to the VRMC_springBone "spring" object.
    pub spring_name: String,

    /// Number of joints in the chain (≥ 1). Joint 0 is the anchor
    /// attached to the parent bone; subsequent joints trail off.
    pub joint_count: u32,

    /// Length of each segment in meters. Total chain length = `joint_count * segment_length_m`.
    pub segment_length_m: f32,

    /// Per-joint stiffness in [0.0, 1.0]. 0 = no restoration; 1 = rigid.
    pub stiffness: f32,

    /// Per-joint drag force in [0.0, 1.0]. 0 = no damping; 1 = critically damped.
    pub drag_force: f32,

    /// Gravity strength (typical: 0.0 for hair, ~1.0 for ribbons).
    pub gravity_power: f32,

    /// Direction of gravity in world space.
    pub gravity_dir: [f32; 3],

    /// Collision radius for the joint in meters. v0.1 has no colliders, so
    /// this is metadata only; it still travels into the emitted JSON because
    /// renderers may use it for self-collision in the future.
    pub hit_radius: f32,

    /// Per-joint angle limit in degrees (optional). When set, emitted under
    /// `joints[].extensions.VRMC_springBone_extended_collider.angleLimit`.
    /// Uniform across all joints in the chain (spec allows per-joint values;
    /// for our sweep, the value is held constant across the chain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint_angle_limit_deg: Option<f32>,
}

impl SpringBoneParams {
    /// Conservative defaults: 4 joints, 5 cm each, moderate stiffness and
    /// drag, gentle gravity. Reasonable for a single hair strand attached
    /// to the head.
    pub fn defaults(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            spring_name: format!("{id}_chain"),
            joint_count: 4,
            segment_length_m: 0.05,
            stiffness: 0.5,
            drag_force: 0.5,
            gravity_power: 0.5,
            gravity_dir: [0.0, -1.0, 0.0],
            hit_radius: 0.02,
            joint_angle_limit_deg: None,
        }
    }
}

/// One-axis-at-a-time sweep over the spring-bone parameter dictionary, same
/// methodology as `mtoon_basic_sweep`. Each variant changes exactly one
/// parameter from the baseline so a renderer regression can be pinned to a
/// single axis. Cell count is intentionally bounded (~20) so the corpus
/// stays small enough for per-PR diffing.
pub fn spring_bone_basic_sweep() -> Vec<SpringBoneParams> {
    let mut out = Vec::new();
    out.push(SpringBoneParams::defaults("springbone_default"));

    for j in [2_u32, 8, 16] {
        let mut p = SpringBoneParams::defaults(format!("springbone_joints_{j}"));
        p.joint_count = j;
        out.push(p);
    }

    for &len in &[0.02_f32, 0.10, 0.20] {
        let mut p = SpringBoneParams::defaults(format!("springbone_segment_{}", fmt_num(len)));
        p.segment_length_m = len;
        out.push(p);
    }

    for &s in &[0.0_f32, 0.2, 0.8, 1.0] {
        let mut p = SpringBoneParams::defaults(format!("springbone_stiffness_{}", fmt_num(s)));
        p.stiffness = s;
        out.push(p);
    }

    for &d in &[0.0_f32, 0.2, 0.8, 1.0] {
        let mut p = SpringBoneParams::defaults(format!("springbone_drag_{}", fmt_num(d)));
        p.drag_force = d;
        out.push(p);
    }

    for &g in &[0.0_f32, 1.0, 2.0] {
        let mut p = SpringBoneParams::defaults(format!("springbone_gravity_{}", fmt_num(g)));
        p.gravity_power = g;
        out.push(p);
    }

    out
}

pub fn fmt_num(v: f32) -> String {
    let s = format!("{v:.3}").replace('.', "p").replace('-', "neg");
    s.trim_end_matches('0').trim_end_matches('p').to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Capsule { radius: f32, tail_offset: [f32; 3] },
    // Extended (VRMC_springBone_extended_collider-1.0):
    Plane { normal: [f32; 3] },
    InsideSphere { radius: f32 },
    InsideCapsule { radius: f32, tail_offset: [f32; 3] },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderAttach {
    Head,
    NewIntermediateNode { y_offset: f32, z_offset: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColliderParams {
    pub shape: ColliderShape,
    pub offset: [f32; 3],
    pub attach: ColliderAttach,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColliderGroupParams {
    pub name: String,
    pub collider_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringBoneSceneParams {
    /// `Vec` so multi-chain (phase 6) plugs in here without API churn.
    pub springs: Vec<SpringBoneParams>,
    pub colliders: Vec<ColliderParams>,
    pub collider_groups: Vec<ColliderGroupParams>,
    /// Per-spring index list into `collider_groups`.
    pub spring_collider_groups: Vec<Vec<usize>>,
}

impl SpringBoneSceneParams {
    /// Single-chain, no-collider scene constructed from a SpringBoneParams.
    /// Backward-compat for callers that don't need colliders.
    pub fn single_spring(s: SpringBoneParams) -> Self {
        Self {
            springs: vec![s],
            colliders: Vec::new(),
            collider_groups: Vec::new(),
            spring_collider_groups: vec![Vec::new()],
        }
    }
}

#[cfg(test)]
mod extended_collider_tests {
    use super::*;

    #[test]
    fn plane_collider_has_normal_vector() {
        let s = ColliderShape::Plane {
            normal: [0.0, 1.0, 0.0],
        };
        if let ColliderShape::Plane { normal } = s {
            assert_eq!(normal, [0.0, 1.0, 0.0]);
        } else {
            panic!("expected plane");
        }
    }

    #[test]
    fn inside_sphere_collider() {
        let s = ColliderShape::InsideSphere { radius: 0.25 };
        if let ColliderShape::InsideSphere { radius } = s {
            assert!((radius - 0.25).abs() < 1e-6);
        } else {
            panic!("expected inside sphere");
        }
    }

    #[test]
    fn inside_capsule_collider() {
        let s = ColliderShape::InsideCapsule {
            radius: 0.10,
            tail_offset: [0.0, 0.20, 0.0],
        };
        if let ColliderShape::InsideCapsule {
            radius,
            tail_offset,
        } = s
        {
            assert!((radius - 0.10).abs() < 1e-6);
            assert_eq!(tail_offset, [0.0, 0.20, 0.0]);
        } else {
            panic!("expected inside capsule");
        }
    }

    #[test]
    fn spring_bone_params_carries_optional_joint_angle_limit() {
        let mut p = SpringBoneParams::defaults("t");
        assert!(p.joint_angle_limit_deg.is_none());
        p.joint_angle_limit_deg = Some(45.0);
        assert_eq!(p.joint_angle_limit_deg, Some(45.0));
    }
}

#[cfg(test)]
mod collider_tests {
    use super::*;

    #[test]
    fn sphere_collider_default_is_at_origin_with_unit_radius() {
        let c = ColliderParams {
            shape: ColliderShape::Sphere { radius: 0.05 },
            offset: [0.0, 0.0, 0.0],
            attach: ColliderAttach::Head,
        };
        match c.shape {
            ColliderShape::Sphere { radius } => assert!((radius - 0.05).abs() < 1e-6),
            _ => panic!("expected sphere"),
        }
    }

    #[test]
    fn capsule_collider_has_tail_offset() {
        let c = ColliderParams {
            shape: ColliderShape::Capsule {
                radius: 0.03,
                tail_offset: [0.0, -0.1, 0.0],
            },
            offset: [0.0, 0.0, 0.0],
            attach: ColliderAttach::Head,
        };
        match c.shape {
            ColliderShape::Capsule { tail_offset, .. } => {
                assert_eq!(tail_offset, [0.0, -0.1, 0.0]);
            }
            _ => panic!("expected capsule"),
        }
    }

    #[test]
    fn collider_group_holds_indices() {
        let g = ColliderGroupParams {
            name: "head_colliders".into(),
            collider_indices: vec![0, 2, 3],
        };
        assert_eq!(g.collider_indices.len(), 3);
    }

    #[test]
    fn scene_params_aggregates_springs_and_colliders() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test_chain")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g0".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        assert_eq!(scene.springs.len(), 1);
        assert_eq!(scene.colliders.len(), 1);
        assert_eq!(scene.collider_groups.len(), 1);
        assert_eq!(scene.spring_collider_groups[0], vec![0]);
    }
}
