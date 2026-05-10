# Phase 2D-a — Spring-Bone Asset Emission

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the asset generator to emit VRM 1.0 assets carrying a valid `VRMC_springBone` extension. Single scenario per emission, validator-clean, with parametric joint chains attached to the humanoid stub's head. Establishes the asset-side foundation for the broader spring-bone test category; renderer-side support and the full 8-scenario sweep land in Phase 2D-b/c.

**Architecture:** Mirror the MToon pattern: a `SpringBoneParams` value drives chain emission, VRMC_springBone JSON construction, and the sidecar `.test.yaml` simultaneously. The humanoid stub's `head` bone gains a configurable number of trailing leaf nodes (the spring chain — think of it as a single hair strand). The emitter weaves those new nodes into `nodes`/`scenes` while VRMC_springBone references them by node index in its `joints` array.

**Tech Stack:** Rust 2021 (existing workspace). No new crates. mrxz/vrm-validator binary is the acceptance oracle (called from the existing C2/C3 test infrastructure).

**Why scope-bound:**
- 2C-b just shipped real three-vrm rendering against MToon assets. The next bottleneck is corpus diversity: today every asset is a static sphere. Spring bones are the first axis adding *dynamic* behavior.
- 2D-a only emits the assets — no renderer-side `step_physics` / `reset_physics` / `animate_root_transform` yet. Three-vrm + the mock still return Unimplemented for those reserved ops. That's intentional: this plan keeps the renderer surface frozen so the asset-side change can land cleanly.
- 2D-b will add the renderer-side ops and at least one end-to-end spring-bone test.
- 2D-c (or later) expands from one chain to the full handover §5.1 matrix (8 scenarios: single chain / sphere collider / capsule collider / multi-chain shared collider / stiffness / drag / gravity / hit radius).

**YAGNI scope guards:**
- ✅ No colliders. `colliders` and `colliderGroups` arrays emit empty. The 4 collider scenarios from the spec are 2D-c.
- ✅ Single chain per asset. Multi-chain is 2D-c.
- ✅ No physics simulation in the asset generator — we emit the spec data; the renderer simulates.
- ✅ No test-plan `physics:` block in this plan. Spring-bone-aware test plans (with `animate_root_transform` excitation + `step_physics` settle steps) land in 2D-b once renderers actually honor those ops.
- ✅ Existing `mtoon_basic_sweep` corpus untouched — spring-bone assets are emitted via a separate CLI subcommand.

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `crates/vrm-asset-generator/src/spring_bone.rs` | Create | `SpringBoneParams` parameter dictionary + `springbone_default()` baseline. |
| `crates/vrm-asset-generator/src/humanoid.rs` | Modify | Add `append_spring_chain(skeleton, parent_bone, length, segment_length) -> Vec<usize>` returning the appended node indices. The skeleton structure stays backward-compatible. |
| `crates/vrm-asset-generator/src/vrm_ext.rs` | Modify | Add `vrmc_spring_bone(name, joint_nodes, params) -> Value`. Emit `extensionsUsed` to include `"VRMC_springBone"` when appropriate. |
| `crates/vrm-asset-generator/src/emit.rs` | Modify | Replace the implicit "no spring bones" path with `emit_vrm_with_spring_bone(mtoon, spring_bone, output)`. Existing `emit_vrm` keeps working for the MToon-only path. |
| `crates/vrm-asset-generator/src/sidecar.rs` | Modify | When spring-bone params are present, the emitted `.meta.json` includes them under `spring_bone:`; the `.test.yaml` is unchanged from the MToon plan default. |
| `crates/vrm-asset-generator/src/lib.rs` | Modify | `pub mod spring_bone;`. |
| `crates/vrm-asset-generator/src/cli.rs` | Modify | Add `emit-springbone` subcommand: takes `--id`, `--output-dir`, `--json`. Update `describe` catalog. |
| `crates/vrm-asset-generator/tests/spring_bone.rs` | Create | Unit tests on the spring-bone params + humanoid chain extension. |
| `crates/vrm-asset-generator/tests/spring_bone_emit.rs` | Create | Integration test: emit a spring-bone asset, run validator, assert `num_errors == 0`. |

---

## Section A — Parameter dictionary + humanoid chain extension

### Task A1: `SpringBoneParams` type

**Files:**
- Create: `crates/vrm-asset-generator/src/spring_bone.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs`

