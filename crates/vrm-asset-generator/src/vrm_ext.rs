//! Builds the JSON fragments for `VRMC_vrm` and `VRMC_materials_mtoon`.
//!
//! Spec references:
//! - VRMC_vrm: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm-1.0
//! - VRMC_materials_mtoon: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_materials_mtoon-1.0

use crate::params::{AlphaMode, MToonParams, OutlineWidthMode};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Build the VRMC_vrm extension JSON.
///
/// `bone_to_node` maps VRM bone names to glTF node indices (from the
/// humanoid skeleton). `mesh_node` is the glTF node index that carries the
/// renderable mesh; it is annotated as `auto` first-person so the
/// `firstPerson.meshAnnotations` array is non-empty (the validator rejects
/// an empty `meshAnnotations` entry).
pub fn vrmc_vrm(
    meta_name: &str,
    bone_to_node: &BTreeMap<String, usize>,
    mesh_node: usize,
) -> Value {
    let human_bones: serde_json::Map<String, Value> = bone_to_node
        .iter()
        .map(|(name, idx)| (name.clone(), json!({ "node": idx })))
        .collect();

    json!({
        "specVersion": "1.0",
        "meta": {
            "name": meta_name,
            "version": "0.1.0",
            "authors": ["arkavo-org/vrm-conformance generator"],
            "licenseUrl": "https://vrm.dev/licenses/1.0/",
            "thirdPartyLicenses": "",
            "avatarPermission": "everyone",
            "allowExcessivelyViolentUsage": false,
            "allowExcessivelySexualUsage": false,
            "commercialUsage": "personalNonProfit",
            "allowPoliticalOrReligiousUsage": false,
            "allowAntisocialOrHateUsage": false,
            "creditNotation": "unnecessary",
            "allowRedistribution": true,
            "modification": "allowModification"
        },
        "humanoid": {
            "humanBones": human_bones
        },
        "firstPerson": {
            "meshAnnotations": [
                { "node": mesh_node, "type": "auto" }
            ]
        },
        "lookAt": {
            "type": "bone",
            "offsetFromHeadBone": [0.0, 0.06, 0.0],
            "rangeMapHorizontalInner": { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapHorizontalOuter": { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapVerticalDown":     { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapVerticalUp":       { "inputMaxValue": 90.0, "outputScale": 10.0 }
        },
        "expressions": {
            "preset": {}
        }
    })
}

/// Build the per-material VRMC_materials_mtoon extension JSON.
pub fn vrmc_materials_mtoon(p: &MToonParams) -> Value {
    let outline_width_mode = match p.outline_width_mode {
        OutlineWidthMode::None => "none",
        OutlineWidthMode::WorldCoordinates => "worldCoordinates",
        OutlineWidthMode::ScreenCoordinates => "screenCoordinates",
    };

    json!({
        "specVersion": "1.0",
        "transparentWithZWrite": p.transparent_with_z_write,
        "renderQueueOffsetNumber": p.render_queue_offset_number,
        "shadeColorFactor": p.shade_color_factor,
        "shadingShiftFactor": p.shading_shift_factor,
        "shadingToonyFactor": p.shading_toony_factor,
        "giEqualizationFactor": p.gi_equalization_factor,
        "matcapFactor": p.matcap_factor,
        "parametricRimColorFactor": p.parametric_rim_color_factor,
        "parametricRimFresnelPowerFactor": p.parametric_rim_fresnel_power_factor,
        "parametricRimLiftFactor": p.parametric_rim_lift_factor,
        "rimLightingMixFactor": p.rim_lighting_mix_factor,
        "outlineWidthMode": outline_width_mode,
        "outlineWidthFactor": p.outline_width_factor,
        "outlineColorFactor": p.outline_color_factor,
        "outlineLightingMixFactor": p.outline_lighting_mix_factor,
        "uvAnimationScrollXSpeedFactor": p.uv_animation_scroll_x_speed_factor,
        "uvAnimationScrollYSpeedFactor": p.uv_animation_scroll_y_speed_factor,
        "uvAnimationRotationSpeedFactor": p.uv_animation_rotation_speed_factor
    })
}

/// glTF base material wrapping MToon. MToon depends on KHR_materials_unlit
/// in the base material so non-MToon-aware viewers fall back gracefully.
pub fn base_material(p: &MToonParams) -> Value {
    let alpha_mode = match p.alpha_mode {
        AlphaMode::Opaque => "OPAQUE",
        AlphaMode::Mask => "MASK",
        AlphaMode::Blend => "BLEND",
    };

    json!({
        "name": p.id,
        "pbrMetallicRoughness": {
            "baseColorFactor": p.base_color_factor,
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9
        },
        "alphaMode": alpha_mode,
        "doubleSided": p.double_sided,
        "extensions": {
            "KHR_materials_unlit": {},
            "VRMC_materials_mtoon": vrmc_materials_mtoon(p)
        }
    })
}

use crate::spring_bone::{ColliderShape, SpringBoneParams, SpringBoneSceneParams};

/// Build a VRMC_springBone extension JSON object given the joint node
/// indices (in chain order, head-to-tail) and the per-spring params.
///
/// Spec reference: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone-1.0
///
/// v0.1 emits one named spring with no colliders. Multi-chain and
/// collider scenarios are out of scope for 2D-a. Delegates to
/// `vrmc_spring_bone_scene` for backward compat.
pub fn vrmc_spring_bone(joint_nodes: &[usize], params: &SpringBoneParams) -> Value {
    let scene = SpringBoneSceneParams::single_spring(params.clone());
    vrmc_spring_bone_scene(joint_nodes, &scene, &[])
}

/// Emit VRMC_springBone extension JSON for a scene with optional colliders.
///
/// `joint_nodes` is in chain order, head-to-tail (phase 1 single-chain only;
/// phase 6 multi-chain will accept Vec<Vec<usize>>).
///
/// `collider_attach_nodes[i]` is the glTF node index that collider `i` is
/// attached to. The caller resolves Head / NewIntermediateNode → node index
/// during emit.
pub fn vrmc_spring_bone_scene(
    joint_nodes: &[usize],
    scene: &SpringBoneSceneParams,
    collider_attach_nodes: &[usize],
) -> Value {
    assert_eq!(
        scene.colliders.len(),
        collider_attach_nodes.len(),
        "collider_attach_nodes must parallel scene.colliders"
    );

    // Phase 1: single spring (scene.springs[0]) for now.
    // Phase 6 will iterate scene.springs.
    let params = &scene.springs[0];
    let joints: Vec<Value> = joint_nodes
        .iter()
        .map(|&node| {
            json!({
                "node": node,
                "hitRadius": params.hit_radius,
                "stiffness": params.stiffness,
                "gravityPower": params.gravity_power,
                "gravityDir": params.gravity_dir,
                "dragForce": params.drag_force,
            })
        })
        .collect();

    let mut spring = json!({
        "name": params.spring_name,
        "joints": joints,
    });

    let spring_groups = scene
        .spring_collider_groups
        .first()
        .cloned()
        .unwrap_or_default();
    if !spring_groups.is_empty() {
        spring["colliderGroups"] = json!(spring_groups);
    }

    let mut out = json!({
        "specVersion": "1.0",
        "springs": [spring],
    });

    if !scene.colliders.is_empty() {
        let colliders: Vec<Value> = scene
            .colliders
            .iter()
            .zip(collider_attach_nodes.iter())
            .map(|(c, &node)| {
                let shape = match &c.shape {
                    ColliderShape::Sphere { radius } => json!({
                        "sphere": {
                            "offset": c.offset,
                            "radius": radius,
                        }
                    }),
                    ColliderShape::Capsule {
                        radius,
                        tail_offset,
                    } => json!({
                        "capsule": {
                            "offset": c.offset,
                            "radius": radius,
                            "tail": tail_offset,
                        }
                    }),
                };
                json!({
                    "node": node,
                    "shape": shape,
                })
            })
            .collect();
        out["colliders"] = json!(colliders);
    }

    if !scene.collider_groups.is_empty() {
        let groups: Vec<Value> = scene
            .collider_groups
            .iter()
            .map(|g| {
                json!({
                    "name": g.name,
                    "colliders": g.collider_indices,
                })
            })
            .collect();
        out["colliderGroups"] = json!(groups);
    }

    out
}

#[cfg(test)]
mod collider_emission_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn no_colliders_omitted_from_json() {
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("c"));
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        assert!(v.get("colliders").is_none());
        assert!(v.get("colliderGroups").is_none());
        assert!(v.get("springs").unwrap().as_array().unwrap()[0]
            .get("colliderGroups")
            .is_none());
    }

    #[test]
    fn sphere_collider_emits_correct_json_shape() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
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
        // attach_nodes parallels colliders by index — for Head attach we pass the head node idx.
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]); // 10 = head node idx
        let colliders = v.get("colliders").unwrap().as_array().unwrap();
        assert_eq!(colliders.len(), 1);
        let c0 = &colliders[0];
        assert_eq!(c0["node"].as_u64().unwrap(), 10);
        let shape = c0["shape"].as_object().unwrap();
        assert!(
            shape.contains_key("sphere"),
            "expected sphere shape, got {shape:?}"
        );
        let sphere = &shape["sphere"];
        assert!((sphere["radius"].as_f64().unwrap() - 0.05).abs() < 1e-6);
        let off = sphere["offset"].as_array().unwrap();
        assert_eq!(off.len(), 3);
        assert!((off[1].as_f64().unwrap() - (-0.04)).abs() < 1e-6);

        let groups = v.get("colliderGroups").unwrap().as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["name"], "g0");
        assert_eq!(groups[0]["colliders"][0], 0);

        let spring = &v["springs"].as_array().unwrap()[0];
        let cg = spring["colliderGroups"].as_array().unwrap();
        assert_eq!(cg.len(), 1);
        assert_eq!(cg[0].as_u64().unwrap(), 0);
    }

    #[test]
    fn capsule_collider_emits_tail_field() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Capsule {
                    radius: 0.03,
                    tail_offset: [0.0, -0.08, 0.0],
                },
                offset: [0.0, 0.0, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g0".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]);
        let shape = v["colliders"][0]["shape"].as_object().unwrap();
        assert!(shape.contains_key("capsule"));
        let cap = &shape["capsule"];
        let tail = cap["tail"].as_array().unwrap();
        assert!((tail[1].as_f64().unwrap() - (-0.08)).abs() < 1e-6);
    }
}
