//! VRM 0.x `secondaryAnimation` emit. Mirrors `spring_bone.rs`'s topology but
//! produces the 0.0 schema (boneGroups + colliderGroups) instead of
//! VRMC_springBone. Shares `SpringBoneParams` so the sweep registries are
//! version-agnostic.

use crate::spring_bone::SpringBoneParams;
use serde_json::{json, Value};

/// One VRM 0.x sphere collider, already resolved to a glTF node + local offset.
///
/// VRM 0.x supports sphere colliders only (offset + radius). Capsule,
/// InsideSphere, and InsideCapsule have no 0.x form and must be filtered out
/// before calling [`build_secondary_animation_with_colliders`].
pub struct V0SphereCollider {
    pub node: usize,
    pub offset: [f32; 3],
    pub radius: f32,
}

/// Build `secondaryAnimation` with optional sphere colliders.
///
/// Each collider becomes its own `colliderGroup`; the single `boneGroup`
/// references all of them by index (`0..colliders.len()`). Identical to
/// [`build_secondary_animation`] when `colliders` is empty.
pub fn build_secondary_animation_with_colliders(
    params: &SpringBoneParams,
    first_bone_node: usize,
    colliders: &[V0SphereCollider],
) -> Value {
    let collider_group_indices: Vec<Value> = (0..colliders.len()).map(|i| json!(i)).collect();

    let collider_groups: Vec<Value> = colliders
        .iter()
        .map(|c| {
            json!({
                "node": c.node,
                "colliders": [{
                    "offset": {
                        "x": c.offset[0],
                        "y": c.offset[1],
                        "z": c.offset[2]
                    },
                    "radius": c.radius
                }]
            })
        })
        .collect();

    json!({
        "boneGroups": [{
            "comment": params.spring_name,
            "stiffiness": params.stiffness,
            "gravityPower": params.gravity_power,
            "gravityDir": {
                "x": params.gravity_dir[0],
                "y": params.gravity_dir[1],
                "z": params.gravity_dir[2]
            },
            "dragForce": params.drag_force,
            "center": -1,
            "hitRadius": params.hit_radius,
            "bones": [first_bone_node],
            "colliderGroups": collider_group_indices
        }],
        "colliderGroups": collider_groups
    })
}

/// Build the VRM 0.x `secondaryAnimation` object for one spring chain.
/// `first_bone_node` is the glTF node index the chain hangs from.
///
/// Delegates to [`build_secondary_animation_with_colliders`] with no
/// colliders — all Phase B callers remain unaffected.
pub fn build_secondary_animation(params: &SpringBoneParams, first_bone_node: usize) -> Value {
    build_secondary_animation_with_colliders(params, first_bone_node, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spring_bone::SpringBoneParams;

    #[test]
    fn single_chain_has_one_bone_group_with_spec_field_names() {
        let p = SpringBoneParams::defaults("sb_v0_smoke");
        let sa = build_secondary_animation(&p, 1);
        let groups = sa["boneGroups"].as_array().expect("boneGroups array");
        assert_eq!(groups.len(), 1, "one chain → one boneGroup");
        let g = &groups[0];
        assert!(
            g.get("stiffiness").is_some(),
            "must use 0.x `stiffiness` key (spec typo)"
        );
        assert!(g.get("dragForce").is_some());
        assert!(g.get("gravityPower").is_some());
        assert!(g["bones"].as_array().is_some(), "bones index array present");
        assert_eq!(g["bones"][0], 1, "first bone node index threads through");
        assert!(sa["colliderGroups"].as_array().is_some());
    }

    #[test]
    fn no_colliders_delegates_to_empty_collider_groups() {
        let p = SpringBoneParams::defaults("sb_v0_no_coll");
        let sa = build_secondary_animation(&p, 2);
        let cgs = sa["colliderGroups"].as_array().expect("colliderGroups");
        assert_eq!(cgs.len(), 0, "no colliders → empty colliderGroups");
        let bg_cgs = sa["boneGroups"][0]["colliderGroups"]
            .as_array()
            .expect("boneGroups[0].colliderGroups");
        assert_eq!(
            bg_cgs.len(),
            0,
            "boneGroup should reference no collider groups"
        );
    }

    #[test]
    fn one_sphere_collider_wires_bone_group_reference_and_collider_group() {
        let p = SpringBoneParams::defaults("sb_v0_coll");
        let colliders = [V0SphereCollider {
            node: 7,
            offset: [0.1, -0.05, 0.0],
            radius: 0.08,
        }];
        let sa = build_secondary_animation_with_colliders(&p, 3, &colliders);

        // boneGroups[0].colliderGroups must reference index [0]
        let bg_cg_indices = sa["boneGroups"][0]["colliderGroups"]
            .as_array()
            .expect("boneGroups[0].colliderGroups array");
        assert_eq!(bg_cg_indices.len(), 1);
        assert_eq!(bg_cg_indices[0], 0, "single collider → index 0 reference");

        // colliderGroups[0].node
        let cg = &sa["colliderGroups"][0];
        assert_eq!(
            cg["node"], 7,
            "colliderGroups[0].node must match V0SphereCollider.node"
        );

        // colliderGroups[0].colliders[0].radius
        let c = &cg["colliders"][0];
        assert!(
            (c["radius"].as_f64().expect("radius f64") - 0.08_f64).abs() < 1e-6,
            "radius must match"
        );

        // colliderGroups[0].colliders[0].offset.x/y/z
        assert!(
            (c["offset"]["x"].as_f64().expect("offset.x") - 0.1_f64).abs() < 1e-6,
            "offset.x must match"
        );
        assert!(
            (c["offset"]["y"].as_f64().expect("offset.y") - (-0.05_f64)).abs() < 1e-6,
            "offset.y must match"
        );
        assert!(
            (c["offset"]["z"].as_f64().expect("offset.z") - 0.0_f64).abs() < 1e-6,
            "offset.z must match"
        );
    }
}
