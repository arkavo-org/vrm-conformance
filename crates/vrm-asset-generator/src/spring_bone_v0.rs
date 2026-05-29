//! VRM 0.x `secondaryAnimation` emit. Mirrors `spring_bone.rs`'s topology but
//! produces the 0.0 schema (boneGroups + colliderGroups) instead of
//! VRMC_springBone. Shares `SpringBoneParams` so the sweep registries are
//! version-agnostic.

use crate::spring_bone::SpringBoneParams;
use serde_json::{json, Value};

/// Build the VRM 0.x `secondaryAnimation` object for one spring chain.
/// `first_bone_node` is the glTF node index the chain hangs from.
pub fn build_secondary_animation(params: &SpringBoneParams, first_bone_node: usize) -> Value {
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
            "colliderGroups": []
        }],
        "colliderGroups": []
    })
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
}
