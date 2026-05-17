# VRMA Phase 3 — Asset Generator (.vrma Emission + 3 Sweep Subcommands)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the asset generator's VRMA emission path: a `.vrma` file builder + three new `emit-vrma-*` subcommands producing paired triplets (`.vrm` + `.vrma` + `.test.yaml`). After this phase, `cargo run -p vrm-asset-generator -- emit-vrma-humanoid-sweep --output-dir /tmp/x` produces ~15 humanoid-bone sweep plans the runner can drive end-to-end through the mock renderer (and through real adapters once phase 4-5 lands them).

**Architecture:** New `vrma_emit` module owns `.vrma` glTF document construction (humanoid bone rotation channels, expression weight channels, lookAt gaze channels). The existing `humanoid::Skeleton` already provides all 15 required + 4 optional humanoid bones; phase 3 just needs to reference its `bone_to_node` map when emitting humanoid animation channels. Three new sweep functions in `sweep.rs` (and three new emit functions in `emit.rs`) parallel the existing `mtoon_basic_sweep` and `spring_bone_*_sweep` shapes. Sidecar test-plan emission already handles the `animation.vrma` field shape from phase 2.

**Tech Stack:** Rust workspace only — `vrm-asset-generator` crate. The runner (phase 2 substrate) and `vrm-ops` (phase 1 op types) are unchanged.

**Spec:** [`docs/superpowers/specs/2026-05-17-vrma-conformance-design.md`](../specs/2026-05-17-vrma-conformance-design.md) — Asset model section. The canonical VRMC_vrm_animation-1.0 spec is at [`vrm-c/vrm-specification`](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm_animation-1.0).

**Builds on:**
- Phase 1 (op surface, commits `36b663d..fab903c`)
- Phase 2 (runner substrate, commits `1e73346..a344436`)
- The MToon alpha sweep detour (commits `8959abe`, `4eda911`, `ebb0686`) demonstrated the generator → bootstrap → consensus-report flow works for new corpus families.

---

## File structure

**Create:**
- `crates/vrm-asset-generator/src/vrma_emit.rs` — `.vrma` glTF document builder + animation channel writers
- `crates/vrm-asset-generator/src/vrma_params.rs` — parameter dictionaries: `VrmaHumanoidParams`, `VrmaExpressionParams`, `VrmaLookAtParams`

