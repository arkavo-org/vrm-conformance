//! Top-level VRM 1.0 asset emission. Combines mesh, buffer, humanoid stub,
//! and VRM extensions into a single `.vrm` GLB on disk.

use crate::buffer::{pack_mesh, pack_mesh_with_morphs, pack_sphere_and_multichains};
use crate::glb::{write_glb, GlbDocument};
use crate::humanoid::minimal_skeleton;
use crate::mesh::sphere;
use crate::params::MToonParams;
use crate::vrm_ext::{base_material, viseme_preset_binds, vrmc_vrm};
use anyhow::Result;
use camino::Utf8Path;
use serde_json::{json, Value};

pub fn emit_vrm(params: &MToonParams, output: &Utf8Path) -> Result<()> {
    emit_vrm_with_custom_expressions(params, output, &[])
}

/// Like [`emit_vrm`] but pre-registers the named custom expressions on
/// the avatar's `VRMC_vrm.expressions.custom` map with empty
/// `morphTargetBinds`. Used by the VRMA expression sweep's custom-
/// expression variants so `VRMExpressionController` accepts
/// `setCustomExpressionWeight(name, …)` instead of silently no-op'ing
/// (per `VRMMorphTargets.swift:533` and equivalents in three-vrm /
/// godot-vrm / UniVRM). Bound morphs are intentionally empty: the test
/// signal is the controller's `weight(forCustom:)` being non-zero, not
/// a visible mesh deformation.
pub fn emit_vrm_with_custom_expressions(
    params: &MToonParams,
    output: &Utf8Path,
    custom_expression_names: &[&str],
) -> Result<()> {
    // 1) Mesh + buffer + five POSITION-only morph targets for visemes
    //    (aa, ih, ou, ee, oh). Each delta produces a visually distinct
    //    deformation pattern when the corresponding expression weight is
    //    driven to 1.0, so cross-renderer SSIM can falsify "weight accepted
    //    but mesh not deformed". Order MUST match `VISEME_PRESETS`.
    let mesh = sphere(0.3, 24, 48); // small radius so the sphere fits at avatar chest height
    let morphs = viseme_morph_deltas(&mesh.positions);
    let (packed, morph_accessors) = pack_mesh_with_morphs(&mesh, &morphs);

    // 2) Humanoid skeleton
    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    // 3) Add a mesh-bearing node parented to head (so the sphere visualizes
    //    where the head is). Material 0 = our MToon material.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", params.id),
        "mesh": 0
    }));
    // Append mesh_node_index as a child of head.
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    let targets: Vec<Value> = morph_accessors
        .iter()
        .map(|&idx| json!({ "POSITION": idx }))
        .collect();

    // Declare every extension the document actually carries. Spec-wise
    // `extensionsUsed` is informative (renderers ignore unknowns) but
    // validators enforce that every extension referenced in the JSON
    // appears here. The hdr_emissiveMultiplier entry is only added when
    // the material's effective multiplier diverges from the default 1.0,
    // matching the conditional emission in `base_material`.
    let mut extensions_used: Vec<&str> =
        vec!["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"];
    let emits_emissive_multiplier = params.emissive_factor.iter().any(|&c| c != 0.0)
        && (params.emissive_multiplier - 1.0).abs() > f32::EPSILON;
    if emits_emissive_multiplier {
        extensions_used.push("VRMC_materials_hdr_emissiveMultiplier");
    }

    // 4) Build the glTF JSON document
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4,
                        "targets": targets
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node, mesh_node_index)
        }
    });

    doc["extensions"]["VRMC_vrm"]["expressions"]["preset"] = viseme_preset_binds(mesh_node_index);

    if !custom_expression_names.is_empty() {
        let mut custom_map = serde_json::Map::new();
        for name in custom_expression_names {
            custom_map.insert((*name).to_string(), json!({ "morphTargetBinds": [] }));
        }
        doc["extensions"]["VRMC_vrm"]["expressions"]["custom"] = Value::Object(custom_map);
    }

    // Splice in buffers/bufferViews/accessors from the packed mesh.
    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    // 5) Serialize and write GLB
    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// POSITION-only morph deltas for the five viseme presets in
/// [`crate::vrm_ext::VISEME_PRESETS`] order. Each delta yields a visually
/// distinct deformation of the head-mounted sphere so the rendered images
/// disambiguate per viseme. NORMAL is omitted by design — shading on the
/// deformed sphere will be approximate but the silhouette change is the
/// signal the SSIM diff measures.
fn viseme_morph_deltas(positions: &[[f32; 3]]) -> Vec<Vec<[f32; 3]>> {
    let n = positions.len();
    let aa: Vec<[f32; 3]> = (0..n).map(|_| [0.04, 0.0, 0.0]).collect();
    let ih: Vec<[f32; 3]> = (0..n).map(|_| [0.0, -0.04, 0.0]).collect();
    let ou: Vec<[f32; 3]> = (0..n).map(|_| [0.0, 0.0, 0.04]).collect();
    let ee: Vec<[f32; 3]> = (0..n).map(|_| [-0.04, 0.0, 0.0]).collect();
    let oh: Vec<[f32; 3]> = positions
        .iter()
        .map(|p| [p[0] * 0.1, p[1] * 0.1, p[2] * 0.1])
        .collect();
    vec![aa, ih, ou, ee, oh]
}

