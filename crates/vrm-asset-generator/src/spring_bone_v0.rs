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

/// Build `secondaryAnimation` with N boneGroups, one per chain.
///
/// `springs[i]` drives boneGroup i; `first_bone_nodes[i]` is the root node index of
/// chain i. The top-level `colliderGroups` is always empty — the multichain sweep has
/// no colliders. Each boneGroup uses `colliderGroups: []` accordingly.
///
/// # Panics
/// Panics (debug only) if `springs.len() != first_bone_nodes.len()`.
pub fn build_secondary_animation_multi(
    springs: &[crate::spring_bone::SpringBoneParams],
    first_bone_nodes: &[usize],
) -> Value {
    assert_eq!(
        springs.len(),
        first_bone_nodes.len(),
        "springs and first_bone_nodes must have the same length"
    );

    let bone_groups: Vec<Value> = springs
        .iter()
        .zip(first_bone_nodes.iter())
        .map(|(params, &first_bone_node)| {
            json!({
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
                "colliderGroups": []
            })
        })
        .collect();

    json!({
        "boneGroups": bone_groups,
        "colliderGroups": []
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spring_bone::SpringBoneParams;

    // ── multi-chain builder tests ────────────────────────────────────────────

    #[test]
    fn multi_three_chains_produces_three_bone_groups() {
        let springs: Vec<SpringBoneParams> = (0..3)
            .map(|i| SpringBoneParams::defaults(format!("chain_{i}")))
            .collect();
        let first_bone_nodes: Vec<usize> = vec![10, 20, 30];
        let sa = build_secondary_animation_multi(&springs, &first_bone_nodes);

        let groups = sa["boneGroups"]
            .as_array()
            .expect("boneGroups must be an array");
        assert_eq!(groups.len(), 3, "3 springs → 3 boneGroups");
    }

    #[test]
    fn multi_bone_group_bones_match_first_bone_nodes() {
        let springs: Vec<SpringBoneParams> = (0..3)
            .map(|i| SpringBoneParams::defaults(format!("mc_{i}")))
            .collect();
        let first_bone_nodes: Vec<usize> = vec![5, 11, 99];
        let sa = build_secondary_animation_multi(&springs, &first_bone_nodes);
        let groups = sa["boneGroups"].as_array().unwrap();
        for (i, &expected_node) in first_bone_nodes.iter().enumerate() {
            assert_eq!(
                groups[i]["bones"][0], expected_node,
                "boneGroups[{i}].bones[0] must be first_bone_nodes[{i}]={expected_node}"
            );
        }
    }

    #[test]
    fn multi_each_bone_group_has_stiffiness_key() {
        let springs: Vec<SpringBoneParams> = (0..3)
            .map(|i| SpringBoneParams::defaults(format!("stiff_{i}")))
            .collect();
        let first_bone_nodes: Vec<usize> = vec![1, 2, 3];
        let sa = build_secondary_animation_multi(&springs, &first_bone_nodes);
        let groups = sa["boneGroups"].as_array().unwrap();
        for (i, g) in groups.iter().enumerate() {
            assert!(
                g.get("stiffiness").is_some(),
                "boneGroups[{i}] must have `stiffiness` key (0.x spec typo)"
            );
        }
    }

    #[test]
    fn multi_top_level_collider_groups_empty() {
        let springs: Vec<SpringBoneParams> = vec![SpringBoneParams::defaults("mc_cg")];
        let sa = build_secondary_animation_multi(&springs, &[7]);
        let cgs = sa["colliderGroups"]
            .as_array()
            .expect("colliderGroups array");
        assert_eq!(
            cgs.len(),
            0,
            "multichain has no colliders → empty colliderGroups"
        );
    }

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
