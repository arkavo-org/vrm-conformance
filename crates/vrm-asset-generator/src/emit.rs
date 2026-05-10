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
use crate::spring_bone::SpringBoneParams;
use crate::vrm_ext::vrmc_spring_bone;

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

/// Emit a `.vrm` GLB containing both MToon material data and a single
/// VRMC_springBone chain attached to the head bone.
pub fn emit_vrm_with_spring_bone(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    output: &Utf8Path,
) -> Result<()> {
    let mesh = sphere(0.3, 24, 48);
    let packed = pack_mesh(&mesh);

    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
    );

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

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
        "meshes": [{
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
        }],
        "materials": [base_material(mtoon)],
        "extensions": {
            "VRMC_vrm": vrmc_vrm(&mtoon.id, &skeleton.bone_to_node, mesh_node_index),
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
    let plan = build_default_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}
