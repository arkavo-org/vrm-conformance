//! glTF 2.0 buffer/bufferView/accessor builder for our generated meshes.
//!
//! Produces a single packed binary blob plus the JSON fragments for one
//! buffer, four bufferViews, and four accessors covering: positions
//! (VEC3 FLOAT), normals (VEC3 FLOAT), uvs (VEC2 FLOAT), indices
//! (SCALAR UNSIGNED_INT).

use crate::chain_mesh::SkinnedMeshData;
use crate::mesh::MeshData;
use serde_json::{json, Value};

const GL_UNSIGNED_SHORT: u32 = 5123;
const GL_UNSIGNED_INT: u32 = 5125;
const GL_FLOAT: u32 = 5126;
const TARGET_ARRAY_BUFFER: u32 = 34962;
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

#[derive(Debug, Clone)]
pub struct PackedMesh {
    pub binary: Vec<u8>,
    pub json: Value,
}

fn align_to(v: &mut Vec<u8>, alignment: usize) {
    let pad = (alignment - v.len() % alignment) % alignment;
    v.resize(v.len() + pad, 0);
}

fn write_vec3_array(out: &mut Vec<u8>, data: &[[f32; 3]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

fn write_vec2_array(out: &mut Vec<u8>, data: &[[f32; 2]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

fn write_u32_array(out: &mut Vec<u8>, data: &[u32]) -> (usize, usize) {
    let offset = out.len();
    for x in data {
        out.extend_from_slice(&x.to_le_bytes());
    }
    let len = out.len() - offset;
    (offset, len)
}

fn min_max_vec3(data: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in data {
        for i in 0..3 {
            if v[i] < min[i] {
                min[i] = v[i];
            }
            if v[i] > max[i] {
                max[i] = v[i];
            }
        }
    }
    (min, max)
}

fn write_u16_vec4_array(out: &mut Vec<u8>, data: &[[u16; 4]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

fn write_f32_vec4_array(out: &mut Vec<u8>, data: &[[f32; 4]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

fn write_mat4_array(out: &mut Vec<u8>, data: &[[f32; 16]]) -> (usize, usize) {
    let offset = out.len();
    for v in data {
        for c in v {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = out.len() - offset;
    (offset, len)
}

pub fn pack_mesh(mesh: &MeshData) -> PackedMesh {
    let mut bin: Vec<u8> = Vec::new();

    // 1) positions
    let (pos_off, pos_len) = write_vec3_array(&mut bin, &mesh.positions);
    align_to(&mut bin, 4);
    // 2) normals
    let (nrm_off, nrm_len) = write_vec3_array(&mut bin, &mesh.normals);
    align_to(&mut bin, 4);
    // 3) uvs
    let (uv_off, uv_len) = write_vec2_array(&mut bin, &mesh.uvs);
    align_to(&mut bin, 4);
    // 4) indices
    let (idx_off, idx_len) = write_u32_array(&mut bin, &mesh.indices);

    let (pos_min, pos_max) = min_max_vec3(&mesh.positions);

    let json = json!({
        "buffers": [
            { "byteLength": bin.len() }
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_off, "byteLength": pos_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": nrm_off, "byteLength": nrm_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": uv_off,  "byteLength": uv_len,  "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": idx_off, "byteLength": idx_len, "target": TARGET_ELEMENT_ARRAY_BUFFER }
        ],
        "accessors": [
            {
                "bufferView": 0, "componentType": GL_FLOAT,
                "count": mesh.positions.len(), "type": "VEC3",
                "min": pos_min, "max": pos_max
            },
            {
                "bufferView": 1, "componentType": GL_FLOAT,
                "count": mesh.normals.len(), "type": "VEC3"
            },
            {
                "bufferView": 2, "componentType": GL_FLOAT,
                "count": mesh.uvs.len(), "type": "VEC2"
            },
            {
                "bufferView": 3, "componentType": GL_UNSIGNED_INT,
                "count": mesh.indices.len(), "type": "SCALAR"
            }
        ]
    });

    PackedMesh { binary: bin, json }
}

/// Pack BOTH the head-mounted sphere AND the spring-bone chain cylinder
/// (with its skin's inverse-bind matrices) into a single binary buffer
/// + JSON fragment.
///
/// Accessor layout: 0..3 sphere (pos/nrm/uv/idx), 4..9 chain (adds
/// joints + weights), 10 inverse-bind matrices.
///
/// NB: currently unused — kept in-tree as deferred infrastructure for
/// the chain-skinned-mesh story (see `chain_mesh` module). The
/// integration into `emit_vrm_with_spring_bone` was drafted and
/// reverted; see `chain_mesh.rs` docs for the VRMMetalKit interop
/// quirk that blocks visible spring-bone diffing.
pub fn pack_sphere_and_chain(
    sphere: &MeshData,
    chain: &SkinnedMeshData,
    inverse_bind_matrices: &[[f32; 16]],
) -> PackedMesh {
    let mut bin: Vec<u8> = Vec::new();

    // ----- sphere -----
    let (s_pos_off, s_pos_len) = write_vec3_array(&mut bin, &sphere.positions);
    align_to(&mut bin, 4);
    let (s_nrm_off, s_nrm_len) = write_vec3_array(&mut bin, &sphere.normals);
    align_to(&mut bin, 4);
    let (s_uv_off, s_uv_len) = write_vec2_array(&mut bin, &sphere.uvs);
    align_to(&mut bin, 4);
    let (s_idx_off, s_idx_len) = write_u32_array(&mut bin, &sphere.indices);
    align_to(&mut bin, 4);

    // ----- chain (positions, normals, uvs, indices, joints, weights) -----
    let (c_pos_off, c_pos_len) = write_vec3_array(&mut bin, &chain.positions);
    align_to(&mut bin, 4);
    let (c_nrm_off, c_nrm_len) = write_vec3_array(&mut bin, &chain.normals);
    align_to(&mut bin, 4);
    let (c_uv_off, c_uv_len) = write_vec2_array(&mut bin, &chain.uvs);
    align_to(&mut bin, 4);
    let (c_idx_off, c_idx_len) = write_u32_array(&mut bin, &chain.indices);
    align_to(&mut bin, 4);
    let (c_j_off, c_j_len) = write_u16_vec4_array(&mut bin, &chain.joints);
    align_to(&mut bin, 4);
    let (c_w_off, c_w_len) = write_f32_vec4_array(&mut bin, &chain.weights);
    align_to(&mut bin, 4);

    // ----- inverse-bind matrices -----
    let (ibm_off, ibm_len) = write_mat4_array(&mut bin, inverse_bind_matrices);

    let (s_pos_min, s_pos_max) = min_max_vec3(&sphere.positions);
    let (c_pos_min, c_pos_max) = min_max_vec3(&chain.positions);

    let json = json!({
        "buffers": [
            { "byteLength": bin.len() }
        ],
        "bufferViews": [
            // 0..3: sphere
            { "buffer": 0, "byteOffset": s_pos_off, "byteLength": s_pos_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": s_nrm_off, "byteLength": s_nrm_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": s_uv_off,  "byteLength": s_uv_len,  "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": s_idx_off, "byteLength": s_idx_len, "target": TARGET_ELEMENT_ARRAY_BUFFER },
            // 4..9: chain
            { "buffer": 0, "byteOffset": c_pos_off, "byteLength": c_pos_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": c_nrm_off, "byteLength": c_nrm_len, "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": c_uv_off,  "byteLength": c_uv_len,  "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": c_idx_off, "byteLength": c_idx_len, "target": TARGET_ELEMENT_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": c_j_off,   "byteLength": c_j_len,   "target": TARGET_ARRAY_BUFFER },
            { "buffer": 0, "byteOffset": c_w_off,   "byteLength": c_w_len,   "target": TARGET_ARRAY_BUFFER },
            // 10: inverse-bind matrices (no target — animation/skin data, not a vertex/index buffer)
            { "buffer": 0, "byteOffset": ibm_off,   "byteLength": ibm_len }
        ],
        "accessors": [
            // 0..3: sphere
            {
                "bufferView": 0, "componentType": GL_FLOAT,
                "count": sphere.positions.len(), "type": "VEC3",
                "min": s_pos_min, "max": s_pos_max
            },
            { "bufferView": 1, "componentType": GL_FLOAT, "count": sphere.normals.len(), "type": "VEC3" },
            { "bufferView": 2, "componentType": GL_FLOAT, "count": sphere.uvs.len(), "type": "VEC2" },
            { "bufferView": 3, "componentType": GL_UNSIGNED_INT, "count": sphere.indices.len(), "type": "SCALAR" },
            // 4..9: chain
            {
                "bufferView": 4, "componentType": GL_FLOAT,
                "count": chain.positions.len(), "type": "VEC3",
                "min": c_pos_min, "max": c_pos_max
            },
            { "bufferView": 5, "componentType": GL_FLOAT, "count": chain.normals.len(), "type": "VEC3" },
            { "bufferView": 6, "componentType": GL_FLOAT, "count": chain.uvs.len(), "type": "VEC2" },
            { "bufferView": 7, "componentType": GL_UNSIGNED_INT, "count": chain.indices.len(), "type": "SCALAR" },
            { "bufferView": 8, "componentType": GL_UNSIGNED_SHORT, "count": chain.joints.len(), "type": "VEC4" },
            { "bufferView": 9, "componentType": GL_FLOAT, "count": chain.weights.len(), "type": "VEC4" },
            // 10: inverse-bind matrices
            { "bufferView": 10, "componentType": GL_FLOAT, "count": inverse_bind_matrices.len(), "type": "MAT4" }
        ]
    });

    PackedMesh { binary: bin, json }
}

/// Pack a sphere mesh plus N chain cylinders (each with its own inverse-bind matrices)
/// into a single binary buffer + JSON fragment.
///
/// Accessor layout:
/// - 0..3: sphere (pos/nrm/uv/idx)
/// - For each chain i: accessors `4 + i*7` through `4 + i*7 + 5` are chain i data
///   (pos/nrm/uv/idx/joints/weights), accessor `4 + i*7 + 6` is the inverse-bind matrices.
///
/// This generalisation of `pack_sphere_and_chain` supports N>1 chains for
/// the multi-chain emit path (phase 6).
pub fn pack_sphere_and_multichains(
    sphere: &MeshData,
    chains: &[(&SkinnedMeshData, &[[f32; 16]])],
) -> PackedMesh {
    let mut bin: Vec<u8> = Vec::new();

    // ----- sphere -----
    let (s_pos_off, s_pos_len) = write_vec3_array(&mut bin, &sphere.positions);
    align_to(&mut bin, 4);
    let (s_nrm_off, s_nrm_len) = write_vec3_array(&mut bin, &sphere.normals);
    align_to(&mut bin, 4);
    let (s_uv_off, s_uv_len) = write_vec2_array(&mut bin, &sphere.uvs);
    align_to(&mut bin, 4);
    let (s_idx_off, s_idx_len) = write_u32_array(&mut bin, &sphere.indices);
    align_to(&mut bin, 4);

    // Per-chain data: (pos, nrm, uv, idx, joints, weights, ibm)
    struct ChainOffsets {
        pos: (usize, usize),
        nrm: (usize, usize),
        uv: (usize, usize),
        idx: (usize, usize),
        jnt: (usize, usize),
        wgt: (usize, usize),
        ibm: (usize, usize),
    }

    let mut chain_offsets: Vec<ChainOffsets> = Vec::with_capacity(chains.len());
    for (chain, ibm) in chains {
        let pos = write_vec3_array(&mut bin, &chain.positions);
        align_to(&mut bin, 4);
        let nrm = write_vec3_array(&mut bin, &chain.normals);
        align_to(&mut bin, 4);
        let uv = write_vec2_array(&mut bin, &chain.uvs);
        align_to(&mut bin, 4);
        let idx = write_u32_array(&mut bin, &chain.indices);
        align_to(&mut bin, 4);
        let jnt = write_u16_vec4_array(&mut bin, &chain.joints);
        align_to(&mut bin, 4);
        let wgt = write_f32_vec4_array(&mut bin, &chain.weights);
        align_to(&mut bin, 4);
        let ibm_data = write_mat4_array(&mut bin, ibm);
        align_to(&mut bin, 4);
        chain_offsets.push(ChainOffsets {
            pos,
            nrm,
            uv,
            idx,
            jnt,
            wgt,
            ibm: ibm_data,
        });
    }

    let (s_pos_min, s_pos_max) = min_max_vec3(&sphere.positions);

    // Build buffer views: sphere (4) + per-chain 7 each.
    let mut buffer_views: Vec<Value> = vec![
        json!({ "buffer": 0, "byteOffset": s_pos_off, "byteLength": s_pos_len, "target": TARGET_ARRAY_BUFFER }),
        json!({ "buffer": 0, "byteOffset": s_nrm_off, "byteLength": s_nrm_len, "target": TARGET_ARRAY_BUFFER }),
        json!({ "buffer": 0, "byteOffset": s_uv_off,  "byteLength": s_uv_len,  "target": TARGET_ARRAY_BUFFER }),
        json!({ "buffer": 0, "byteOffset": s_idx_off, "byteLength": s_idx_len, "target": TARGET_ELEMENT_ARRAY_BUFFER }),
    ];

    let mut accessors: Vec<Value> = vec![
        json!({
            "bufferView": 0, "componentType": GL_FLOAT,
            "count": sphere.positions.len(), "type": "VEC3",
            "min": s_pos_min, "max": s_pos_max
        }),
        json!({ "bufferView": 1, "componentType": GL_FLOAT, "count": sphere.normals.len(), "type": "VEC3" }),
        json!({ "bufferView": 2, "componentType": GL_FLOAT, "count": sphere.uvs.len(), "type": "VEC2" }),
        json!({ "bufferView": 3, "componentType": GL_UNSIGNED_INT, "count": sphere.indices.len(), "type": "SCALAR" }),
    ];

    for (ci, (co, (chain, _ibm))) in chain_offsets.iter().zip(chains.iter()).enumerate() {
        let bv_base = buffer_views.len();
        let (c_pos_min, c_pos_max) = min_max_vec3(&chain.positions);
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.pos.0, "byteLength": co.pos.1, "target": TARGET_ARRAY_BUFFER }));
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.nrm.0, "byteLength": co.nrm.1, "target": TARGET_ARRAY_BUFFER }));
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.uv.0,  "byteLength": co.uv.1,  "target": TARGET_ARRAY_BUFFER }));
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.idx.0, "byteLength": co.idx.1, "target": TARGET_ELEMENT_ARRAY_BUFFER }));
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.jnt.0, "byteLength": co.jnt.1, "target": TARGET_ARRAY_BUFFER }));
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.wgt.0, "byteLength": co.wgt.1, "target": TARGET_ARRAY_BUFFER }));
        buffer_views.push(json!({ "buffer": 0, "byteOffset": co.ibm.0, "byteLength": co.ibm.1 }));

        let _ = ci; // Used only for debug labelling if needed.
        accessors.push(json!({
            "bufferView": bv_base, "componentType": GL_FLOAT,
            "count": chain.positions.len(), "type": "VEC3",
            "min": c_pos_min, "max": c_pos_max
        }));
        accessors.push(json!({ "bufferView": bv_base + 1, "componentType": GL_FLOAT, "count": chain.normals.len(), "type": "VEC3" }));
        accessors.push(json!({ "bufferView": bv_base + 2, "componentType": GL_FLOAT, "count": chain.uvs.len(), "type": "VEC2" }));
        accessors.push(json!({ "bufferView": bv_base + 3, "componentType": GL_UNSIGNED_INT, "count": chain.indices.len(), "type": "SCALAR" }));
        accessors.push(json!({ "bufferView": bv_base + 4, "componentType": GL_UNSIGNED_SHORT, "count": chain.joints.len(), "type": "VEC4" }));
        accessors.push(json!({ "bufferView": bv_base + 5, "componentType": GL_FLOAT, "count": chain.weights.len(), "type": "VEC4" }));
        accessors.push(json!({ "bufferView": bv_base + 6, "componentType": GL_FLOAT, "count": _ibm.len(), "type": "MAT4" }));
    }

    let json = json!({
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": buffer_views,
        "accessors": accessors,
    });

    PackedMesh { binary: bin, json }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain_mesh::build_chain_cylinder;
    use crate::mesh::sphere;

    #[test]
    fn pack_sphere_and_chain_emits_11_accessors() {
        let s = sphere(0.3, 8, 16);
        let c = build_chain_cylinder(4, 0.05, 0.02, 1.31, 8);
        let ibm: Vec<[f32; 16]> = (0..4).map(|_| [0.0; 16]).collect();
        let packed = pack_sphere_and_chain(&s, &c, &ibm);

        let accessors = packed.json["accessors"].as_array().unwrap();
        assert_eq!(accessors.len(), 11, "sphere(4) + chain(6) + ibm(1) = 11");

        let buffer_views = packed.json["bufferViews"].as_array().unwrap();
        assert_eq!(buffer_views.len(), 11);

        // Sphere accessors first (0..3).
        assert_eq!(accessors[0]["type"], "VEC3");
        assert_eq!(accessors[3]["type"], "SCALAR");

        // Chain accessors next (4..9): pos, nrm, uv, idx, joints, weights.
        assert_eq!(accessors[4]["type"], "VEC3");
        assert_eq!(accessors[8]["type"], "VEC4");
        assert_eq!(accessors[8]["componentType"], GL_UNSIGNED_SHORT);
        assert_eq!(accessors[9]["type"], "VEC4");
        assert_eq!(accessors[9]["componentType"], GL_FLOAT);

        // Last: inverse-bind matrices.
        assert_eq!(accessors[10]["type"], "MAT4");
        assert_eq!(accessors[10]["count"], 4);

        // Binary buffer is non-empty and matches the JSON-declared length.
        let declared = packed.json["buffers"][0]["byteLength"].as_u64().unwrap();
        assert_eq!(packed.binary.len() as u64, declared);
        assert!(!packed.binary.is_empty());
    }
}
