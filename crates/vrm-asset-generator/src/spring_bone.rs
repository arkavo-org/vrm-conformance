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