**Modify:**
- `crates/vrm-asset-generator/src/lib.rs` — export `vrma_emit` and `vrma_params` modules
- `crates/vrm-asset-generator/src/sweep.rs` — add `vrma_humanoid_sweep()`, `vrma_expression_sweep()`, `vrma_lookat_sweep()` functions returning `Vec<...Params>`
- `crates/vrm-asset-generator/src/sidecar.rs` — add `build_vrma_*_test_plan()` helpers emitting test-plan YAML with `animation.vrma` block + `pose_tolerance`
- `crates/vrm-asset-generator/src/cli.rs` — add three new `Commands` variants + describe-catalog entries: `EmitVrmaHumanoidSweep`, `EmitVrmaExpressionSweep`, `EmitVrmaLookatSweep`
- `crates/vrm-asset-generator/src/emit.rs` — add `emit_vrma_humanoid_set()`, `emit_vrma_expression_set()`, `emit_vrma_lookat_set()` that write triplets to disk
- `crates/vrm-asset-generator/src/vrm_ext.rs` — add helper for emitting the VRM 1.0 extension with lookAt.type set to either `bone` or `aim` (for lookAt sweep's dual-config corpus)
- `scripts/bootstrap-goldens.sh` — wire the three new emit subcommands

**Reuse (no changes):**
- `crates/vrm-asset-generator/src/humanoid.rs` — `Skeleton` already provides all 15 required humanoid bones. We reference its `bone_to_node: BTreeMap<String, usize>` when wiring `VRMC_vrm_animation.humanoid.humanBones`.

---

## Task 1: `.vrma` glTF document scaffold

**Files:**
- Create: `crates/vrm-asset-generator/src/vrma_emit.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`

The job of this task is to produce a minimal-valid `.vrma` GLB binary with:
- glTF 2.0 header
- One empty `animations[]` entry as a placeholder
- `extensionsUsed: ["VRMC_vrm_animation"]`
- `extensions.VRMC_vrm_animation: { specVersion: "1.0" }`
- No nodes, no humanoid mapping yet — subsequent tasks add channels

This locks the GLB scaffolding so the per-channel tasks only have to deal with adding animation channels + extension fields.

- [ ] **Step 1.1: Add module to lib.rs**

In `crates/vrm-asset-generator/src/lib.rs`, add `pub mod vrma_emit;` next to the existing module declarations (alphabetical).

- [ ] **Step 1.2: Write failing test**

Create `crates/vrm-asset-generator/src/vrma_emit.rs` with the test first (TDD red phase):

```rust
//! .vrma (VRMC_vrm_animation) glTF document builder. Mirrors the .vrm
//! emission flow in emit.rs but produces an animation-only glTF
//! document (no mesh, no materials) per the VRMA spec.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_vrma_is_valid_glb_with_extension() {
        let bytes = emit_empty_vrma_for_test();
        // GLB magic
        assert_eq!(&bytes[..4], b"glTF");
        // Parse JSON chunk and verify it declares VRMC_vrm_animation
        let json = read_glb_json(&bytes);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let used: Vec<&str> = v["extensionsUsed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().unwrap())
            .collect();
        assert!(used.contains(&"VRMC_vrm_animation"), "extensionsUsed must list VRMC_vrm_animation, got {:?}", used);
        assert_eq!(v["extensions"]["VRMC_vrm_animation"]["specVersion"], "1.0");
    }

    fn read_glb_json(bytes: &[u8]) -> String {
        // GLB layout: header (12 bytes) + JSON chunk (length, type, content) + ...
        let chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let chunk_start = 20;
        String::from_utf8(bytes[chunk_start..chunk_start + chunk_len].to_vec()).unwrap().trim_end_matches('\0').trim_end().to_string()
    }
}
```

- [ ] **Step 1.3: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator empty_vrma`
Expected: FAIL — `cannot find function 'emit_empty_vrma_for_test'`.

- [ ] **Step 1.4: Implement the scaffold**

Replace the entire contents of `crates/vrm-asset-generator/src/vrma_emit.rs` with:

```rust
//! .vrma (VRMC_vrm_animation) glTF document builder. Mirrors the .vrm
//! emission flow in emit.rs but produces an animation-only glTF
//! document (no mesh, no materials) per the VRMA spec.
//!
//! Per the spec:
//!   - `extensionsUsed` must list "VRMC_vrm_animation"
//!   - `extensions.VRMC_vrm_animation.specVersion` is required ("1.0")
//!   - `humanoid`, `expressions`, `lookAt` are each independently optional
//!   - The first `animations[]` entry is the portable clip

use crate::buffer::BufferBuilder;
use crate::glb::write_glb;
use serde_json::{json, Value};

/// Build a minimal-valid `.vrma` GLB body: just the extension declaration
/// and a placeholder empty animation. Used as the starting document by
/// the per-channel builders in tasks 2-4 which add humanoid bone
/// rotation channels, expression weight channels, and lookAt channels.
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

/// Write a minimal `.vrma` GLB to a byte vector. Test-only entry point;
/// production callers build the JSON document by adding humanoid /
/// expression / lookAt channels first, then call write_vrma_glb.
#[cfg(test)]
pub fn emit_empty_vrma_for_test() -> Vec<u8> {
    let json_doc = build_empty_vrma();
    let buffer = BufferBuilder::new();
    write_vrma_glb(&json_doc, &buffer.into_inner())
}

/// Write a complete `.vrma` glTF document to a GLB byte stream.
/// `json_doc` is the assembled glTF JSON; `buffer` is the binary chunk
/// data (timestamps + values for animation samplers).
pub fn write_vrma_glb(json_doc: &Value, buffer: &[u8]) -> Vec<u8> {
    write_glb(json_doc, buffer)
}
```

If `BufferBuilder::new()` doesn't exist with that exact API, adapt — look at `crates/vrm-asset-generator/src/buffer.rs` to find the canonical constructor + `into_inner()` accessor (or equivalent). Similarly for `write_glb` in `glb.rs`.

- [ ] **Step 1.5: Verify**

Run: `cargo test -p vrm-asset-generator vrma_emit`
Expected: test passes.

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 1.6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): .vrma GLB scaffold + extension declaration

Phase 3.1 of VRMA closure. New vrma_emit module builds a minimal-valid
.vrma glTF document: VRMC_vrm_animation extension with specVersion "1.0"
and one empty animations[] placeholder. Subsequent tasks add humanoid /
expression / lookAt channels.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Humanoid bone rotation channel emission

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_emit.rs`

Animation channels in glTF have two halves: samplers (input/output buffers) and channels (which sampler drives which node TRS field). For humanoid bone rotation: emit a sampler with timestamps + quaternion values, and a channel pointing at `node.rotation`. The VRMC_vrm_animation extension declares which glTF node represents each humanoid bone.

- [ ] **Step 2.1: Failing test**

Append to `mod tests` in `vrma_emit.rs`:

```rust
    #[test]
    fn humanoid_bone_rotation_emits_one_channel() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();

        // Place a node at index 0 for "head"; map humanoid bone "head" → node 0.
        let nodes_arr = doc["nodes"].as_array_mut().unwrap();
        nodes_arr.push(json!({ "name": "head" }));

        add_humanoid_bone_rotation_channel(
            &mut doc,
            &mut buffer,
            0,  // glTF node index
            "head",  // bone name
            &[(0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]), (1.0, [0.0, 0.7071, 0.0, 0.7071])],
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
    }
```

- [ ] **Step 2.2: Verify fail**

Run: `cargo test -p vrm-asset-generator humanoid_bone_rotation_emits`
Expected: FAIL — function doesn't exist.

- [ ] **Step 2.3: Implement helper**

Append to `vrma_emit.rs` (before `mod tests`):

```rust
/// Add a humanoid bone rotation animation channel to the document.
///
/// `node_index` is the glTF node index in `doc["nodes"]`.
/// `bone_name` is the VRMA humanoid bone name (must be one of the 55
/// names in the spec; the 15 required names are most useful).
/// `keyframes` is `[(time_seconds, [x, y, z, w])]` pairs; values are
/// node-local rotation quaternions.
///
/// Side effects:
/// - Appends entries to `doc["animations"][0]["samplers"]` and `channels`
/// - Appends accessors / bufferViews to top-level arrays (creating them
///   if missing)
/// - Appends raw bytes to `buffer`
/// - Updates `doc["extensions"]["VRMC_vrm_animation"]["humanoid"]
///   ["humanBones"][bone_name]["node"]` to `node_index`
pub fn add_humanoid_bone_rotation_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    bone_name: &str,
    keyframes: &[(f32, [f32; 4])],
) {
    // Ensure top-level arrays exist
    if doc.get("accessors").is_none() {
        doc["accessors"] = json!([]);
    }
    if doc.get("bufferViews").is_none() {
        doc["bufferViews"] = json!([]);
    }
    if doc.get("buffers").is_none() {
        doc["buffers"] = json!([{ "byteLength": 0 }]);
    }

    // 1. Write timestamp buffer
    let timestamps: Vec<f32> = keyframes.iter().map(|(t, _)| *t).collect();
    let timestamp_offset = buffer.len();
    for t in &timestamps {
        buffer.extend_from_slice(&t.to_le_bytes());
    }
    let timestamp_byte_length = buffer.len() - timestamp_offset;
    // 4-byte align
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }

    // 2. Write values buffer
    let values_offset = buffer.len();
    for (_, q) in keyframes {
        for component in q {
            buffer.extend_from_slice(&component.to_le_bytes());
        }
    }
    let values_byte_length = buffer.len() - values_offset;
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }

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
        "componentType": 5126,  // FLOAT
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
```

- [ ] **Step 2.4: Verify**

Run: `cargo test -p vrm-asset-generator humanoid_bone_rotation_emits`
Expected: PASS.

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_emit.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): humanoid bone rotation channel emission

add_humanoid_bone_rotation_channel appends a glTF animation channel
targeting node.rotation + corresponding sampler with timestamp/quaternion
buffers + VRMC_vrm_animation.humanoid.humanBones mapping for the named
bone. Sub-task for the humanoid sweep emit command.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Expression weight channel emission

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_emit.rs`

