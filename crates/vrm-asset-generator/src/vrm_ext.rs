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

    let mut material = json!({
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
    });

    // alphaCutoff is meaningful only when alphaMode == MASK per glTF spec.
    // Omit on OPAQUE/BLEND so renderers fall back to the spec default (0.5
    // for MASK; ignored elsewhere) rather than carrying a misleading value.
    if matches!(p.alpha_mode, AlphaMode::Mask) {
        material["alphaCutoff"] = json!(p.alpha_cutoff);
    }

    material
}

use crate::spring_bone::{
    ColliderGroupParams, ColliderShape, SpringBoneParams, SpringBoneSceneParams,
};

/// Return the per-joint value from `per_joint[joint_idx]` if the vector is
/// `Some`, otherwise return `uniform`. Panics (programmer error) when the
/// vector length does not match `joint_count`.
fn joint_value(
    per_joint: &Option<Vec<f32>>,
    uniform: f32,
    joint_idx: usize,
    joint_count: usize,
    field_name: &str,
) -> f32 {
    if let Some(v) = per_joint {
        assert_eq!(
            v.len(),
            joint_count,
            "{}_per_joint length {} must match joint_count {}",
            field_name,
            v.len(),
            joint_count
        );
        v[joint_idx]
    } else {
        uniform
    }
}

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

