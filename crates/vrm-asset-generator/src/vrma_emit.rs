//! .vrma (VRMC_vrm_animation) glTF document builder. Mirrors the .vrm
//! emission flow in emit.rs but produces an animation-only glTF
//! document (no mesh, no materials) per the VRMA spec.
//!
//! Per the spec:
//!   - `extensionsUsed` must list "VRMC_vrm_animation"
//!   - `extensions.VRMC_vrm_animation.specVersion` is required ("1.0")
//!   - `humanoid`, `expressions`, `lookAt` are each independently optional
//!   - The first `animations[]` entry is the portable clip

use crate::glb::{write_glb, GlbDocument};
use serde_json::{json, Value};

/// Build a minimal-valid `.vrma` JSON document: extension declaration
/// and a placeholder empty animation. Subsequent helpers in this module
/// (humanoid / expression / lookAt) mutate the returned `Value` to add
/// channels.
pub fn build_empty_vrma() -> Value {
    json!({
        "asset": { "version": "2.0", "generator": "vrm-asset-generator (vrma-v1)" },
        "nodes": [],
        "animations": [
            { "channels": [], "samplers": [] }
        ],
        "extensionsUsed": ["VRMC_vrm_animation"],
        "extensions": {
            "VRMC_vrm_animation": { "specVersion": "1.0" }
        }
    })
}

/// Add a humanoid bone rotation animation channel to the document.
///
/// `node_index` is the glTF node index in `doc["nodes"]`.
/// `bone_name` is the VRMA humanoid bone name (one of the 55 names in
/// the spec; the 15 required names are most useful).
/// `keyframes` is `[(time_seconds, [x, y, z, w])]`; values are
/// node-local rotation quaternions.
///
/// Side effects:
/// - Appends entries to `doc["animations"][0]["samplers"]` and `channels`
/// - Appends accessors + bufferViews to top-level arrays (creating them
///   if missing)
/// - Appends raw bytes to `buffer`, 4-byte aligned
/// - Adds `doc["buffers"][0]` if missing; updates its byteLength
/// - Updates `doc["extensions"]["VRMC_vrm_animation"]["humanoid"]
///   .["humanBones"][bone_name].node` to `node_index`
pub fn add_humanoid_bone_rotation_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    bone_name: &str,
    keyframes: &[(f32, [f32; 4])],
) {
    ensure_buffer_infrastructure(doc);

    // 1. Write timestamp buffer (f32 LE)
    let timestamps: Vec<f32> = keyframes.iter().map(|(t, _)| *t).collect();
    let timestamp_offset = buffer.len();
    for t in &timestamps {
        buffer.extend_from_slice(&t.to_le_bytes());
    }
    let timestamp_byte_length = buffer.len() - timestamp_offset;
    align_to_4(buffer);

    // 2. Write values buffer (VEC4 LE)
    let values_offset = buffer.len();
    for (_, q) in keyframes {
        for component in q {
            buffer.extend_from_slice(&component.to_le_bytes());
        }
    }
    let values_byte_length = buffer.len() - values_offset;
    align_to_4(buffer);

    // 3. Add bufferViews
    let bv_arr = doc["bufferViews"].as_array_mut().unwrap();
    let timestamp_bv = bv_arr.len();
    bv_arr.push(json!({
        "buffer": 0,
        "byteOffset": timestamp_offset,
        "byteLength": timestamp_byte_length,
    }));
    let values_bv = bv_arr.len();
    bv_arr.push(json!({
        "buffer": 0,
        "byteOffset": values_offset,
        "byteLength": values_byte_length,
    }));

    // 4. Add accessors
    let acc_arr = doc["accessors"].as_array_mut().unwrap();
    let timestamp_acc = acc_arr.len();
    let max_time = timestamps.iter().copied().fold(0.0_f32, f32::max);
    acc_arr.push(json!({
        "bufferView": timestamp_bv,
        "componentType": 5126,
        "count": timestamps.len(),
        "type": "SCALAR",
        "min": [0.0],
        "max": [max_time],
    }));
    let values_acc = acc_arr.len();
    acc_arr.push(json!({
        "bufferView": values_bv,
        "componentType": 5126,
        "count": keyframes.len(),
        "type": "VEC4",
    }));

    // 5. Add sampler + channel
    let anim = doc["animations"][0].as_object_mut().unwrap();
    let samplers = anim.get_mut("samplers").unwrap().as_array_mut().unwrap();
    let sampler_index = samplers.len();
    samplers.push(json!({
        "input": timestamp_acc,
        "output": values_acc,
        "interpolation": "LINEAR",
    }));
    let channels = anim.get_mut("channels").unwrap().as_array_mut().unwrap();
    channels.push(json!({
        "sampler": sampler_index,
        "target": { "node": node_index, "path": "rotation" },
    }));

    // 6. Update VRMC_vrm_animation.humanoid.humanBones
    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    let humanoid = ext
        .entry("humanoid")
        .or_insert_with(|| json!({ "humanBones": {} }));
    let human_bones = humanoid["humanBones"].as_object_mut().unwrap();
    human_bones.insert(bone_name.to_string(), json!({ "node": node_index }));

    // 7. Update buffer byteLength
    doc["buffers"][0]["byteLength"] = json!(buffer.len());
}

