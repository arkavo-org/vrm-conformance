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
///
/// `scene` + `scenes` are included so that UniVRM 0.131.0's
/// `VrmAnimationImporter.LoadAsync` does not crash on the unconditional
/// `Data.GLTF.scenes[0].nodes = ...` at line 245. The scenes array is
/// populated lazily by `finalize_vrma_scenes` after all nodes have been
/// added; call that function before `write_vrma_glb`.
pub fn build_empty_vrma() -> Value {
    json!({
        "asset": { "version": "2.0", "generator": "vrm-asset-generator (vrma-v1)" },
        "scene": 0,
        "scenes": [{ "nodes": [] }],
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

/// Populate `scenes[0].nodes` with the root-level glTF node indices
/// (nodes that are not referenced as children of any other node).
/// Must be called after all nodes have been appended to `doc["nodes"]`
/// and before `write_vrma_glb`. Safe to call multiple times (idempotent).
pub fn finalize_vrma_scenes(doc: &mut Value) {
    use std::collections::HashSet;
    let nodes = match doc["nodes"].as_array() {
        Some(n) => n,
        None => return,
    };
    let mut referenced: HashSet<usize> = HashSet::new();
    for node in nodes {
        if let Some(children) = node["children"].as_array() {
            for c in children {
                if let Some(idx) = c.as_u64() {
                    referenced.insert(idx as usize);
                }
            }
        }
    }
    let root_indices: Vec<serde_json::Value> = (0..nodes.len())
        .filter(|i| !referenced.contains(i))
        .map(serde_json::Value::from)
        .collect();
    // scenes[0] is guaranteed to exist — build_empty_vrma initialises it.
    doc["scenes"][0]["nodes"] = serde_json::Value::Array(root_indices);
}

/// Register all bones in `bone_to_node` into the VRMA `humanBones` map
/// without emitting any animation channels. This is required so that
/// UniVRM 0.131.0's VrmAnimationImporter can build a valid Unity
/// `HumanAvatar` (which requires at minimum hips, spine, head, and the
/// four limb bones). Call this after all nodes have been set on `doc`
/// and before emitting any rotation channels.
pub fn register_all_humanoid_bones(
    doc: &mut Value,
    bone_to_node: &std::collections::BTreeMap<String, usize>,
) {
    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    let humanoid = ext
        .entry("humanoid")
        .or_insert_with(|| json!({ "humanBones": {} }));
    let human_bones = humanoid["humanBones"].as_object_mut().unwrap();
    for (bone_name, &node_idx) in bone_to_node {
        human_bones
            .entry(bone_name.clone())
            .or_insert_with(|| json!({ "node": node_idx }));
    }
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
    add_node_rotation_channel(doc, buffer, node_index, keyframes);

    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    let humanoid = ext
        .entry("humanoid")
        .or_insert_with(|| json!({ "humanBones": {} }));
    let human_bones = humanoid["humanBones"].as_object_mut().unwrap();
    human_bones.insert(bone_name.to_string(), json!({ "node": node_index }));
}

/// Add a lookAt gaze direction animation channel to the document.
///
/// `node_index` is the glTF node index whose rotation encodes the gaze quaternion.
/// `offset_from_head_bone` is the 3-vec offset used by the VRMA spec.
/// `keyframes` is `[(time_seconds, [x, y, z, w])]`.
///
/// Side effects:
/// - Writes a `node.rotation` animation channel (same binary path as humanoid bone rotation)
/// - Sets `doc["extensions"]["VRMC_vrm_animation"]["lookAt"]` to
///   `{ "node": node_index, "offsetFromHeadBone": [...] }`
/// - Does NOT touch `humanoid.humanBones`
pub fn add_look_at_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    offset_from_head_bone: [f32; 3],
    keyframes: &[(f32, [f32; 4])],
) {
    add_node_rotation_channel(doc, buffer, node_index, keyframes);

    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    ext.insert(
        "lookAt".to_string(),
        json!({
            "node": node_index,
            "offsetFromHeadBone": offset_from_head_bone,
        }),
    );
}

/// Whether an expression is one of the 14 spec-defined presets or a
/// caller-named custom expression.
#[derive(Debug, Clone, Copy)]
pub enum ExpressionKind<'a> {
    Preset(&'a str),
    Custom(&'a str),
}

/// Add an expression weight animation channel.
///
/// Per the VRMA spec, weights are the X-component of node.translation
/// animation. Y and Z are always 0.
pub fn add_expression_weight_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    kind: ExpressionKind,
    keyframes: &[(f32, f32)],
) {
    let translation_keyframes: Vec<(f32, [f32; 3])> = keyframes
        .iter()
        .map(|(t, w)| (*t, [*w, 0.0, 0.0]))
        .collect();
    add_node_translation_channel(doc, buffer, node_index, &translation_keyframes);

    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    let expressions = ext
        .entry("expressions")
        .or_insert_with(|| json!({ "preset": {}, "custom": {} }));
    let expressions_obj = expressions.as_object_mut().unwrap();
    expressions_obj
        .entry("preset".to_string())
        .or_insert_with(|| json!({}));
    expressions_obj
        .entry("custom".to_string())
        .or_insert_with(|| json!({}));

    let (key, name) = match kind {
        ExpressionKind::Preset(n) => ("preset", n),
        ExpressionKind::Custom(n) => ("custom", n),
    };
    let category = expressions_obj[key].as_object_mut().unwrap();
    category.insert(name.to_string(), json!({ "node": node_index }));
}

fn add_node_rotation_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
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

    // 6. Update buffer byteLength
    doc["buffers"][0]["byteLength"] = json!(buffer.len());
}

fn add_node_translation_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    keyframes: &[(f32, [f32; 3])],
) {
    ensure_buffer_infrastructure(doc);

    let timestamps: Vec<f32> = keyframes.iter().map(|(t, _)| *t).collect();
    let timestamp_offset = buffer.len();
    for t in &timestamps {
        buffer.extend_from_slice(&t.to_le_bytes());
    }
    let timestamp_byte_length = buffer.len() - timestamp_offset;
    align_to_4(buffer);

    let values_offset = buffer.len();
    for (_, v) in keyframes {
        for c in v {
            buffer.extend_from_slice(&c.to_le_bytes());
        }
    }
    let values_byte_length = buffer.len() - values_offset;
    align_to_4(buffer);

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
        "type": "VEC3",
    }));

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
        "target": { "node": node_index, "path": "translation" },
    }));

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
        assert_eq!(
            parsed["extensions"]["VRMC_vrm_animation"]["specVersion"],
            "1.0"
        );
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
            &[
                (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
                (
                    1.0,
                    [
                        0.0,
                        std::f32::consts::FRAC_1_SQRT_2,
                        0.0,
                        std::f32::consts::FRAC_1_SQRT_2,
                    ],
                ),
            ],
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

    #[test]
    fn expression_weight_emits_translation_channel_with_x_component() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();

        let nodes_arr = doc["nodes"].as_array_mut().unwrap();
        nodes_arr.push(json!({ "name": "happy_expr_target" }));

        add_expression_weight_channel(
            &mut doc,
            &mut buffer,
            0,
            ExpressionKind::Preset("happy"),
            &[(0.0_f32, 0.0_f32), (0.5, 1.0), (1.0, 0.0)],
        );

        let presets = &doc["extensions"]["VRMC_vrm_animation"]["expressions"]["preset"];
        assert_eq!(presets["happy"]["node"], 0);

        let anim = &doc["animations"][0];
        let channels = anim["channels"].as_array().unwrap();
        let last = channels.last().unwrap();
        assert_eq!(last["target"]["path"], "translation");
        assert_eq!(last["target"]["node"], 0);
    }

    #[test]
    fn custom_expression_lands_in_custom_section() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();
        doc["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "name": "smug" }));

        add_expression_weight_channel(
            &mut doc,
            &mut buffer,
            0,
            ExpressionKind::Custom("smug"),
            &[(0.0_f32, 0.0_f32), (1.0, 0.7)],
        );

        let expressions = &doc["extensions"]["VRMC_vrm_animation"]["expressions"];
        assert_eq!(expressions["custom"]["smug"]["node"], 0);
        assert!(
            !expressions
                .get("preset")
                .map(|p| p.as_object().is_some_and(|o| o.contains_key("smug")))
                .unwrap_or(false),
            "custom expression must not leak into preset section"
        );
    }

    #[test]
    fn look_at_emits_node_rotation_channel() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();

        doc["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "name": "look_at_target" }));

        add_look_at_channel(
            &mut doc,
            &mut buffer,
            0,
            [0.0, 0.06, 0.0],
            &[
                (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
                (1.0, [0.0, 0.259, 0.0, 0.966]),
            ],
        );

        let look_at = &doc["extensions"]["VRMC_vrm_animation"]["lookAt"];
        assert_eq!(look_at["node"], 0);
        let offset = look_at["offsetFromHeadBone"].as_array().unwrap();
        assert_eq!(offset.len(), 3);
        assert!(
            (offset[1].as_f64().unwrap() - 0.06_f64).abs() < 1e-5,
            "offsetFromHeadBone[1] should be ~0.06, got {}",
            offset[1]
        );

        // Must NOT pollute humanBones with a placeholder.
        let humanoid = doc["extensions"]["VRMC_vrm_animation"].get("humanoid");
        if let Some(h) = humanoid {
            if let Some(bones) = h.get("humanBones") {
                let bone_names: Vec<&String> = bones.as_object().unwrap().keys().collect();
                assert!(
                    !bone_names
                        .iter()
                        .any(|n| n.contains("look_at") || n.contains("placeholder")),
                    "lookAt must not pollute humanBones, got {bone_names:?}"
                );
            }
        }

        // A rotation channel must exist for the lookAt node.
        let anim = &doc["animations"][0];
        let channels = anim["channels"].as_array().unwrap();
        let has_rotation_for_node_0 = channels
            .iter()
            .any(|c| c["target"]["node"] == 0 && c["target"]["path"] == "rotation");
        assert!(
            has_rotation_for_node_0,
            "expected rotation channel on node 0 for lookAt"
        );
    }
}