Per VRMA spec: expression weights are encoded as the X-component of the bound node's translation animation. A node is created per expression, and the animation channel targets `node.translation` with VEC3 values; the X-component drives the weight curve (Y/Z = 0).

- [ ] **Step 3.1: Failing test**

Append to `mod tests`:

```rust
    #[test]
    fn expression_weight_emits_translation_channel_with_x_component() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();

        // Place a node at index 0 for the "happy" expression target.
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
        assert_eq!(channels.last().unwrap()["target"]["path"], "translation");
    }
```

- [ ] **Step 3.2: Verify fail**

Run: `cargo test -p vrm-asset-generator expression_weight_emits`
Expected: FAIL.

- [ ] **Step 3.3: Implement**

Append to `vrma_emit.rs`:

```rust
/// Whether an expression is one of the 14 spec-defined presets or a
/// caller-named custom expression.
#[derive(Debug, Clone, Copy)]
pub enum ExpressionKind<'a> {
    Preset(&'a str),
    Custom(&'a str),
}

/// Add an expression weight animation channel.
///
/// Per the VRMA spec, weights are the X-component of node.translation;
/// the runner / adapter pulls the X component out at apply_vrma_at_time.
/// Y and Z are always 0.
pub fn add_expression_weight_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    kind: ExpressionKind,
    keyframes: &[(f32, f32)],
) {
    // Promote scalar weights to (time, [weight, 0, 0]) for translation channel.
    let translation_keyframes: Vec<(f32, [f32; 3])> =
        keyframes.iter().map(|(t, w)| (*t, [*w, 0.0, 0.0])).collect();
    add_node_translation_channel(doc, buffer, node_index, &translation_keyframes);

    // Update VRMC_vrm_animation.expressions.{preset|custom}.<name>
    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    let expressions = ext
        .entry("expressions")
        .or_insert_with(|| json!({ "preset": {}, "custom": {} }));
    let (key, name) = match kind {
        ExpressionKind::Preset(n) => ("preset", n),
        ExpressionKind::Custom(n) => ("custom", n),
    };
    let category = expressions[key].as_object_mut().unwrap();
    category.insert(name.to_string(), json!({ "node": node_index }));
}

/// Helper: write a node translation channel (VEC3 values).
fn add_node_translation_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    keyframes: &[(f32, [f32; 3])],
) {
    if doc.get("accessors").is_none() {
        doc["accessors"] = json!([]);
    }
    if doc.get("bufferViews").is_none() {
        doc["bufferViews"] = json!([]);
    }
    if doc.get("buffers").is_none() {
        doc["buffers"] = json!([{ "byteLength": 0 }]);
    }

    let timestamps: Vec<f32> = keyframes.iter().map(|(t, _)| *t).collect();
    let timestamp_offset = buffer.len();
    for t in &timestamps {
        buffer.extend_from_slice(&t.to_le_bytes());
    }
    let timestamp_byte_length = buffer.len() - timestamp_offset;
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }

    let values_offset = buffer.len();
    for (_, v) in keyframes {
        for c in v {
            buffer.extend_from_slice(&c.to_le_bytes());
        }
    }
    let values_byte_length = buffer.len() - values_offset;
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }

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
```

- [ ] **Step 3.4: Verify**

Run: `cargo test -p vrm-asset-generator expression_weight`
Expected: PASS.

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 3.5: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_emit.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): expression weight channel emission

Per VRMA spec, expression weights are the X-component of node.translation
animation. add_expression_weight_channel promotes (time, weight) pairs
to (time, [weight, 0, 0]) and emits a translation channel + updates
VRMC_vrm_animation.expressions.{preset|custom}.<name>.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: LookAt gaze direction channel emission

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_emit.rs`

The VRMA lookAt block declares a single node whose rotation is interpreted as the gaze direction quaternion. Per the spec, the quaternion is converted to Extrinsic ZXY yaw/pitch when applied.

- [ ] **Step 4.1: Failing test**

```rust
    #[test]
    fn look_at_emits_node_rotation_channel() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();

        let nodes_arr = doc["nodes"].as_array_mut().unwrap();
        nodes_arr.push(json!({ "name": "look_at_target" }));

        add_look_at_channel(
            &mut doc,
            &mut buffer,
            0,
            [0.0, 0.06, 0.0],  // offsetFromHeadBone
            &[(0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]), (1.0, [0.0, 0.259, 0.0, 0.966])],
        );

        let look_at = &doc["extensions"]["VRMC_vrm_animation"]["lookAt"];
        assert_eq!(look_at["node"], 0);
        assert_eq!(look_at["offsetFromHeadBone"], json!([0.0, 0.06, 0.0]));
    }