use crate::sidecar::{build_default_test_plan, write_meta_json, write_test_yaml};
use crate::spring_bone::{ColliderAttach, ColliderShape, SpringBoneParams, SpringBoneSceneParams};
use crate::vrm_ext::{
    vrmc_spring_bone, vrmc_spring_bone_scene, vrmc_vrm_with_lookat_type, LookAtType,
};

/// Emit a `.vrm` GLB identical to `emit_vrm` except that the avatar's
/// `VRMC_vrm.lookAt.type` is set to the caller-supplied value.
/// Existing callers of `emit_vrm` are unaffected.
pub fn emit_vrm_with_lookat_type(
    params: &MToonParams,
    lookat_type: LookAtType,
    output: &Utf8Path,
) -> Result<()> {
    // 1) Mesh + buffer
    let mesh = sphere(0.3, 24, 48);
    let packed = pack_mesh(&mesh);

    // 2) Humanoid skeleton
    let skeleton = minimal_skeleton();
    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();
    let head_node = skeleton.bone_to_node["head"];

    // 3) Mesh-bearing node parented to head.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", params.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // 4) Build the glTF JSON document with the requested lookAt.type.
    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": ["KHR_materials_unlit", "VRMC_vrm", "VRMC_materials_mtoon"],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [
            { "nodes": [skeleton.root_node] }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", params.id),
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": 0,
                            "NORMAL": 1,
                            "TEXCOORD_0": 2
                        },
                        "indices": 3,
                        "material": 0,
                        "mode": 4
                    }
                ]
            }
        ],
        "materials": [base_material(params)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm_with_lookat_type(&params.id, &skeleton.bone_to_node, mesh_node_index, lookat_type)
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Returns `true` when the scene contains any extended-shape collider
/// (Plane, InsideSphere, InsideCapsule) or any spring with a joint angle
/// limit. When true, the glTF `extensionsUsed` must declare
/// `"VRMC_springBone_extended_collider"`.
fn scene_uses_extended_collider(scene: &SpringBoneSceneParams) -> bool {
    let has_extended_shape = scene.colliders.iter().any(|c| {
        matches!(
            &c.shape,
            ColliderShape::Plane { .. }
                | ColliderShape::InsideSphere { .. }
                | ColliderShape::InsideCapsule { .. }
        )
    });
    let has_angle_limit = scene
        .springs
        .iter()
        .any(|s| s.joint_angle_limit_deg.is_some());
    has_extended_shape || has_angle_limit
}

/// Emits `<stem>.vrm`, `<stem>.meta.json`, and `<stem>.test.yaml` from a
/// single MToonParams value.
pub fn emit_with_sidecars(params: &MToonParams, stem: &Utf8Path) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm(params, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(params, None, &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = build_default_test_plan(params, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a `.vrm` GLB containing MToon material data, a VRMC_springBone
/// chain attached to the head bone, **and a cylinder mesh skinned to
/// the chain joints** so spring-bone physics is visible in pixel space.
///
/// Three renderables:
///
/// 1. A head-mounted sphere (the existing MToon material reference shape,
///    parented to the head node, not skinned).
/// 2. A vertical cylinder hanging from the head, weighted to the chain
///    joints. When a renderer's spring-bone physics moves a joint, the
///    cylinder bends with it.
/// 3. The VRMC_springBone extension declaring the chain itself.
///
/// Wiring was previously deferred because vrm-metal-kit at our prior pin
/// dropped non-skinned meshes when any skin was present
/// ([VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181));
/// the 0.13.1 release closes that, so the chain-skinned mesh now coexists
/// with the head-mounted sphere across all renderers.
pub fn emit_vrm_with_spring_bone(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    output: &Utf8Path,
) -> Result<()> {
    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
    );

    // Each chain joint's rest-pose world Y is head_world_y - (i+1)*segment_length.
    // The cylinder runs from joint 0's Y (top) downward and is skinned to
    // the chain joints by ring (see chain_mesh.rs).
    let head_world = crate::humanoid::rest_pose_world_position("head");
    let head_world_y = head_world[1];
    let chain_top_y = head_world_y - spring_bone.segment_length_m;

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        /* radius */ 0.025,
        chain_top_y,
        /* ring_segments */ 12,
    );

    // Inverse-bind matrices: each joint's bind-pose world transform is a
    // pure translation to its rest-pose Y. Inverse = translation by
    // negated Y. Stored column-major per the glTF Mat4 convention.
    let inv_bind: Vec<[f32; 16]> = (0..spring_bone.joint_count)
        .map(|i| {
            let jy = head_world_y - ((i + 1) as f32) * spring_bone.segment_length_m;
            #[rustfmt::skip]
            let m = [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, -jy, 0.0, 1.0,
            ];
            m
        })
        .collect();

    let packed = crate::buffer::pack_sphere_and_chain(&mesh, &chain_mesh, &inv_bind);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head, identical to emit_vrm's wiring.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // Chain-skinned node — child of hips so the scene stays single-rooted.
    // Its own transform is ignored at draw time; skin.joints +
    // inverseBindMatrices fully determine vertex positions.
    let chain_mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_chain_mesh", mtoon.id),
        "mesh": 1,
        "skin": 0
    }));
    let hips = &mut nodes[skeleton.root_node];
    let mut hips_children = hips["children"].as_array().cloned().unwrap_or_default();
    hips_children.push(json!(chain_mesh_node_index));
    hips["children"] = Value::Array(hips_children);

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": [
            "KHR_materials_unlit",
            "VRMC_vrm",
            "VRMC_materials_mtoon",
            "VRMC_springBone"
        ],
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": [
            // mesh 0: head-mounted sphere
            {
                "name": format!("{}_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1,
                        "TEXCOORD_0": 2
                    },
                    "indices": 3,
                    "material": 0,
                    "mode": 4
                }]
            },
            // mesh 1: chain cylinder (skinned to spring-bone joints)
            {
                "name": format!("{}_chain_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 4,
                        "NORMAL": 5,
                        "TEXCOORD_0": 6,
                        "JOINTS_0": 8,
                        "WEIGHTS_0": 9
                    },
                    "indices": 7,
                    "material": 0,
                    "mode": 4
                }]
            }
        ],
        "skins": [{
            "joints": chain_nodes,
            "inverseBindMatrices": 10,
            "skeleton": chain_nodes[0]
        }],
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm_with_chain_mesh(
                &mtoon.id, &skeleton.bone_to_node, mesh_node_index, chain_mesh_node_index
            ),
            "VRMC_springBone": vrmc_spring_bone(&chain_nodes, spring_bone),
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// VRMC_vrm helper that annotates BOTH the head mesh and the chain mesh
/// in `firstPerson.meshAnnotations`. Both use `type: "both"` so they're
/// unconditionally visible regardless of camera mode; the conformance
/// test plans are always third-person renders.
fn vrmc_vrm_with_chain_mesh(
    meta_name: &str,
    bone_to_node: &std::collections::BTreeMap<String, usize>,
    sphere_mesh_node: usize,
    chain_mesh_node: usize,
) -> Value {
    let mut ext = vrmc_vrm(meta_name, bone_to_node, sphere_mesh_node);
    ext["firstPerson"]["meshAnnotations"] = json!([
        { "node": sphere_mesh_node, "type": "both" },
        { "node": chain_mesh_node,  "type": "both" }
    ]);
    ext
}

