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

fn fmt_num(v: f32) -> String {
    let s = format!("{v:.3}").replace('.', "p").replace('-', "neg");
    s.trim_end_matches('0').trim_end_matches('p').to_string()
}