```

- [ ] **Step 4.2: Verify fail**

Run: `cargo test -p vrm-asset-generator look_at_emits_node_rotation`
Expected: FAIL.

- [ ] **Step 4.3: Implement**

Append to `vrma_emit.rs`:

```rust
/// Add a lookAt gaze direction channel + offsetFromHeadBone declaration.
///
/// The gaze direction is encoded as the rotation of `node_index`. Per spec,
/// it is converted to Extrinsic ZXY yaw/pitch when applied.
pub fn add_look_at_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    offset_from_head_bone: [f32; 3],
    keyframes: &[(f32, [f32; 4])],
) {
    // Reuse the rotation-channel helper from the humanoid path. It also
    // updates humanBones, but we'll override the extension block below.
    let saved_humanoid = doc["extensions"]["VRMC_vrm_animation"]
        .get("humanoid")
        .cloned();

    add_humanoid_bone_rotation_channel(doc, buffer, node_index, "__look_at_placeholder", keyframes);

    // Undo the "humanBones" insert that the helper made for our placeholder.
    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    match saved_humanoid {
        Some(prior) => {
            ext.insert("humanoid".into(), prior);
        }
        None => {
            ext.remove("humanoid");
        }
    }

    // Set lookAt block.
    ext.insert(
        "lookAt".into(),
        json!({
            "node": node_index,
            "offsetFromHeadBone": offset_from_head_bone,
        }),
    );
}
```

Note: the implementation reuses `add_humanoid_bone_rotation_channel`'s machinery for the rotation channel (channels/samplers/accessors/bufferViews) but then restores the humanoid extension block since lookAt nodes aren't humanoid bones. If this composability proves too fragile, refactor to extract the channel-writing into its own helper (e.g. `add_node_rotation_channel` parallel to `add_node_translation_channel`).

- [ ] **Step 4.4: Verify**

Run: `cargo test -p vrm-asset-generator look_at`
Expected: PASS.

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4.5: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_emit.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): lookAt gaze direction channel emission

add_look_at_channel writes a node.rotation channel for gaze direction +
sets VRMC_vrm_animation.lookAt = { node, offsetFromHeadBone }. Reuses
the humanoid bone rotation channel-writing code path but does not
register the node in humanBones.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Parameter dictionaries (`vrma_params.rs`)

**Files:**
- Create: `crates/vrm-asset-generator/src/vrma_params.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`

Mirror the `MToonParams` / `SpringBoneParams` shape: a struct per sweep family carrying the swept-axis values, plus a `defaults(id)` constructor.

- [ ] **Step 5.1: Add module to lib.rs**

In `crates/vrm-asset-generator/src/lib.rs`, add `pub mod vrma_params;`.

- [ ] **Step 5.2: Create the module with tests**

Create `crates/vrm-asset-generator/src/vrma_params.rs`:

```rust
//! Parameter dictionaries for VRMA sweep emission. Each sweep variant
//! holds one of these; the variant id becomes the file stem.

use serde::{Deserialize, Serialize};

/// Single-bone rotation sweep. Rotates `bone_name` about the named axis
/// from 0° at t=0 to `angle_deg` at t=`duration_s`, linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaHumanoidParams {
    pub id: String,
    pub bone_name: String,
    pub axis: RotationAxis,
    pub angle_deg: f32,
    pub duration_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RotationAxis {
    X,
    Y,
    Z,
}

/// Single-expression weight ramp: 0 → 1 → 0 over `duration_s`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaExpressionParams {
    pub id: String,
    pub expression_name: String,
    pub is_preset: bool,
    pub duration_s: f32,
}

/// LookAt direction sweep: rotate gaze about yaw or pitch axis to the
/// given angle. Avatar config controls whether the .vrm renders gaze
/// via bone or aim/expression path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaLookAtParams {
    pub id: String,
    pub axis: RotationAxis,  // Y = yaw, X = pitch
    pub angle_deg: f32,
    pub avatar_lookat_type: AvatarLookAtType,
    pub duration_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AvatarLookAtType {
    /// Avatar `VRMC_vrm.lookAt.type: bone`.
    Bone,
    /// Avatar `VRMC_vrm.lookAt.type: expression`.
    Expression,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanoid_params_roundtrips() {
        let p = VrmaHumanoidParams {
            id: "vrma_humanoid_leftUpperArm_yaw_30".into(),
            bone_name: "leftUpperArm".into(),
            axis: RotationAxis::Y,
            angle_deg: 30.0,
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VrmaHumanoidParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.bone_name, "leftUpperArm");
    }
}
```

- [ ] **Step 5.3: Verify**

Run: `cargo test -p vrm-asset-generator vrma_params`
Expected: PASS.

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5.4: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_params.rs crates/vrm-asset-generator/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): VrmaHumanoidParams + VrmaExpressionParams + VrmaLookAtParams

Parameter dictionaries for the 3 sweep families. Each has a unique id
(used as file stem), the swept-axis values, and a duration. Mirrors the
MToonParams / SpringBoneParams pattern.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Humanoid sweep — `vrma_humanoid_sweep()` + `emit-vrma-humanoid-sweep` subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs`
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`
- Modify: `crates/vrm-asset-generator/src/cli.rs`

15 variants:
- `hips` translation Y (vertical bob, 0.1 m)
- `spine` rotation Y +30°
- `head` rotation Y +45°, X +20°, Z +15° (3 variants)
- `leftUpperArm` rotation Y/X/Z (3 variants)
- `rightUpperArm` rotation Y/X/Z (3 variants)
- `leftUpperLeg` rotation Y/X (2 variants)
- `leftLowerLeg` rotation X +60°

Each variant emits a paired triplet: a canonical .vrm (the existing `humanoid::Skeleton` rig), a .vrma with one bone animation, and a .test.yaml plan with `animation.vrma`.

- [ ] **Step 6.1: Add `vrma_humanoid_sweep()` to sweep.rs**

Append to `crates/vrm-asset-generator/src/sweep.rs`:

```rust
use crate::vrma_params::{RotationAxis, VrmaHumanoidParams};

