//! glTF 2.0 buffer/bufferView/accessor builder for our generated meshes.
//!
//! Produces a single packed binary blob plus the JSON fragments for one
//! buffer, four bufferViews, and four accessors covering: positions
//! (VEC3 FLOAT), normals (VEC3 FLOAT), uvs (VEC2 FLOAT), indices
//! (SCALAR UNSIGNED_INT).

use crate::mesh::MeshData;
use serde_json::{json, Value};

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