/// Emit VRMC_springBone extension JSON for N parallel spring chains with optional colliders.
///
/// `joint_nodes_per_chain[c]` is the slice of joint glTF node indices for chain `c`,
/// in head-to-tail order. `scene.springs[c]` carries the physics params for that chain.
/// `collider_attach_nodes[i]` is the glTF node index that collider `i` is attached to.
///
/// This is the Phase 6 multi-chain emitter. The single-chain wrapper `vrmc_spring_bone_scene`
/// delegates here.
pub fn vrmc_spring_bone_scene_multichain(
    joint_nodes_per_chain: &[Vec<usize>],
    scene: &SpringBoneSceneParams,
    collider_attach_nodes: &[usize],
) -> Value {
    assert_eq!(
        joint_nodes_per_chain.len(),
        scene.springs.len(),
        "joint_nodes_per_chain must parallel scene.springs"
    );
    assert_eq!(
        scene.spring_collider_groups.len(),
        scene.springs.len(),
        "spring_collider_groups must parallel scene.springs"
    );
    assert_eq!(
        scene.colliders.len(),
        collider_attach_nodes.len(),
        "collider_attach_nodes must parallel scene.colliders"
    );

    let springs_json: Vec<Value> = scene
        .springs
        .iter()
        .enumerate()
        .map(|(c_idx, params)| {
            let chain_joints = &joint_nodes_per_chain[c_idx];
            let joint_count = chain_joints.len();
            let joints_json: Vec<Value> = chain_joints
                .iter()
                .enumerate()
                .map(|(j_idx, &node)| {
                    let stiffness = joint_value(
                        &params.stiffness_per_joint,
                        params.stiffness,
                        j_idx,
                        joint_count,
                        "stiffness",
                    );
                    let drag = joint_value(
                        &params.drag_force_per_joint,
                        params.drag_force,
                        j_idx,
                        joint_count,
                        "drag_force",
                    );
                    let gravity_power = joint_value(
                        &params.gravity_power_per_joint,
                        params.gravity_power,
                        j_idx,
                        joint_count,
                        "gravity_power",
                    );
                    let hit_radius = joint_value(
                        &params.hit_radius_per_joint,
                        params.hit_radius,
                        j_idx,
                        joint_count,
                        "hit_radius",
                    );
                    let mut j = json!({
                        "node": node,
                        "hitRadius": hit_radius,
                        "stiffness": stiffness,
                        "gravityPower": gravity_power,
                        "gravityDir": params.gravity_dir,
                        "dragForce": drag,
                    });
                    if let Some(deg) = params.joint_angle_limit_deg {
                        j["extensions"] = json!({
                            "VRMC_springBone_extended_collider": { "angleLimit": deg }
                        });
                    }
                    j
                })
                .collect();

            let mut spring = json!({
                "name": params.spring_name,
                "joints": joints_json,
            });
            let groups = &scene.spring_collider_groups[c_idx];
            if !groups.is_empty() {
                spring["colliderGroups"] = json!(groups);
            }
            spring
        })
        .collect();

    let mut out = json!({
        "specVersion": "1.0",
        "springs": springs_json,
    });

    // Colliders and colliderGroups are scene-level (shared across all chains).
    if !scene.colliders.is_empty() {
        let colliders: Vec<Value> = scene
            .colliders
            .iter()
            .zip(collider_attach_nodes.iter())
            .map(|(c, &node)| match &c.shape {
                ColliderShape::Sphere { radius } => {
                    json!({
                        "node": node,
                        "shape": { "sphere": { "offset": c.offset, "radius": radius } }
                    })
                }
                ColliderShape::Capsule {
                    radius,
                    tail_offset,
                } => {
                    json!({
                        "node": node,
                        "shape": { "capsule": { "offset": c.offset, "radius": radius, "tail": tail_offset } }
                    })
                }
                ColliderShape::Plane { normal } => {
                    json!({
                        "node": node,
                        "extensions": {
                            "VRMC_springBone_extended_collider": {
                                "shape": { "plane": { "offset": c.offset, "normal": normal } }
                            }
                        }
                    })
                }
                ColliderShape::InsideSphere { radius } => {
                    json!({
                        "node": node,
                        "extensions": {
                            "VRMC_springBone_extended_collider": {
                                "shape": { "sphere": { "offset": c.offset, "radius": radius, "inside": true } }
                            }
                        }
                    })
                }
                ColliderShape::InsideCapsule {
                    radius,
                    tail_offset,
                } => {
                    json!({
                        "node": node,
                        "extensions": {
                            "VRMC_springBone_extended_collider": {
                                "shape": { "capsule": { "offset": c.offset, "radius": radius, "tail": tail_offset, "inside": true } }
                            }
                        }
                    })
                }
            })
            .collect();
        out["colliders"] = json!(colliders);
    }

    if !scene.collider_groups.is_empty() {
        let groups: Vec<Value> = scene
            .collider_groups
            .iter()
            .map(|g: &ColliderGroupParams| {
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

/// Emit VRMC_springBone extension JSON for a scene with optional colliders.
///
/// Single-chain wrapper over `vrmc_spring_bone_scene_multichain`. `joint_nodes` is
/// in chain order, head-to-tail.
///
/// `collider_attach_nodes[i]` is the glTF node index that collider `i` is
/// attached to. The caller resolves Head / NewIntermediateNode → node index
/// during emit.
pub fn vrmc_spring_bone_scene(
    joint_nodes: &[usize],
    scene: &SpringBoneSceneParams,
    collider_attach_nodes: &[usize],
) -> Value {
    vrmc_spring_bone_scene_multichain(&[joint_nodes.to_vec()], scene, collider_attach_nodes)
}

#[cfg(test)]
mod multichain_emit_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn two_chains_emit_two_springs_entries() {
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("chain_a"),
                SpringBoneParams::defaults("chain_b"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![]],
        };
        let joint_nodes_per_chain = vec![vec![10, 11, 12, 13], vec![20, 21, 22, 23]];
        let v = vrmc_spring_bone_scene_multichain(&joint_nodes_per_chain, &scene, &[]);
        let springs = v["springs"].as_array().unwrap();
        assert_eq!(springs.len(), 2);
        assert_eq!(springs[0]["name"], "chain_a_chain");
        assert_eq!(springs[1]["name"], "chain_b_chain");
        assert_eq!(springs[0]["joints"].as_array().unwrap().len(), 4);
        assert_eq!(springs[1]["joints"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn three_chains_with_shared_collider_group_emit_per_spring_group_indices() {
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("ca"),
                SpringBoneParams::defaults("cb"),
                SpringBoneParams::defaults("cc"),
            ],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "shared".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0], vec![0], vec![0]],
        };
        let joints = vec![
            vec![10, 11, 12, 13],
            vec![20, 21, 22, 23],
            vec![30, 31, 32, 33],
        ];
        let v = vrmc_spring_bone_scene_multichain(&joints, &scene, &[40]);
        let springs = v["springs"].as_array().unwrap();
        assert_eq!(springs.len(), 3);
        for s in springs {
            let groups = s["colliderGroups"].as_array().unwrap();
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].as_u64().unwrap(), 0);
        }
    }

    #[test]
    fn single_chain_wrapper_delegates_to_multichain() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        let scene = SpringBoneSceneParams::single_spring(p);
        let v1 = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        let v2 = vrmc_spring_bone_scene_multichain(&[vec![0, 1, 2, 3]], &scene, &[]);
        assert_eq!(v1, v2);
    }
}