pub fn vrma_humanoid_sweep() -> Vec<VrmaHumanoidParams> {
    let entries: Vec<(&str, &str, RotationAxis, f32)> = vec![
        ("vrma_humanoid_hips_y",          "hips",           RotationAxis::Y, 0.10), // hips: 10cm "translation" Y — emitted as rotation here for simplicity; refine later
        ("vrma_humanoid_spine_yaw_30",    "spine",          RotationAxis::Y, 30.0),
        ("vrma_humanoid_head_yaw_45",     "head",           RotationAxis::Y, 45.0),
        ("vrma_humanoid_head_pitch_20",   "head",           RotationAxis::X, 20.0),
        ("vrma_humanoid_head_roll_15",    "head",           RotationAxis::Z, 15.0),
        ("vrma_humanoid_l_upperarm_yaw",  "leftUpperArm",   RotationAxis::Y, 60.0),
        ("vrma_humanoid_l_upperarm_pit",  "leftUpperArm",   RotationAxis::X, 60.0),
        ("vrma_humanoid_l_upperarm_rol",  "leftUpperArm",   RotationAxis::Z, 30.0),
        ("vrma_humanoid_r_upperarm_yaw",  "rightUpperArm",  RotationAxis::Y, -60.0),
        ("vrma_humanoid_r_upperarm_pit",  "rightUpperArm",  RotationAxis::X, 60.0),
        ("vrma_humanoid_r_upperarm_rol",  "rightUpperArm",  RotationAxis::Z, 30.0),
        ("vrma_humanoid_l_upperleg_pit",  "leftUpperLeg",   RotationAxis::X, 45.0),
        ("vrma_humanoid_l_upperleg_yaw",  "leftUpperLeg",   RotationAxis::Y, 15.0),
        ("vrma_humanoid_l_lowerleg_x",    "leftLowerLeg",   RotationAxis::X, 60.0),
        ("vrma_humanoid_neck_yaw_30",     "neck",           RotationAxis::Y, 30.0),
    ];

    entries
        .into_iter()
        .map(|(id, bone, axis, angle)| VrmaHumanoidParams {
            id: id.into(),
            bone_name: bone.into(),
            axis,
            angle_deg: angle,
            duration_s: 1.0,
        })
        .collect()
}
```

Note: `hips` translation deserves a separate code path (since VRMA hips is the only bone that allows translation animation). For phase 3 v1, emit it as a hips-bone rotation around Y — coverage will follow up if needed.

- [ ] **Step 6.2: Add emit function**

In `crates/vrm-asset-generator/src/emit.rs`, append:

```rust
use crate::vrma_emit::add_humanoid_bone_rotation_channel;
use crate::vrma_params::{RotationAxis, VrmaHumanoidParams};

/// Emit a humanoid-sweep triplet to `output_dir`:
///   - `<id>.vrm`     canonical humanoid rig
///   - `<id>.vrma`    single-bone rotation animation
///   - `<id>.test.yaml`
pub fn emit_vrma_humanoid_triplet(
    output_dir: &std::path::Path,
    params: &VrmaHumanoidParams,
) -> std::io::Result<()> {
    // 1. .vrm — canonical humanoid rig (reuse the existing emit-default path).
    let vrm_default = crate::params::MToonParams::defaults(&params.id);
    let vrm_bytes = build_vrm_glb(&vrm_default);  // existing helper; adapt name to your generator's actual function
    std::fs::write(output_dir.join(format!("{}.vrm", params.id)), &vrm_bytes)?;

    // 2. .vrma — single-bone rotation channel against the canonical skeleton.
    let skel = crate::humanoid::build_skeleton();
    let node_idx = *skel
        .bone_to_node
        .get(&params.bone_name)
        .unwrap_or_else(|| panic!("bone {} not in canonical skeleton", params.bone_name));

    let mut doc = crate::vrma_emit::build_empty_vrma();
    // Pre-populate doc.nodes to match the skeleton so the human-bone
    // mapping references valid indices. (We don't need the full mesh,
    // just nodes matching the skeleton hierarchy.)
    doc["nodes"] = serde_json::json!(skel.nodes_json);

    let mut buffer = Vec::<u8>::new();
    let axis_angle_rad = params.angle_deg.to_radians();
    let half = axis_angle_rad / 2.0;
    let s = half.sin();
    let target_quat = match params.axis {
        RotationAxis::X => [s, 0.0, 0.0, half.cos()],
        RotationAxis::Y => [0.0, s, 0.0, half.cos()],
        RotationAxis::Z => [0.0, 0.0, s, half.cos()],
    };
    let keyframes = [(0.0_f32, [0.0, 0.0, 0.0, 1.0]), (params.duration_s, target_quat)];

    add_humanoid_bone_rotation_channel(&mut doc, &mut buffer, node_idx, &params.bone_name, &keyframes);

    let vrma_bytes = crate::vrma_emit::write_vrma_glb(&doc, &buffer);
    std::fs::write(output_dir.join(format!("{}.vrma", params.id)), &vrma_bytes)?;

    // 3. .test.yaml — paired plan.
    let plan_yaml = crate::sidecar::build_vrma_humanoid_test_plan(params);
    std::fs::write(output_dir.join(format!("{}.test.yaml", params.id)), &plan_yaml)?;

    Ok(())
}
```

Adapt `build_vrm_glb`, `build_skeleton`, and `skel.nodes_json` names to match the actual helpers in `humanoid.rs` / `emit.rs`. Read the existing `emit_default` function to copy its rig-construction path verbatim.

- [ ] **Step 6.3: Add sidecar plan builder**

In `crates/vrm-asset-generator/src/sidecar.rs`, append:

```rust
use crate::vrma_params::VrmaHumanoidParams;