A small parameter struct mirroring the VRMC_springBone joint schema. Single-chain scope for v0.1: one named spring, N joints, uniform per-joint params. Adjustable via the CLI later in 2D-c.

- [ ] **Step 1: Implement**

`crates/vrm-asset-generator/src/spring_bone.rs`:

```rust
//! Parameter dictionary for VRMC_springBone scenario generation.
//!
//! v0.1 emits a single named spring with N uniform joints. Per-joint
//! variation (the stiffness / drag / gravity sweeps from handover §5.1)
//! is supported by emitting separate assets each with different uniform
//! values — collider variants and multi-chain assets are deferred to 2D-c.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringBoneParams {
    pub id: String,

    /// Name attached to the VRMC_springBone "spring" object.
    pub spring_name: String,

    /// Number of joints in the chain (≥ 1). Joint 0 is the anchor
    /// attached to the parent bone; subsequent joints trail off.
    pub joint_count: u32,

    /// Length of each segment in meters. Total chain length = `joint_count * segment_length_m`.
    pub segment_length_m: f32,

    /// Per-joint stiffness in [0.0, 1.0]. 0 = no restoration; 1 = rigid.
    pub stiffness: f32,

    /// Per-joint drag force in [0.0, 1.0]. 0 = no damping; 1 = critically damped.
    pub drag_force: f32,

    /// Gravity strength (typical: 0.0 for hair, ~1.0 for ribbons).
    pub gravity_power: f32,

    /// Direction of gravity in world space.
    pub gravity_dir: [f32; 3],

    /// Collision radius for the joint in meters. v0.1 has no colliders, so
    /// this is metadata only; it still travels into the emitted JSON because
    /// renderers may use it for self-collision in the future.
    pub hit_radius: f32,
}

impl SpringBoneParams {
    /// Conservative defaults: 4 joints, 5 cm each, moderate stiffness and
    /// drag, gentle gravity. Reasonable for a single hair strand attached
    /// to the head.
    pub fn defaults(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            spring_name: format!("{id}_chain"),
            joint_count: 4,
            segment_length_m: 0.05,
            stiffness: 0.5,
            drag_force: 0.5,
            gravity_power: 0.5,
            gravity_dir: [0.0, -1.0, 0.0],
            hit_radius: 0.02,
        }
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

In `crates/vrm-asset-generator/src/lib.rs`, add `pub mod spring_bone;` alongside the existing module declarations (alphabetical order).

- [ ] **Step 3: Verify it compiles**

```bash
cargo build -p vrm-asset-generator
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-asset-generator/src/spring_bone.rs crates/vrm-asset-generator/src/lib.rs
git commit -m "feat(asset-generator): SpringBoneParams parameter dictionary"
```

---

### Task A2: Humanoid `append_spring_chain` helper (TDD)

**Files:**
- Modify: `crates/vrm-asset-generator/src/humanoid.rs`
- Create: `crates/vrm-asset-generator/tests/spring_bone.rs`

Adds a function that, given a `Skeleton` and a parent bone name, appends N child nodes to that bone's children. Returns the indices of the new nodes (the chain's joint list).

- [ ] **Step 1: Failing test**

`crates/vrm-asset-generator/tests/spring_bone.rs`:

```rust
use vrm_asset_generator::humanoid::{append_spring_chain, minimal_skeleton};