#[cfg(test)]
mod taper_emit_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn uniform_stiffness_emits_same_value_on_all_joints() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        p.stiffness = 0.5;
        let scene = SpringBoneSceneParams::single_spring(p);
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        let joints = v["springs"][0]["joints"].as_array().unwrap();
        for j in joints {
            assert!((j["stiffness"].as_f64().unwrap() - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn per_joint_stiffness_emits_taper() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        p.stiffness = 0.5; // ignored when per-joint set
        p.stiffness_per_joint = Some(vec![1.0, 0.7, 0.4, 0.1]);
        let scene = SpringBoneSceneParams::single_spring(p);
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        let joints = v["springs"][0]["joints"].as_array().unwrap();
        let stiffnesses: Vec<f64> = joints
            .iter()
            .map(|j| j["stiffness"].as_f64().unwrap())
            .collect();
        assert!((stiffnesses[0] - 1.0).abs() < 1e-6);
        assert!((stiffnesses[1] - 0.7).abs() < 1e-6);
        assert!((stiffnesses[2] - 0.4).abs() < 1e-6);
        assert!((stiffnesses[3] - 0.1).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "stiffness_per_joint length")]
    fn per_joint_length_mismatch_panics() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        p.stiffness_per_joint = Some(vec![1.0, 0.5]); // only 2, not 4
        let scene = SpringBoneSceneParams::single_spring(p);
        // This should panic at emission time — length mismatch is a programmer error.
        vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
    }

    #[test]
    fn per_joint_drag_force_emits_taper() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 3;
        p.drag_force_per_joint = Some(vec![0.9, 0.5, 0.1]);
        let scene = SpringBoneSceneParams::single_spring(p);
        let v = vrmc_spring_bone_scene(&[0, 1, 2], &scene, &[]);
        let drags: Vec<f64> = v["springs"][0]["joints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|j| j["dragForce"].as_f64().unwrap())
            .collect();
        assert!((drags[0] - 0.9).abs() < 1e-6);
        assert!((drags[2] - 0.1).abs() < 1e-6);
    }
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

#[cfg(test)]
mod extended_emit_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn plane_collider_emits_extension_shape() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Plane {
                    normal: [0.0, 1.0, 0.0],
                },
                offset: [0.0, -0.10, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]);
        let c0 = &v["colliders"][0];
        // Base shape MUST be omitted when extended is used:
        assert!(
            c0.get("shape").is_none()
                || c0["shape"]
                    .as_object()
                    .map(|o| o.is_empty())
                    .unwrap_or(false),
            "base shape must be omitted when using extended shape, got {c0}"
        );
        let ext = &c0["extensions"]["VRMC_springBone_extended_collider"]["shape"];
        assert!(
            ext["plane"].is_object(),
            "expected plane extended shape: {c0}"
        );
        let normal = ext["plane"]["normal"].as_array().unwrap();
        assert!((normal[1].as_f64().unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn inside_sphere_emits_extension_shape() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::InsideSphere { radius: 0.20 },
                offset: [0.0, 0.0, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]);
        let ext = &v["colliders"][0]["extensions"]["VRMC_springBone_extended_collider"]["shape"];
        assert!(ext["sphere"].is_object());
        assert_eq!(ext["sphere"]["inside"], true);
        assert!((ext["sphere"]["radius"].as_f64().unwrap() - 0.20).abs() < 1e-6);
    }

    #[test]
    fn inside_capsule_emits_inside_true() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::InsideCapsule {
                    radius: 0.10,
                    tail_offset: [0.0, 0.30, 0.0],
                },
                offset: [0.0, 0.0, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]);
        let ext = &v["colliders"][0]["extensions"]["VRMC_springBone_extended_collider"]["shape"];
        assert!(ext["capsule"].is_object());
        assert_eq!(ext["capsule"]["inside"], true);
    }

    #[test]
    fn joint_angle_limit_emits_under_extension() {
        let mut spring = SpringBoneParams::defaults("c");
        spring.joint_angle_limit_deg = Some(60.0);
        let scene = SpringBoneSceneParams::single_spring(spring);
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        let joints = v["springs"][0]["joints"].as_array().unwrap();
        for j in joints {
            let limit = &j["extensions"]["VRMC_springBone_extended_collider"]["angleLimit"];
            assert!(
                (limit.as_f64().unwrap() - 60.0).abs() < 1e-6,
                "expected angleLimit=60 on every joint, got {j}"
            );
        }
    }

    #[test]
    fn no_angle_limit_does_not_emit_extension_on_joints() {
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("c"));
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        let j0 = &v["springs"][0]["joints"][0];
        assert!(
            j0.get("extensions").is_none() || j0["extensions"].as_object().unwrap().is_empty(),
            "joint with no angle limit must not carry extensions block, got {j0}"
        );
    }
}