pub fn build_vrma_humanoid_test_plan(params: &VrmaHumanoidParams) -> String {
    format!(
        r#"id: {id}
spec_section: VRMC_vrm_animation (humanoid bone sweep: {bone} {angle}°)
asset: {id}.vrm
animation:
  vrma: {id}.vrma
  apply_at_time: {sample_time}
camera:
  position: [0.0, 1.5, 1.2]
  target: [0.0, 1.0, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.2
  cast_shadows: false
  receive_shadows: false
post_processing:
  tone_mapping: none
  exposure: 1.0
output:
  width: 1024
  height: 1024
  color_space: srgb
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: univrm
  pose_tolerance:
    per_bone_quaternion_radians: 0.010
    hips_translation_m: 0.005
    per_preset_expression: 0.005
    per_custom_expression: 0.005
    look_at_yaw_pitch_degrees: 1.0
    offset_from_head_bone_m: 0.001
  conformance_status:
    kind: included
"#,
        id = params.id,
        bone = params.bone_name,
        angle = params.angle_deg,
        sample_time = params.duration_s,
    )
}
```

- [ ] **Step 6.4: Wire the CLI subcommand**

In `crates/vrm-asset-generator/src/cli.rs`, add a new variant to the `Commands` enum:

```rust
    /// Emit the VRMA humanoid bone sweep (~15 plans). One bone per plan,
    /// single-axis rotation arc over 1 s.
    EmitVrmaHumanoidSweep {
        #[arg(long)]
        output_dir: std::path::PathBuf,
        #[arg(long)]
        json: bool,
    },
```

And add a handler for it in the match block alongside `EmitSweep`:

```rust
        Commands::EmitVrmaHumanoidSweep { output_dir, json } => {
            std::fs::create_dir_all(&output_dir)?;
            let sweep = sweep::vrma_humanoid_sweep();
            for (i, params) in sweep.iter().enumerate() {
                emit::emit_vrma_humanoid_triplet(&output_dir, params)?;
                if json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-humanoid-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        total = sweep.len(),
                        id = params.id,
                    );
                }
            }
            println!("emitted {} VRMA humanoid sweep plans to {}", sweep.len(), output_dir.display());
            Ok(())
        }
```

Also add a describe-catalog entry near the other emit-* catalog entries (find `"emit-springbone-sweep"` in cli.rs and add `"emit-vrma-humanoid-sweep"` nearby with summary + arg shape).

- [ ] **Step 6.5: End-to-end smoke**

```bash
cargo run -p vrm-asset-generator -- emit-vrma-humanoid-sweep --output-dir /tmp/vrma-humanoid-sweep
ls /tmp/vrma-humanoid-sweep | head -10
file /tmp/vrma-humanoid-sweep/vrma_humanoid_head_yaw_45.vrma
```

Expected: 45 files (15 × 3); the .vrma files are reported as `glTF binary 2.0`.

Run: `cargo test -p vrm-asset-generator`
Expected: all tests pass.

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6.6: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): vrma_humanoid_sweep + emit-vrma-humanoid-sweep

15 single-bone rotation variants targeting hips, spine, head (3 axes),
leftUpperArm (3 axes), rightUpperArm (3 axes), leftUpperLeg (2 axes),
leftLowerLeg, neck. Each emits paired triplet (.vrm + .vrma +
.test.yaml). Reuses the canonical humanoid::Skeleton.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Expression sweep — `vrma_expression_sweep()` + `emit-vrma-expression-sweep`

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs`
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`
- Modify: `crates/vrm-asset-generator/src/cli.rs`

12 variants: 10 preset expressions (skipping the 4 lookUp/lookDown/lookLeft/lookRight excluded per spec, plus the 14 - 10 = 4 most visually informative) + 2 custom blendshape variants. Each animates a single expression from 0 → 1 → 0 over 1 s with the peak at t=0.5 s.

- [ ] **Step 7.1: Add `vrma_expression_sweep()` to sweep.rs**

```rust
use crate::vrma_params::VrmaExpressionParams;

pub fn vrma_expression_sweep() -> Vec<VrmaExpressionParams> {
    let presets = [
        "happy", "angry", "sad", "relaxed", "surprised",
        "aa", "ih", "ou", "ee", "blink",
    ];
    let custom = ["smug", "drowsy"];

    presets
        .iter()
        .map(|n| VrmaExpressionParams {
            id: format!("vrma_expression_preset_{n}"),
            expression_name: (*n).into(),
            is_preset: true,
            duration_s: 1.0,
        })
        .chain(custom.iter().map(|n| VrmaExpressionParams {
            id: format!("vrma_expression_custom_{n}"),
            expression_name: (*n).into(),
            is_preset: false,
            duration_s: 1.0,
        }))
        .collect()
}
```

- [ ] **Step 7.2: emit + sidecar**

In `emit.rs`:

```rust
use crate::vrma_emit::{add_expression_weight_channel, ExpressionKind};
use crate::vrma_params::VrmaExpressionParams;

