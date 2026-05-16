//! Top-level VRM 1.0 asset emission. Combines mesh, buffer, humanoid stub,
//! and VRM extensions into a single `.vrm` GLB on disk.

use crate::buffer::pack_mesh;
use crate::glb::{write_glb, GlbDocument};
use crate::humanoid::minimal_skeleton;
use crate::mesh::sphere;
use crate::params::MToonParams;
use crate::vrm_ext::{base_material, vrmc_vrm};
use anyhow::Result;
use camino::Utf8Path;
use serde_json::{json, Value};

pub fn emit_vrm(params: &MToonParams, output: &Utf8Path) -> Result<()> {
    // 1) Mesh + buffer
    let mesh = sphere(0.3, 24, 48); // small radius so the sphere fits at avatar chest height
    let packed = pack_mesh(&mesh);

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

    // 4) Build the glTF JSON document
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
            "VRMC_vrm": vrmc_vrm(&params.id, &skeleton.bone_to_node, mesh_node_index)
        }
    });

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

use crate::sidecar::{build_default_test_plan, write_meta_json, write_test_yaml};
use crate::spring_bone::{ColliderAttach, ColliderShape, SpringBoneParams, SpringBoneSceneParams};
use crate::vrm_ext::{vrmc_spring_bone, vrmc_spring_bone_scene};

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