fn ensure_buffer_infrastructure(doc: &mut Value) {
    if doc.get("accessors").is_none() {
        doc["accessors"] = json!([]);
    }
    if doc.get("bufferViews").is_none() {
        doc["bufferViews"] = json!([]);
    }
    if doc.get("buffers").is_none() {
        doc["buffers"] = json!([{ "byteLength": 0 }]);
    }
}

fn align_to_4(buffer: &mut Vec<u8>) {
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }
}

/// Serialize a complete `.vrma` document (JSON + optional binary buffer)
/// to a GLB byte stream.
pub fn write_vrma_glb(json_doc: &Value, buffer: &[u8]) -> anyhow::Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(json_doc)?;
    let doc = GlbDocument {
        json: json_bytes,
        binary: buffer.to_vec(),
    };
    write_glb(&doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::extract_json_chunk;

    #[test]
    fn empty_vrma_is_valid_glb_with_extension() {
        let doc = build_empty_vrma();
        let bytes = write_vrma_glb(&doc, &[]).unwrap();
        assert_eq!(&bytes[..4], b"glTF");

        let json_chunk = extract_json_chunk(&bytes).expect("GLB has JSON chunk");
        let parsed: Value = serde_json::from_slice(&json_chunk).unwrap();
        let used: Vec<&str> = parsed["extensionsUsed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().unwrap())
            .collect();
        assert!(
            used.contains(&"VRMC_vrm_animation"),
            "extensionsUsed must list VRMC_vrm_animation, got {used:?}"
        );
        assert_eq!(parsed["extensions"]["VRMC_vrm_animation"]["specVersion"], "1.0");
    }

    #[test]
    fn humanoid_bone_rotation_emits_one_channel() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();

        let nodes_arr = doc["nodes"].as_array_mut().unwrap();
        nodes_arr.push(json!({ "name": "head" }));

        add_humanoid_bone_rotation_channel(
            &mut doc,
            &mut buffer,
            0,
            "head",
            &[(0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]), (1.0, [0.0, std::f32::consts::FRAC_1_SQRT_2, 0.0, std::f32::consts::FRAC_1_SQRT_2])],
        );

        let humanoid = &doc["extensions"]["VRMC_vrm_animation"]["humanoid"];
        assert_eq!(humanoid["humanBones"]["head"]["node"], 0);

        let anim = &doc["animations"][0];
        let channels = anim["channels"].as_array().unwrap();
        let samplers = anim["samplers"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(samplers.len(), 1);
        assert_eq!(channels[0]["target"]["path"], "rotation");
        assert_eq!(channels[0]["target"]["node"], 0);

        // Buffer should now hold timestamps (2 × f32 = 8 bytes) + quaternions (2 × 4 × f32 = 32 bytes), aligned.
        assert!(buffer.len() >= 40, "buffer too small: {}", buffer.len());
        assert_eq!(buffer.len() % 4, 0, "buffer not 4-aligned");
    }
}