pub fn emit_vrma_expression_triplet(
    output_dir: &std::path::Path,
    params: &VrmaExpressionParams,
) -> std::io::Result<()> {
    // Reuse the canonical humanoid rig for the .vrm side (it's an inert
    // host avatar — the .vrma carries the test signal).
    let vrm_default = crate::params::MToonParams::defaults(&params.id);
    let vrm_bytes = build_vrm_glb(&vrm_default);
    std::fs::write(output_dir.join(format!("{}.vrm", params.id)), &vrm_bytes)?;

    // .vrma: one node for the expression target + a weight ramp 0→1→0.
    let mut doc = crate::vrma_emit::build_empty_vrma();
    let nodes = doc["nodes"].as_array_mut().unwrap();
    nodes.push(serde_json::json!({ "name": format!("{}_expr_target", params.expression_name) }));
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

    let vrma_bytes = crate::vrma_emit::write_vrma_glb(&doc, &buffer);
    std::fs::write(output_dir.join(format!("{}.vrma", params.id)), &vrma_bytes)?;

    let plan = crate::sidecar::build_vrma_expression_test_plan(params);
    std::fs::write(output_dir.join(format!("{}.test.yaml", params.id)), &plan)?;

    Ok(())
}
```

In `sidecar.rs`:

```rust
use crate::vrma_params::VrmaExpressionParams;

pub fn build_vrma_expression_test_plan(params: &VrmaExpressionParams) -> String {
    let kind_label = if params.is_preset { "preset" } else { "custom" };
    format!(
        r#"id: {id}
spec_section: VRMC_vrm_animation (expression sweep: {kind_label} {name})
asset: {id}.vrm
animation:
  vrma: {id}.vrma
  apply_at_time: {sample_time}
camera:
  position: [0.0, 1.5, 1.2]
  target: [0.0, 1.5, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.2
  cast_shadows: false
  receive_shadows: false
post_processing:
  tone_mapping: none
  exposure: 1.0
output:
  width: 1024
  height: 1024
  color_space: srgb
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: univrm
  pose_tolerance:
    per_bone_quaternion_radians: 0.010
    hips_translation_m: 0.005
    per_preset_expression: 0.005
    per_custom_expression: 0.005
    look_at_yaw_pitch_degrees: 1.0
    offset_from_head_bone_m: 0.001
  conformance_status:
    kind: included
"#,
        id = params.id,
        kind_label = kind_label,
        name = params.expression_name,
        sample_time = params.duration_s / 2.0,  // sample at peak weight
    )
}
```

- [ ] **Step 7.3: Wire CLI**

Add `EmitVrmaExpressionSweep` to `Commands` with the same shape as `EmitVrmaHumanoidSweep`. Add the describe-catalog entry.

- [ ] **Step 7.4: Smoke + verify**

```bash
cargo run -p vrm-asset-generator -- emit-vrma-expression-sweep --output-dir /tmp/vrma-expr-sweep
ls /tmp/vrma-expr-sweep | wc -l   # expect 36 (12 × 3)
```

Run: `cargo test -p vrm-asset-generator`, `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings` — both clean.

- [ ] **Step 7.5: Commit**

```bash
git add crates/vrm-asset-generator/
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): vrma_expression_sweep + emit-vrma-expression-sweep

12 variants: 10 preset expressions (happy, angry, sad, relaxed,
surprised, aa, ih, ou, ee, blink — the 4 lookUp/Down/Left/Right
excluded per spec) + 2 custom blendshape names. Each animates a single
expression 0 → 1 → 0 over 1 s; test plans sample at peak (t=0.5).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: LookAt sweep — `vrma_lookat_sweep()` + `emit-vrma-lookat-sweep` + dual avatar configs

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs`
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`
- Modify: `crates/vrm-asset-generator/src/cli.rs`
- Modify: `crates/vrm-asset-generator/src/vrm_ext.rs`

The lookAt sweep tests the same .vrma against two avatar configurations: avatars with `VRMC_vrm.lookAt.type: bone` and `: expression` (the spec's two application modes — there's also `none` but it's degenerate). 5 directions × 2 avatar configs = 10 plans.

- [ ] **Step 8.1: Avatar-side lookAt.type emission**

In `crates/vrm-asset-generator/src/vrm_ext.rs`, find the function that emits the `VRMC_vrm.lookAt` block (search for `"lookAt"`). Modify it to accept an enum parameter:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AvatarLookAtType {
    Bone,
    Expression,
}

// Modify the VRMC_vrm builder to take an AvatarLookAtType parameter and
// emit `"type": "bone"` or `"type": "expression"` accordingly. Default
// callers that don't specify it use Bone.
```

Where the existing VRMC_vrm builder writes the lookAt block, set `"type"` based on the new parameter.

- [ ] **Step 8.2: Add `vrma_lookat_sweep()` to sweep.rs**

```rust
use crate::vrma_params::{AvatarLookAtType, RotationAxis, VrmaLookAtParams};

pub fn vrma_lookat_sweep() -> Vec<VrmaLookAtParams> {
    // 5 directions × 2 avatar configs = 10 variants.
    let directions: [(&str, RotationAxis, f32); 5] = [
        ("yaw_neg60", RotationAxis::Y, -60.0),
        ("yaw_pos60", RotationAxis::Y, 60.0),
        ("pitch_neg30", RotationAxis::X, -30.0),
        ("pitch_pos30", RotationAxis::X, 30.0),
        ("neutral", RotationAxis::Y, 0.0),
    ];
    let configs = [AvatarLookAtType::Bone, AvatarLookAtType::Expression];

    let mut out = Vec::new();
    for (dir_name, axis, angle) in &directions {
        for config in &configs {
            let config_str = match config {
                AvatarLookAtType::Bone => "bone",
                AvatarLookAtType::Expression => "expr",
            };
            out.push(VrmaLookAtParams {
                id: format!("vrma_lookat_{dir_name}_{config_str}"),
                axis: *axis,
                angle_deg: *angle,
                avatar_lookat_type: *config,
                duration_s: 1.0,
            });
        }
    }
    out
}
```

- [ ] **Step 8.3: emit + sidecar**

Emit a .vrm whose `VRMC_vrm.lookAt.type` matches the params. The .vrma encodes the gaze direction quaternion via lookAt node rotation. Plan YAML follows the existing pattern.

- [ ] **Step 8.4: Wire CLI**

Add `EmitVrmaLookatSweep` to `Commands` enum + describe catalog.

- [ ] **Step 8.5: Smoke + verify + commit**

```bash
cargo run -p vrm-asset-generator -- emit-vrma-lookat-sweep --output-dir /tmp/vrma-lookat-sweep
ls /tmp/vrma-lookat-sweep | wc -l   # expect 30 (10 × 3)
```

Commit:

```bash
git add crates/vrm-asset-generator/
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): vrma_lookat_sweep + emit-vrma-lookat-sweep