#[test]
fn chain_appends_to_parent_bone() {
    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];

    let chain = append_spring_chain(&mut skeleton, head_node, 4, 0.05);

    // Returned chain has 4 fresh node indices, all greater than the
    // original max node index in the skeleton.
    assert_eq!(chain.len(), 4);
    let original_node_count = 19; // the 19 humanoid bones
    for &idx in &chain {
        assert!(idx >= original_node_count, "chain node {idx} not appended");
    }
    let unique: std::collections::HashSet<_> = chain.iter().collect();
    assert_eq!(unique.len(), 4, "chain node indices must be unique");

    // The head node's children list now includes the FIRST chain node.
    let nodes = skeleton.nodes_json.as_array().unwrap();
    let head_children = nodes[head_node]["children"]
        .as_array()
        .expect("head node should have children array");
    let head_children_indices: Vec<u64> =
        head_children.iter().filter_map(|v| v.as_u64()).collect();
    assert!(
        head_children_indices.contains(&(chain[0] as u64)),
        "head's children must include the first chain node (got {head_children_indices:?})",
    );

    // Each chain node (except the last) lists the next chain node as its child.
    for window in chain.windows(2) {
        let (this_idx, next_idx) = (window[0], window[1]);
        let this_node = &nodes[this_idx];
        let children = this_node["children"]
            .as_array()
            .unwrap_or_else(|| panic!("chain node {this_idx} missing children"));
        assert!(
            children.iter().any(|v| v.as_u64() == Some(next_idx as u64)),
            "chain node {this_idx} must list {next_idx} as child",
        );
    }

    // Last chain node has no children array (or empty).
    let last_idx = *chain.last().unwrap();
    let last_node = &nodes[last_idx];
    let last_children = last_node.get("children");
    if let Some(c) = last_children {
        let arr = c.as_array().unwrap();
        assert!(arr.is_empty(), "last chain node must have no children");
    }

    // Each chain node carries a translation matching segment_length_m
    // along Y (or whatever the chain's spec says).
    let first_chain_node = &nodes[chain[0]];
    let t = first_chain_node["translation"].as_array().unwrap();
    let dy = t[1].as_f64().unwrap();
    // Default chain hangs straight down from head; segment 0.05 m.
    assert!(
        (dy - (-0.05)).abs() < 1e-6,
        "first chain joint translation Y should be -0.05, got {dy}",
    );
}