/// Emits `<stem>.vrm` (MToon + spring-bone), `<stem>.meta.json` (with
/// spring-bone params), and `<stem>.test.yaml` from one MToonParams +
/// one SpringBoneParams pair.
pub fn emit_with_sidecars_spring_bone(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same VRM body as `emit_with_sidecars_spring_bone`, but the emitted
/// `.test.yaml` carries an additional `animation.root_transform` block.
/// The runner will settle the chain, then translate the root sideways
/// 15 cm over 0.25 s before rendering — capturing the chain mid-swing
/// rather than at the static settle. See
/// `build_spring_bone_swing_test_plan` for the rationale on the numbers.
pub fn emit_with_sidecars_spring_bone_swing(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_swing_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same VRM body as `emit_with_sidecars_spring_bone_swing`, but the
/// `.test.yaml` carries a `render_sequence:` block (sequence-mode) instead
/// of `animation: { root_transform }` (single-frame mode). Used by the
/// `emit-sequence-sweep` CLI subcommand.
pub fn emit_with_sidecars_spring_bone_swing_sequence(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_swing_sequence_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a `.vrm` GLB identical in structure to `emit_vrm_with_spring_bone`
/// but with VRMC_springBone colliders declared in the extension.
///
/// For `ColliderAttach::Head`, the collider node is the head node index.
/// For `ColliderAttach::NewIntermediateNode { y_offset, z_offset }`, a new
/// glTF node is inserted as a child of head at the given local offset and
/// its index is used.
///
/// NOTE: factor shared chain setup with `emit_vrm_with_spring_bone` in phase 6
/// multi-chain refactor; for phase 2 we accept the duplication.
pub fn emit_vrm_with_spring_bone_colliders(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    output: &Utf8Path,
) -> Result<()> {
    let spring_bone = &scene.springs[0];
    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
    );

    let head_world = crate::humanoid::rest_pose_world_position("head");
    let head_world_y = head_world[1];
    let chain_top_y = head_world_y - spring_bone.segment_length_m;

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        /* radius */ 0.025,
        chain_top_y,
        /* ring_segments */ 12,
    );

    let inv_bind: Vec<[f32; 16]> = (0..spring_bone.joint_count)
        .map(|i| {
            let jy = head_world_y - ((i + 1) as f32) * spring_bone.segment_length_m;
            #[rustfmt::skip]
            let m = [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, -jy, 0.0, 1.0,
            ];
            m
        })
        .collect();

    let packed = crate::buffer::pack_sphere_and_chain(&mesh, &chain_mesh, &inv_bind);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node — child of head.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // Chain-skinned node — child of hips.
    let chain_mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_chain_mesh", mtoon.id),
        "mesh": 1,
        "skin": 0
    }));
    let hips = &mut nodes[skeleton.root_node];
    let mut hips_children = hips["children"].as_array().cloned().unwrap_or_default();
    hips_children.push(json!(chain_mesh_node_index));
    hips["children"] = Value::Array(hips_children);

    // Resolve collider attach nodes. For Head attach, use head_node.
    // For NewIntermediateNode, add a new glTF node parented under head.
    let mut collider_attach_nodes: Vec<usize> = Vec::with_capacity(scene.colliders.len());
    for collider in &scene.colliders {
        match &collider.attach {
            ColliderAttach::Head => {
                collider_attach_nodes.push(head_node);
            }
            ColliderAttach::NewIntermediateNode { y_offset, z_offset } => {
                let new_node_idx = nodes.len();
                nodes.push(json!({
                    "name": format!("{}_collider_node_{}", mtoon.id, new_node_idx),
                    "translation": [0.0, y_offset, z_offset],
                }));
                // Parent under head
                let head_node_ref = &mut nodes[head_node];
                let mut hc = head_node_ref["children"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                hc.push(json!(new_node_idx));
                head_node_ref["children"] = Value::Array(hc);
                collider_attach_nodes.push(new_node_idx);
            }
        }
    }

    // Build extensionsUsed: always declare springBone; add extended collider
    // extension only when the scene actually uses it.
    let mut extensions_used = vec![
        "KHR_materials_unlit",
        "VRMC_vrm",
        "VRMC_materials_mtoon",
        "VRMC_springBone",
    ];
    if scene_uses_extended_collider(scene) {
        extensions_used.push("VRMC_springBone_extended_collider");
    }

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": [
            {
                "name": format!("{}_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1,
                        "TEXCOORD_0": 2
                    },
                    "indices": 3,
                    "material": 0,
                    "mode": 4
                }]
            },
            {
                "name": format!("{}_chain_geom", mtoon.id),
                "primitives": [{
                    "attributes": {
                        "POSITION": 4,
                        "NORMAL": 5,
                        "TEXCOORD_0": 6,
                        "JOINTS_0": 8,
                        "WEIGHTS_0": 9
                    },
                    "indices": 7,
                    "material": 0,
                    "mode": 4
                }]
            }
        ],
        "skins": [{
            "joints": chain_nodes,
            "inverseBindMatrices": 10,
            "skeleton": chain_nodes[0]
        }],
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm_with_chain_mesh(
                &mtoon.id, &skeleton.bone_to_node, mesh_node_index, chain_mesh_node_index
            ),
            "VRMC_springBone": vrmc_spring_bone_scene(&chain_nodes, scene, &collider_attach_nodes),
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emits `<stem>.vrm` (MToon + spring-bone with colliders), `<stem>.meta.json`,
/// and `<stem>.test.yaml` (settle variant, 60-step settle).
pub fn emit_with_sidecars_spring_bone_colliders(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_collider_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as `emit_with_sidecars_spring_bone_colliders` but the test plan
/// carries an `animation.root_transform` block (swing variant).
pub fn emit_with_sidecars_spring_bone_colliders_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan =
        crate::sidecar::build_spring_bone_collider_swing_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emits `<stem>.vrm` (MToon + spring-bone with extended colliders),
/// `<stem>.meta.json`, and `<stem>.test.yaml` (settle variant, 60-step settle).
/// Uses `VRMC_springBone_extended_collider` extension shapes/angle limits.
pub fn emit_with_sidecars_spring_bone_extended(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_extended_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as `emit_with_sidecars_spring_bone_extended` but the test plan
/// carries an `animation.root_transform` block (swing variant).
pub fn emit_with_sidecars_spring_bone_extended_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_colliders(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan =
        crate::sidecar::build_spring_bone_extended_swing_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a `.vrm` GLB with N parallel spring-bone chains.
///
/// Each chain gets its own intermediate node radially spaced around the head bone
/// (angles 0°, 360°/N, 2·360°/N, …, in the XZ plane at a fixed radial distance of
/// 0.05 m from the head axis). Each chain hangs straight down from its intermediate
/// node and is skinned by its own cylinder mesh.
///
/// Structure:
/// - 1 sphere mesh (head-mounted, parented to head — the MToon reference shape)
/// - N chain cylinder meshes (each skinned to its chain's joints)
/// - N skins (one per chain)
/// - VRMC_springBone extension with N springs entries
pub fn emit_vrm_with_spring_bone_multichain(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    output: &Utf8Path,
) -> Result<()> {
    let n_chains = scene.springs.len();
    assert!(n_chains >= 1, "multichain emit needs at least 1 chain");

    let mesh = sphere(0.3, 24, 48);

    let mut skeleton = crate::humanoid::minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let head_world = crate::humanoid::rest_pose_world_position("head");
    let head_world_y = head_world[1];

    // Radial spacing: chains are placed at radius CHAIN_RADIAL_M from the head axis.
    // For n_chains=1 this puts the single chain at angle 0 (same as the single-chain emit).
    const CHAIN_RADIAL_M: f32 = 0.05;

    let mut chain_joint_nodes: Vec<Vec<usize>> = Vec::with_capacity(n_chains);
    let mut chain_meshes: Vec<crate::chain_mesh::SkinnedMeshData> = Vec::with_capacity(n_chains);
    let mut inv_binds: Vec<Vec<[f32; 16]>> = Vec::with_capacity(n_chains);

    for (c_idx, spring_params) in scene.springs.iter().enumerate() {
        // Radial angle for this chain.
        let angle = (c_idx as f32) * 2.0 * std::f32::consts::PI / (n_chains as f32);
        let (sin_a, cos_a) = angle.sin_cos();
        let rx = CHAIN_RADIAL_M * sin_a;
        let rz = CHAIN_RADIAL_M * cos_a;

        // Intermediate node: child of head, offset radially so each chain hangs
        // from a distinct XZ position.
        let nodes = skeleton.nodes_json.as_array_mut().unwrap();
        let inter_idx = nodes.len();
        nodes.push(json!({
            "name": format!("{}_chain{}_inter", mtoon.id, c_idx),
            "translation": [rx, 0.0, rz],
        }));
        // Wire intermediate under head.
        let head_ref = nodes.get_mut(head_node).unwrap();
        let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
        hc.push(json!(inter_idx));
        head_ref["children"] = Value::Array(hc);

        // Append chain joints as children of the intermediate node.
        let chain_nodes = crate::humanoid::append_spring_chain(
            &mut skeleton,
            inter_idx,
            spring_params.joint_count,
            spring_params.segment_length_m,
        );

        // Chain cylinder: top_world_y is the intermediate node's world Y (same as head)
        // since the intermediate node has translation (rx, 0, rz) relative to head.
        let chain_top_y = head_world_y - spring_params.segment_length_m;
        let chain_mesh = crate::chain_mesh::build_chain_cylinder(
            spring_params.joint_count,
            spring_params.segment_length_m,
            0.025,
            chain_top_y,
            12,
        );

        // Inverse-bind matrices for this chain's joints.
        let ibm: Vec<[f32; 16]> = (0..spring_params.joint_count)
            .map(|i| {
                let jy = head_world_y - ((i + 1) as f32) * spring_params.segment_length_m;
                #[rustfmt::skip]
                let m = [
                    1.0, 0.0, 0.0, 0.0,
                    0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                    0.0, -jy, 0.0, 1.0,
                ];
                m
            })
            .collect();

        chain_joint_nodes.push(chain_nodes);
        chain_meshes.push(chain_mesh);
        inv_binds.push(ibm);
    }

    // Pack all geometry into a single buffer.
    let chains_for_pack: Vec<(&crate::chain_mesh::SkinnedMeshData, &[[f32; 16]])> = chain_meshes
        .iter()
        .zip(inv_binds.iter())
        .map(|(cm, ibm)| (cm, ibm.as_slice()))
        .collect();
    let packed = pack_sphere_and_multichains(&mesh, &chains_for_pack);

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // Sphere mesh node: child of head.
    let sphere_mesh_node = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head_ref = &mut nodes[head_node];
    let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
    hc.push(json!(sphere_mesh_node));
    head_ref["children"] = Value::Array(hc);

    // Chain mesh nodes: child of hips (so scene stays single-rooted).
    // Mesh index 1..=N, skin index 0..N-1.
    let mut chain_mesh_nodes: Vec<usize> = Vec::with_capacity(n_chains);
    for c_idx in 0..n_chains {
        let chain_mesh_node = nodes.len();
        nodes.push(json!({
            "name": format!("{}_chain{}_mesh", mtoon.id, c_idx),
            "mesh": 1 + c_idx,
            "skin": c_idx
        }));
        let hips_ref = &mut nodes[skeleton.root_node];
        let mut hips_children = hips_ref["children"].as_array().cloned().unwrap_or_default();
        hips_children.push(json!(chain_mesh_node));
        hips_ref["children"] = Value::Array(hips_children);
        chain_mesh_nodes.push(chain_mesh_node);
    }

    // Resolve collider attach nodes.
    let mut collider_attach_nodes: Vec<usize> = Vec::with_capacity(scene.colliders.len());
    for collider in &scene.colliders {
        match &collider.attach {
            ColliderAttach::Head => {
                collider_attach_nodes.push(head_node);
            }
            ColliderAttach::NewIntermediateNode { y_offset, z_offset } => {
                let new_node_idx = nodes.len();
                nodes.push(json!({
                    "name": format!("{}_collider_node_{}", mtoon.id, new_node_idx),
                    "translation": [0.0, y_offset, z_offset],
                }));
                let head_ref = &mut nodes[head_node];
                let mut hc = head_ref["children"].as_array().cloned().unwrap_or_default();
                hc.push(json!(new_node_idx));
                head_ref["children"] = Value::Array(hc);
                collider_attach_nodes.push(new_node_idx);
            }
        }
    }

    // extensionsUsed.
    let mut extensions_used = vec![
        "KHR_materials_unlit",
        "VRMC_vrm",
        "VRMC_materials_mtoon",
        "VRMC_springBone",
    ];
    if scene_uses_extended_collider(scene) {
        extensions_used.push("VRMC_springBone_extended_collider");
    }

    // Meshes: sphere (index 0) + N chain cylinders (index 1..N).
    // Accessor base for chain i: 4 + i*7.
    let mut meshes: Vec<Value> = vec![json!({
        "name": format!("{}_geom", mtoon.id),
        "primitives": [{
            "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 },
            "indices": 3,
            "material": 0,
            "mode": 4
        }]
    })];
    for c_idx in 0..n_chains {
        let base = 4 + c_idx * 7;
        meshes.push(json!({
            "name": format!("{}_chain{}_geom", mtoon.id, c_idx),
            "primitives": [{
                "attributes": {
                    "POSITION": base,
                    "NORMAL": base + 1,
                    "TEXCOORD_0": base + 2,
                    "JOINTS_0": base + 4,
                    "WEIGHTS_0": base + 5
                },
                "indices": base + 3,
                "material": 0,
                "mode": 4
            }]
        }));
    }

    // Skins: one per chain.  inverseBindMatrices accessor index = 4 + i*7 + 6.
    let skins: Vec<Value> = (0..n_chains)
        .map(|c_idx| {
            let ibm_acc = 4 + c_idx * 7 + 6;
            json!({
                "joints": chain_joint_nodes[c_idx],
                "inverseBindMatrices": ibm_acc,
                "skeleton": chain_joint_nodes[c_idx][0]
            })
        })
        .collect();

    // VRMC_vrm firstPerson: annotate sphere + all chain mesh nodes.
    let mut all_mesh_nodes: Vec<Value> = vec![json!({ "node": sphere_mesh_node, "type": "both" })];
    for &cmn in &chain_mesh_nodes {
        all_mesh_nodes.push(json!({ "node": cmn, "type": "both" }));
    }
    let mut vrm_ext = vrmc_vrm(&mtoon.id, &skeleton.bone_to_node, sphere_mesh_node);
    vrm_ext["firstPerson"]["meshAnnotations"] = Value::Array(all_mesh_nodes);

    let mut doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "arkavo-org/vrm-conformance vrm-asset-generator 0.1"
        },
        "extensionsUsed": extensions_used,
        "extensionsRequired": ["VRMC_vrm"],
        "scene": 0,
        "scenes": [{ "nodes": [skeleton.root_node] }],
        "nodes": nodes,
        "meshes": meshes,
        "skins": skins,
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrm_ext,
            "VRMC_springBone": crate::vrm_ext::vrmc_spring_bone_scene_multichain(
                &chain_joint_nodes,
                scene,
                &collider_attach_nodes,
            ),
        }
    });

    for key in ["buffers", "bufferViews", "accessors"] {
        doc[key] = packed.json[key].clone();
    }

    let json_bytes = serde_json::to_vec(&doc)?;
    let glb = write_glb(&GlbDocument {
        json: json_bytes,
        binary: packed.binary,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, glb)?;
    Ok(())
}