10 variants: 5 gaze directions (yaw ±60°, pitch ±30°, neutral) × 2
avatar configs (VRMC_vrm.lookAt.type: bone vs expression). Same .vrma
gaze direction tested against both rendering paths surfaces the
avatar-config-vs-vrma-encoding split per methodology hazard #5.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Bootstrap integration

**Files:**
- Modify: `scripts/bootstrap-goldens.sh`

Wire the 3 new emit subcommands into `scripts/bootstrap-goldens.sh` so the next bootstrap picks up the VRMA sweep families. Each writes to its own `_assets_vrma_humanoid`, `_assets_vrma_expression`, `_assets_vrma_lookat` subdirectory.

- [ ] **Step 9.1: Find the existing emit-springbone-*-sweep block**

```bash
grep -n "emit-springbone" scripts/bootstrap-goldens.sh
```

- [ ] **Step 9.2: Add 3 new emit blocks**

After the last `emit-springbone-*-sweep` block (likely emit-springbone-multichain-sweep around line 128), insert:

```bash
    echo "==> Emitting VRMA humanoid sweep (phase 3: 15 plans)"
    cargo run --release -q -p vrm-asset-generator -- emit-vrma-humanoid-sweep \
        --output-dir "$GOLDENS_DIR/_assets_vrma_humanoid" \
        --json 2>&1 | tee -a "$GOLDENS_DIR/_assets_vrma_humanoid.log"

    echo "==> Emitting VRMA expression sweep (phase 3: 12 plans)"
    cargo run --release -q -p vrm-asset-generator -- emit-vrma-expression-sweep \
        --output-dir "$GOLDENS_DIR/_assets_vrma_expression" \
        --json 2>&1 | tee -a "$GOLDENS_DIR/_assets_vrma_expression.log"

    echo "==> Emitting VRMA lookAt sweep (phase 3: 10 plans)"
    cargo run --release -q -p vrm-asset-generator -- emit-vrma-lookat-sweep \
        --output-dir "$GOLDENS_DIR/_assets_vrma_lookat" \
        --json 2>&1 | tee -a "$GOLDENS_DIR/_assets_vrma_lookat.log"
```

The `consensus-report.sh` walk-all-`_assets*` logic landed during the spring-bone closure already auto-picks up new subdirs, so no script change there.

- [ ] **Step 9.3: Smoke**

```bash
SKIP_THREE_VRM=1 SKIP_GODOT_VRM=1 SKIP_VRM_METAL_KIT=1 scripts/bootstrap-goldens.sh 2>&1 | tail -30
```

Expected: the 3 new emit lines fire; ~37 new plans appear under `goldens-cache/_assets_vrma_*/`. With all real adapters skipped, no rendering — just the asset-generation pass.

- [ ] **Step 9.4: Commit**

```bash
git add scripts/bootstrap-goldens.sh
git commit -m "$(cat <<'EOF'
feat(scripts/bootstrap-goldens): wire VRMA emit subcommands

emit-vrma-humanoid-sweep + emit-vrma-expression-sweep +
emit-vrma-lookat-sweep populate _assets_vrma_humanoid /
_assets_vrma_expression / _assets_vrma_lookat. consensus-report.sh
already walks any _assets* subdir so it auto-picks up the new
families on the next bootstrap.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Workspace fmt + clippy + test pass

**Files:** none directly.

- [ ] **Step 10.1: fmt + clippy + test**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 10.2: Commit if anything changed**

```bash
git status -s
```

If modifications:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt + clippy clean-up after VRMA phase 3

Final workspace pass after VRMA phase 3 (vrma_emit module + 3 sweep
subcommands + bootstrap integration). Zero clippy warnings, zero fmt
diffs.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If clean, no commit.

---

## Phase 3 completion checklist

- [ ] `vrma_emit.rs` provides `build_empty_vrma`, `write_vrma_glb`, `add_humanoid_bone_rotation_channel`, `add_expression_weight_channel`, `add_look_at_channel` with 4 unit tests
- [ ] `vrma_params.rs` provides `VrmaHumanoidParams`, `VrmaExpressionParams`, `VrmaLookAtParams` + supporting enums
- [ ] `sweep.rs` provides `vrma_humanoid_sweep` (15), `vrma_expression_sweep` (12), `vrma_lookat_sweep` (10) returning Vec of param structs
- [ ] `emit.rs` provides triplet-writer functions for each sweep
- [ ] `sidecar.rs` provides plan-YAML builders for each sweep
- [ ] `cli.rs` exposes `emit-vrma-humanoid-sweep`, `emit-vrma-expression-sweep`, `emit-vrma-lookat-sweep` subcommands + describe catalog entries
- [ ] `bootstrap-goldens.sh` runs the 3 new emit subcommands
- [ ] All test plans use the canonical humanoid::Skeleton on the .vrm side
- [ ] LookAt sweep emits 2 avatar configs per gaze direction (bone vs expression)
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all clean
- [ ] End-to-end smoke run produces ~37 new test_ids in the corpus

After this phase, the corpus has ~37 new VRMA test plans ready to render. Phase 4 wires the UniVRM adapter; Phase 5 wires three-vrm. Phase 6 lands manual humanoid clips, bootstrap, findings, and upstream issues.