#[test]
fn chain_length_zero_is_a_no_op() {
    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let before = skeleton.nodes_json.as_array().unwrap().len();

    let chain = append_spring_chain(&mut skeleton, head_node, 0, 0.05);

    let after = skeleton.nodes_json.as_array().unwrap().len();
    assert_eq!(chain.len(), 0);
    assert_eq!(after, before, "zero-length chain should not add nodes");
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p vrm-asset-generator --test spring_bone`

Expected: compile error — `append_spring_chain` doesn't exist.

- [ ] **Step 3: Implement `append_spring_chain`**

In `crates/vrm-asset-generator/src/humanoid.rs`, append:

```rust
use serde_json::{json, Value};

/// Append N child nodes to the parent at `parent_node_index`, forming a
/// linear chain. Each chain node carries `translation = (0, -segment_length_m, 0)`
/// relative to its parent (the chain hangs straight down in rest pose).
/// Returns the new chain node indices in head-to-tail order.
///
/// This mutates `skeleton.nodes_json` in place; the new nodes are appended
/// to the end of the nodes array, and the parent's `children` array gains
/// the index of the first chain node.
pub fn append_spring_chain(
    skeleton: &mut Skeleton,
    parent_node_index: usize,
    joint_count: u32,
    segment_length_m: f32,
) -> Vec<usize> {
    if joint_count == 0 {
        return Vec::new();
    }

    let nodes = skeleton
        .nodes_json
        .as_array_mut()
        .expect("skeleton nodes_json must be an array");
    let mut chain_indices = Vec::with_capacity(joint_count as usize);

    let segment_translation = json!([0.0, -segment_length_m, 0.0]);

    // Reserve indices and emit each chain node. The parent of the first
    // chain node is parent_node_index; the parent of each subsequent chain
    // node is the previous chain node.
    for i in 0..joint_count {
        let my_idx = nodes.len();
        let mut node = json!({
            "name": format!("spring_joint_{i}"),
            "translation": segment_translation.clone(),
        });
        // All but the last chain joint will be assigned a `children`
        // entry pointing at the next joint, set in the loop below after
        // we know `my_idx + 1`.
        if i + 1 < joint_count {
            node["children"] = json!([my_idx + 1]);
        }
        nodes.push(node);
        chain_indices.push(my_idx);
    }

    // Wire the first chain node into the parent's children array.
    let parent = nodes
        .get_mut(parent_node_index)
        .expect("parent_node_index out of range");
    let mut parent_children = parent
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    parent_children.push(json!(chain_indices[0]));
    parent["children"] = Value::Array(parent_children);

    chain_indices
}
```

- [ ] **Step 4: Tests pass**

Run: `cargo test -p vrm-asset-generator --test spring_bone`

Expected: 2 tests pass.

- [ ] **Step 5: Workspace clean**

```bash
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo fmt --all -- --check
```

Both clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/humanoid.rs crates/vrm-asset-generator/tests/spring_bone.rs
git commit -m "feat(asset-generator): append_spring_chain extends humanoid stub with chain leaves"
```

---

## Section B — VRMC_springBone JSON emission

### Task B1: `vrmc_spring_bone` JSON fragment

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrm_ext.rs`

Builds the JSON for one VRMC_springBone extension entry given a joint-node list and the params. Single-spring shape, empty colliders/colliderGroups.

- [ ] **Step 1: Implement**

Append to `crates/vrm-asset-generator/src/vrm_ext.rs`:

```rust
use crate::spring_bone::SpringBoneParams;

/// Build a VRMC_springBone extension JSON object given the joint node
/// indices (in chain order, head-to-tail) and the per-spring params.
///
/// Spec reference: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone-1.0
///
/// v0.1 emits one named spring with no colliders. Multi-chain and
/// collider scenarios are out of scope for 2D-a.
pub fn vrmc_spring_bone(joint_nodes: &[usize], params: &SpringBoneParams) -> Value {
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

    json!({
        "specVersion": "1.0",
        "colliders": [],
        "colliderGroups": [],
        "springs": [{
            "name": params.spring_name,
            "joints": joints,
            "colliderGroups": [],
        }],
    })
}
```

- [ ] **Step 2: Verify compiles**

```bash
cargo build -p vrm-asset-generator
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-asset-generator/src/vrm_ext.rs
git commit -m "feat(asset-generator): vrmc_spring_bone JSON fragment builder"
```

---

### Task B2: `emit_vrm_with_spring_bone` top-level emitter

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs`

A new public function combines the existing MToon emission path with the spring-bone chain extension. The existing `emit_vrm` continues to work for MToon-only assets (no behavioral change).

- [ ] **Step 1: Implement**

In `crates/vrm-asset-generator/src/emit.rs`, append:

```rust
use crate::spring_bone::SpringBoneParams;
use crate::vrm_ext::vrmc_spring_bone;

/// Emit a `.vrm` GLB containing both MToon material data and a single
/// VRMC_springBone chain. The chain is attached to the head bone.
pub fn emit_vrm_with_spring_bone(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    output: &Utf8Path,
) -> Result<()> {
    // 1) Mesh + buffer (identical to emit_vrm).
    let mesh = sphere(0.3, 24, 48);
    let packed = pack_mesh(&mesh);

    // 2) Humanoid skeleton + spring chain off the head.
    let mut skeleton = minimal_skeleton();
    let head_node = skeleton.bone_to_node["head"];
    let chain_nodes = crate::humanoid::append_spring_chain(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
    );

    let mut nodes: Vec<Value> = skeleton.nodes_json.as_array().unwrap().clone();

    // 3) Add the mesh-bearing node parented to head.
    let mesh_node_index = nodes.len();
    nodes.push(json!({
        "name": format!("{}_mesh", mtoon.id),
        "mesh": 0
    }));
    let head = &mut nodes[head_node];
    let mut head_children = head["children"].as_array().cloned().unwrap_or_default();
    head_children.push(json!(mesh_node_index));
    head["children"] = Value::Array(head_children);

    // 4) Build glTF JSON with both extensions.
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
```

- [ ] **Step 2: Verify compiles**

```bash
cargo build -p vrm-asset-generator
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(asset-generator): emit_vrm_with_spring_bone combines MToon + VRMC_springBone"
```

---

## Section C — Validator-clean integration test

### Task C1: End-to-end emit + validate

**Files:**
- Create: `crates/vrm-asset-generator/tests/spring_bone_emit.rs`

Mirror of the existing F2d emit test, but with spring bones. Asserts `report.issues.num_errors == 0` against the real `mrxz/vrm-validator` shim.

- [ ] **Step 1: Implement**

`crates/vrm-asset-generator/tests/spring_bone_emit.rs`:

```rust
use camino::Utf8PathBuf;
use vrm_asset_generator::{
    emit::emit_vrm_with_spring_bone,
    params::MToonParams,
    spring_bone::SpringBoneParams,
};
use vrm_validator_wrap::{validate, ValidatorConfig};

fn config_or_skip() -> Option<ValidatorConfig> {
    match ValidatorConfig::from_env() {
        Ok(c) => Some(c),
        Err(_) => {
            eprintln!("SKIP: validator not installed");
            None
        }
    }
}

#[test]
fn emits_validator_clean_vrm_with_default_spring_bone() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("default_spring.vrm")).unwrap();

    let mtoon = MToonParams::defaults("default_spring");
    let spring = SpringBoneParams::defaults("default_spring");
    emit_vrm_with_spring_bone(&mtoon, &spring, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "spring-bone-bearing VRM should have zero validator errors. report: {:#?}",
        report.issues.messages
    );
    assert_eq!(report.mime_type.as_deref(), Some("model/gltf-binary"));
}

#[test]
fn emits_validator_clean_vrm_with_stiff_spring_bone() {
    let Some(cfg) = config_or_skip() else { return };

    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("stiff_spring.vrm")).unwrap();

    let mtoon = MToonParams::defaults("stiff_spring");
    let mut spring = SpringBoneParams::defaults("stiff_spring");
    spring.stiffness = 1.0;
    spring.drag_force = 0.9;
    spring.gravity_power = 0.0;
    spring.joint_count = 6;

    emit_vrm_with_spring_bone(&mtoon, &spring, &out).expect("emission must succeed");

    let report = validate(&cfg, &out).expect("validator must produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "stiff-variant VRM should validate clean. report: {:#?}",
        report.issues.messages
    );
}
```

- [ ] **Step 2: Run the test (validator must be installed)**

```bash
VRM_VALIDATOR_BIN=$(pwd)/.tools/vrm-validator-cli cargo test -p vrm-asset-generator --test spring_bone_emit -- --nocapture
```

Expected: both tests pass with `num_errors == 0`.

> **Caveat for the implementing engineer:** the validator may flag specific VRMC_springBone JSON shape issues. Read the validator messages and iterate on `vrmc_spring_bone` in `vrm_ext.rs`. Likely areas for fixes if any:
> - Missing fields: the spec may require `center: null` (an explicit null) on each spring; some implementations require it present.
> - Joint node indices must be valid into the `nodes` array — verify they're appended consistently in `append_spring_chain` and that `emit_vrm_with_spring_bone` uses the post-append node count.
> - Whether colliders/colliderGroups can be empty arrays vs. omitted — the spec allows both; some validator versions are stricter.
>
> If a validator error appears that isn't a JSON shape issue (e.g. references a missing extension dependency), STOP and escalate.

- [ ] **Step 3: Workspace clean**

```bash
cargo test -p vrm-asset-generator
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-asset-generator/tests/spring_bone_emit.rs
git commit -m "test(asset-generator): spring-bone emission validates clean against mrxz/vrm-validator"
```

---

## Section D — CLI subcommand

### Task D1: `emit-springbone` subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs`
- Modify: `crates/vrm-asset-generator/src/sidecar.rs` (optional: include spring-bone params in `.meta.json`)

The user-facing entry point: `vrm-asset-generator emit-springbone --id <id> --output-dir <dir> [--json]`. Emits `<id>.vrm`, `<id>.meta.json`, `<id>.test.yaml` using default MToon + default spring bone. Per-axis sweep variants are out of scope; this just proves the CLI surface for spring-bone-bearing assets.

- [ ] **Step 1: Update sidecar emission to include spring-bone params**

In `crates/vrm-asset-generator/src/sidecar.rs`, find `write_meta_json` and modify it to optionally accept spring-bone params:

```rust
pub fn write_meta_json(
    params: &MToonParams,
    spring_bone: Option<&crate::spring_bone::SpringBoneParams>,
    vrm_path: &Utf8Path,
    out: &Utf8Path,
) -> Result<()> {
    let bytes = std::fs::read(vrm_path)?;
    let hash = blake3::hash(&bytes);
    let mut meta = json!({
        "id": params.id,
        "license": "CC0-1.0",
        "generator": format!("arkavo-org/vrm-conformance vrm-asset-generator {}", env!("CARGO_PKG_VERSION")),
        "spec_section": "VRMC_materials_mtoon",
        "blake3": format!("blake3:{}", hash.to_hex()),
        "byte_size": bytes.len(),
        "params": params,
    });
    if let Some(sb) = spring_bone {
        meta["spring_bone"] = serde_json::to_value(sb)?;
        meta["spec_section"] = serde_json::Value::String(
            "VRMC_materials_mtoon + VRMC_springBone".into(),
        );
    }
    std::fs::write(out, serde_json::to_vec_pretty(&meta)?)?;
    Ok(())
}
```

Then update the existing single call site (`emit_with_sidecars` in `emit.rs`) to pass `None`:

In `crates/vrm-asset-generator/src/emit.rs`, find the call to `write_meta_json` and change:

```rust
let meta_path = stem.with_extension("meta.json");
write_meta_json(params, &vrm_path, &meta_path)?;
```

to:

```rust
let meta_path = stem.with_extension("meta.json");
write_meta_json(params, None, &vrm_path, &meta_path)?;
```

- [ ] **Step 2: Add `emit_with_sidecars_spring_bone` convenience function**

Append to `crates/vrm-asset-generator/src/emit.rs`:

```rust
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
```

- [ ] **Step 3: Wire the CLI subcommand**

In `crates/vrm-asset-generator/src/cli.rs`, find the `Cmd` enum and add (after `EmitSweep`):

```rust
/// Emit one `.vrm` carrying both default MToon material and a default
/// VRMC_springBone chain attached to the head.
EmitSpringbone {
    #[arg(long)]
    id: String,
    #[arg(long)]
    output_dir: Utf8PathBuf,
    #[arg(long)]
    json: bool,
},
```

In `run()`, add the arm before the `Describe` arm:

```rust
Cmd::EmitSpringbone {
    id,
    output_dir,
    json: emit_json,
} => {
    use crate::emit::emit_with_sidecars_spring_bone;
    use crate::spring_bone::SpringBoneParams;

    std::fs::create_dir_all(&output_dir)?;
    let stem = output_dir.join(&id);
    let mtoon = MToonParams::defaults(&id);
    let spring = SpringBoneParams::defaults(&id);
    emit_with_sidecars_spring_bone(&mtoon, &spring, &stem)?;

    if emit_json {
        let result = json!({
            "ok": true,
            "outputs": {
                "vrm": stem.with_extension("vrm"),
                "meta": stem.with_extension("meta.json"),
                "test_plan": stem.with_extension("test.yaml")
            }
        });
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("emitted: {}", stem.with_extension("vrm"));
        println!("emitted: {}", stem.with_extension("meta.json"));
        println!("emitted: {}", stem.with_extension("test.yaml"));
    }
    Ok(())
}
```

Update the `describe` catalog to include `emit-springbone`. In the `Cmd::Describe { format }` arm, find the existing `operations` object and add an `emit-springbone` entry alongside `emit-default` and `emit-sweep`:

```rust
"emit-springbone": {
    "summary": "Emit one .vrm with default MToon + default VRMC_springBone chain",
    "input_schema": {
        "type": "object",
        "required": ["id", "output_dir"],
        "properties": {
            "id": { "type": "string" },
            "output_dir": { "type": "string" }
        }
    },
    "output_schema": {
        "type": "object",
        "properties": {
            "ok": { "type": "boolean" },
            "outputs": {
                "type": "object",
                "properties": {
                    "vrm": { "type": "string" },
                    "meta": { "type": "string" },
                    "test_plan": { "type": "string" }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Smoke-test the CLI**

```bash
mkdir -p /tmp/springbone-smoke
cargo run -p vrm-asset-generator -- emit-springbone --id smoke_spring --output-dir /tmp/springbone-smoke --json
ls /tmp/springbone-smoke/
```

Expected: stdout JSON with three output paths; `smoke_spring.vrm`, `smoke_spring.meta.json`, `smoke_spring.test.yaml` present.

Verify the meta sidecar has the spring-bone params:

```bash
cat /tmp/springbone-smoke/smoke_spring.meta.json | python3 -c "
import json, sys
d = json.load(sys.stdin)
print('spec_section:', d['spec_section'])
print('spring_bone keys:', list(d.get('spring_bone', {}).keys()))
"
```

Expected: `spec_section: VRMC_materials_mtoon + VRMC_springBone`, spring_bone keys include `joint_count`, `stiffness`, `drag_force`, etc.

Verify the asset validates:

```bash
.tools/vrm-validator-cli /tmp/springbone-smoke/smoke_spring.vrm | python3 -c "
import json, sys
d = json.load(sys.stdin)
print('numErrors:', d['issues']['numErrors'])
print('numWarnings:', d['issues']['numWarnings'])
"
```

Expected: `numErrors: 0`.

- [ ] **Step 5: Run full asset-generator tests**

```bash
cargo test -p vrm-asset-generator
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green. Existing tests still pass — `emit_with_sidecars` now calls `write_meta_json` with the extra `None` argument, which doesn't change behavior.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/src/sidecar.rs
git commit -m "feat(asset-generator): emit-springbone CLI subcommand + spring-bone meta sidecar"
```

---

## Section E — Docs

### Task E1: Update README + operation contract notes

**Files:**
- Modify: `README.md` (briefly mention spring-bone asset emission)
- Modify: `docs/methodology.md` (cross-reference spring-bone scenario emission to the methodology hazards already documented)

- [ ] **Step 1: README update**

In `README.md`, find the "What this is" bullet about the parametric VRM asset generator and change:

> A **parametric VRM asset generator** that emits a deterministic test corpus covering the MToon material spec, spring bone behaviors, constraints, and expressions.

to:

> A **parametric VRM asset generator** that emits a deterministic test corpus. v0.1: full MToon material parameter sweep + single-chain VRMC_springBone scenarios (validator-clean). Constraints + expressions land in later phases.

- [ ] **Step 2: methodology.md cross-reference**

In `docs/methodology.md`, find the "Spring bone determinism" section. After the existing paragraph, append:

```markdown
**Asset emission (Phase 2D-a)**: the asset generator's `emit-springbone` subcommand produces VRM 1.0 assets with `VRMC_springBone` chains attached to the head bone. Each chain is parametrized by `SpringBoneParams` (joint_count, segment_length_m, stiffness, drag_force, gravity_power, gravity_dir, hit_radius). The renderer-side `step_physics` / `reset_physics` / `animate_root_transform` ops that exercise these assets land in 2D-b.
```

- [ ] **Step 3: Commit**

```bash
git add README.md docs/methodology.md
git commit -m "docs: spring-bone asset emission lands in 2D-a (corpus expansion notes)"
```

---

## Self-Review

**Spec coverage:**

| 2D-a goal | Task |
|---|---|
| Spring-bone parameter dictionary | A1 |
| Humanoid stub chain extension | A2 |
| VRMC_springBone JSON emission | B1, B2 |
| Validator-clean integration test | C1 |
| CLI subcommand | D1 |
| Documentation | E1 |

**Placeholder scan:** none. All code blocks are complete; tests assert behavior.

**Type consistency:**

- `SpringBoneParams` defined in A1, consumed in B1, B2, C1, D1.
- `append_spring_chain(&mut Skeleton, usize, u32, f32) -> Vec<usize>` consistent across A2, B2.
- `vrmc_spring_bone(&[usize], &SpringBoneParams) -> Value` consistent in B1, B2.
- `emit_vrm_with_spring_bone(&MToonParams, &SpringBoneParams, &Utf8Path)` consistent in B2, C1, D1.
- `write_meta_json` signature changed (added `Option<&SpringBoneParams>` second arg); D1 Step 1 updates the existing call site in `emit_with_sidecars` to pass `None`. Existing `tests/sidecar.rs` still passes because the wire-shape under `params:` is unchanged.

**YAGNI guards:**

- ✅ Single chain per asset; no colliders.
- ✅ No renderer-side changes (mock + three-vrm still return Unimplemented for `step_physics` etc.).
- ✅ No test-plan `physics:` block — current default test plans render statically.
- ✅ No spring-bone sweep matrix yet (defer to 2D-c).

**Risk register:**

- **Validator schema fidelity.** The validator may flag specific JSON shape issues (e.g. missing `center: null`, malformed joint references). C1 Step 2 flags this with explicit fix guidance: read the messages, adjust `vrmc_spring_bone` until errors hit zero. Two-test coverage (default + stiff variant) gives confidence the fix is general.
- **`write_meta_json` signature change.** Adding a parameter to a public function. The only caller is `emit_with_sidecars`; D1 Step 1 updates it. Existing `tests/sidecar.rs` round-trips the produced JSON back into a `TestPlan`, which doesn't touch the new spring-bone metadata field, so it still passes.
- **Skeleton mutation in A2.** `append_spring_chain` mutates `skeleton.nodes_json` in place. Existing emit code that reads `nodes_json` before this function is called sees the original skeleton; after the function is called, sees the extended one. The order in B2 (`minimal_skeleton()` → `append_spring_chain()` → clone into `nodes`) makes that ordering explicit.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2d-a-springbone-asset-emission.md`. Two execution options:

1. **Subagent-Driven** — fresh subagent per task. 7 tasks; A1 → A2 → B1 → B2 → C1 → D1 → E1 strictly sequential (each depends on the prior).
2. **Inline Execution (recommended)** — sequential dependencies make this a poor fit for subagent parallelism. Inline keeps the validator-iteration tight if C1 hits any JSON-shape issues.

Estimated time: ~20-30 minutes inline if the validator accepts our first VRMC_springBone JSON shape; longer if it requires JSON iteration.