/// Emits `<stem>.vrm` (MToon + multi-chain spring-bone), `<stem>.meta.json`,
/// and `<stem>.test.yaml` (settle variant).
pub fn emit_with_sidecars_spring_bone_multichain(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_multichain(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_multichain_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Same as `emit_with_sidecars_spring_bone_multichain` but the test plan
/// carries an `animation.root_transform` block (swing variant).
pub fn emit_with_sidecars_spring_bone_multichain_swing(
    mtoon: &MToonParams,
    scene: &SpringBoneSceneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone_multichain(mtoon, scene, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    let spring_bone = &scene.springs[0];
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan =
        crate::sidecar::build_spring_bone_multichain_swing_test_plan(mtoon, scene, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a humanoid-sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm is the canonical minimal humanoid rig (same as emit_default).
/// The .vrma carries a single-bone rotation animation; the .test.yaml
/// declares `animation.vrma` so the runner drives the VRMA op sequence
/// against any adapter.
pub fn emit_vrma_humanoid_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaHumanoidParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_humanoid_bone_rotation_channel, build_empty_vrma, finalize_vrma_scenes,
        register_all_humanoid_bones, write_vrma_glb,
    };
    use crate::vrma_params::RotationAxis;

    std::fs::create_dir_all(output_dir)?;

    // 1. Emit the .vrm (canonical default avatar — the .vrma carries the test signal).
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    emit_vrm(&mtoon_defaults, &vrm_path)?;

    // 2. Emit the .vrma.
    let skel = crate::humanoid::minimal_skeleton();
    let node_idx = *skel
        .bone_to_node
        .get(&params.bone_name)
        .unwrap_or_else(|| panic!("bone {} not in canonical skeleton", params.bone_name));

    let mut doc = build_empty_vrma();
    // Populate doc.nodes with the canonical skeleton's nodes (so node
    // indices in humanBones resolve).
    doc["nodes"] = skel.nodes_json.clone();

    // Declare all bones in humanBones so UniVRM 0.131.0 can build a valid
    // Unity HumanAvatar (requires at minimum hips + limb bones). Without
    // this the Avatar is invalid, AssignBonesFromAnimator returns false,
    // BoxMan is null, and TransferOwnership panics.
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let mut buffer = Vec::<u8>::new();
    let half_rad = params.angle_deg.to_radians() / 2.0;
    let sin_h = half_rad.sin();
    let target_quat = match params.axis {
        RotationAxis::X => [sin_h, 0.0, 0.0, half_rad.cos()],
        RotationAxis::Y => [0.0, sin_h, 0.0, half_rad.cos()],
        RotationAxis::Z => [0.0, 0.0, sin_h, half_rad.cos()],
    };
    let keyframes = [
        (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
        (params.duration_s, target_quat),
    ];
    add_humanoid_bone_rotation_channel(
        &mut doc,
        &mut buffer,
        node_idx,
        &params.bone_name,
        &keyframes,
    );

    // Populate scenes[0].nodes with root-level nodes before serialising.
    // Required by UniVRM 0.131.0 VrmAnimationImporter which unconditionally
    // accesses scenes[0] at LoadAsync line 245.
    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_path = output_dir.join(&vrma_relpath);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;

    // 3. Emit the .test.yaml.
    let plan = crate::sidecar::build_vrma_humanoid_test_plan(params, &vrm_relpath, &vrma_relpath);
    let yaml_path = output_dir.join(format!("{}.test.yaml", params.id));
    crate::sidecar::write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRMA expression sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm is the canonical minimal humanoid rig.
/// The .vrma carries a single-expression weight ramp: 0 → 1 → 0 over `duration_s`.
/// The .test.yaml declares `animation.vrma` with `apply_at_time = duration_s / 2`
/// so the runner samples at peak weight.
pub fn emit_vrma_expression_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaExpressionParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_expression_weight_channel, build_empty_vrma, finalize_vrma_scenes, write_vrma_glb,
        ExpressionKind,
    };

    std::fs::create_dir_all(output_dir)?;

    // 1. .vrm avatar. For preset-expression variants we use the canonical
    //    default avatar (which already registers the 5 viseme presets via
    //    `viseme_preset_binds`). For custom-expression variants we use
    //    `emit_vrm_with_custom_expressions` to pre-register the named
    //    custom expression so `VRMExpressionController.setCustomExpressionWeight`
    //    doesn't silently no-op (the controllers require the name to be in
    //    `VRMC_vrm.expressions.custom` before they accept a weight write).
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    if params.is_preset {
        emit_vrm(&mtoon_defaults, &vrm_path)?;
    } else {
        emit_vrm_with_custom_expressions(
            &mtoon_defaults,
            &vrm_path,
            &[params.expression_name.as_str()],
        )?;
    }

    // 2. .vrma: one node for the expression target + a 0→1→0 ramp.
    let mut doc = build_empty_vrma();
    let nodes = doc["nodes"].as_array_mut().unwrap();
    nodes.push(serde_json::json!({
        "name": format!("{}_expr_target", params.expression_name)
    }));
    let node_idx = nodes.len() - 1;

    let kind = if params.is_preset {
        ExpressionKind::Preset(&params.expression_name)
    } else {
        ExpressionKind::Custom(&params.expression_name)
    };
    let keyframes = [
        (0.0_f32, 0.0_f32),
        (params.duration_s / 2.0, 1.0),
        (params.duration_s, 0.0),
    ];

    let mut buffer = Vec::<u8>::new();
    add_expression_weight_channel(&mut doc, &mut buffer, node_idx, kind, &keyframes);

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_path = output_dir.join(&vrma_relpath);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;

    // 3. .test.yaml.
    let plan = crate::sidecar::build_vrma_expression_test_plan(params, &vrm_relpath, &vrma_relpath);
    let yaml_path = output_dir.join(format!("{}.test.yaml", params.id));
    crate::sidecar::write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}

/// Emit a VRMA lookAt sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm uses the avatar lookAt.type specified by `params.avatar_lookat_type`
/// (bone or expression). The .vrma carries a single lookAt gaze rotation from
/// identity to the target direction over `params.duration_s`. The .test.yaml
/// declares `animation.vrma` so the runner drives the VRMA op sequence.
pub fn emit_vrma_lookat_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaLookAtParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_look_at_channel, build_empty_vrma, finalize_vrma_scenes, write_vrma_glb,
    };
    use crate::vrma_params::{AvatarLookAtType, RotationAxis};

    std::fs::create_dir_all(output_dir)?;

    // 1. .vrm with the avatar's lookAt.type matching params.
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let avatar_lookat = match params.avatar_lookat_type {
        AvatarLookAtType::Bone => LookAtType::Bone,
        AvatarLookAtType::Expression => LookAtType::Expression,
    };
    emit_vrm_with_lookat_type(&mtoon_defaults, avatar_lookat, &vrm_path)?;

    // 2. .vrma with a single lookAt gaze direction.
    let mut doc = build_empty_vrma();
    doc["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "vrma_lookat_target"
        }));
    let node_idx: usize = 0; // first (and only) node

    let half_rad = (params.angle_deg.to_radians()) / 2.0;
    let sin_h = half_rad.sin();
    let cos_h = half_rad.cos();
    let target_quat = match params.axis {
        RotationAxis::X => [sin_h, 0.0, 0.0, cos_h],
        RotationAxis::Y => [0.0, sin_h, 0.0, cos_h],
        RotationAxis::Z => [0.0, 0.0, sin_h, cos_h],
    };
    let keyframes: [(f32, [f32; 4]); 2] = [
        (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
        (params.duration_s, target_quat),
    ];

    let mut buffer = Vec::<u8>::new();
    add_look_at_channel(
        &mut doc,
        &mut buffer,
        node_idx,
        [0.0, 0.06, 0.0],
        &keyframes,
    );

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_path = output_dir.join(&vrma_relpath);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;

    // 3. .test.yaml.
    let plan = crate::sidecar::build_vrma_lookat_test_plan(params, &vrm_relpath, &vrma_relpath);
    crate::sidecar::write_test_yaml(&plan, &output_dir.join(format!("{}.test.yaml", params.id)))?;

    Ok(())
}

#[cfg(test)]
mod multichain_emit_integration_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emit_three_chain_scene_produces_three_springs_in_glb_json() {
        let mtoon = MToonParams::defaults("multichain_test");
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("chain_a"),
                SpringBoneParams::defaults("chain_b"),
                SpringBoneParams::defaults("chain_c"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![], vec![]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_multichain(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let springs = &doc["extensions"]["VRMC_springBone"]["springs"];
        assert_eq!(springs.as_array().unwrap().len(), 3);
    }

    #[test]
    fn emit_two_chain_scene_produces_two_skins() {
        let mtoon = MToonParams::defaults("mc2_test");
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("ca"),
                SpringBoneParams::defaults("cb"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_multichain(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let skins = doc["skins"].as_array().unwrap();
        assert_eq!(skins.len(), 2, "one skin per chain");
        // Sphere mesh + N chain meshes = N+1 meshes.
        let meshes = doc["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 3, "sphere + 2 chain meshes");
    }
}

#[cfg(test)]
mod collider_emit_tests {
    use super::*;
    use crate::spring_bone::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emit_with_sphere_collider_produces_loadable_glb_with_collider_json() {
        let mtoon = crate::params::MToonParams::defaults("test_collider");
        let mut spring = SpringBoneParams::defaults("test_chain");
        spring.joint_count = 4;
        let scene = SpringBoneSceneParams {
            springs: vec![spring],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "head_g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };

        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        assert!(vrm_path.exists());

        // Inspect the GLB's JSON chunk to verify the collider made it through.
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).expect("read JSON chunk");
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let vrmc = &doc["extensions"]["VRMC_springBone"];
        assert!(vrmc["colliders"].is_array());
        assert_eq!(vrmc["colliders"].as_array().unwrap().len(), 1);
        let c0 = &vrmc["colliders"][0];
        assert!(c0["shape"]["sphere"].is_object());
        assert!(vrmc["colliderGroups"].is_array());
        assert_eq!(
            vrmc["springs"][0]["colliderGroups"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[cfg(test)]
mod extended_emit_integration_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emitted_glb_with_plane_collider_declares_extended_collider_in_extensions_used() {
        let mtoon = MToonParams::defaults("test_plane");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
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
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let used = doc["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = used.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            names.contains(&"VRMC_springBone"),
            "extensionsUsed must declare VRMC_springBone: {names:?}"
        );
        assert!(
            names.contains(&"VRMC_springBone_extended_collider"),
            "extensionsUsed must declare VRMC_springBone_extended_collider when plane shape used: {names:?}"
        );
    }

    #[test]
    fn emitted_glb_with_base_sphere_collider_does_not_declare_extended_collider() {
        let mtoon = MToonParams::defaults("test_sphere_no_ext");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let used = doc["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = used.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            !names.contains(&"VRMC_springBone_extended_collider"),
            "base sphere collider must NOT declare VRMC_springBone_extended_collider: {names:?}"
        );
    }
}
