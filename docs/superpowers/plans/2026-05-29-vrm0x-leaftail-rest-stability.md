# VRM 0.x Leaf-Tail Rest-Stability Conformance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Exercise VRM 0.x leaf-tail (7 cm) spring synthesis across the orientation × length input space at zero gravity, plus a 0.x↔1.0 parity axis, so the conformance suite flags any renderer's direction-dependent leaf-tail error (surfacing symptom: VMK #306).

**Architecture:** Add two `#[serde(default)]` fields to `SpringBoneParams` — `chain_axis: [f32;3]` (chain orientation) and `explicit_tail: bool` (1.0 explicit `_end` node). Thread `chain_axis` through the three hard-coded −Y geometry sites (joint placement, cylinder mesh, inverse-bind matrices), preserving byte-identical output for the `[0,-1,0]` default. Add a focused `spring_bone_v0_leaftail_sweep()` and an `emit-springbone-leaftail-sweep` subcommand. Signal via existing consensus-diff (UniVRM oracle) + per-renderer 0.x↔1.0 parity SSIM.

**Tech Stack:** Rust (vrm-asset-generator crate), serde_json, glam (Vec3), clap CLI. Spec source: `docs/upstream-specs/vrm-specification/specification/VRMC_springBone-1.0/README.md:137-153`.

**Spec reference (the rule under test):** A 0.x chain whose final joint has no child gets a tail synthesized **7 cm along the bone's own local rest axis** (`tail_local = bone_rest_axis.normalized() * 0.07`; ref impl `adapters/godot-vrm/addons/vrm/vrm_spring_bone.gd:103`). Under `gravityPower=0` with no animation the chain must settle to that rest with zero net deformation, in every orientation.

**Design doc:** `docs/superpowers/specs/2026-05-29-vrm0x-leaftail-rest-stability-design.md`

---

## File Structure

- `crates/vrm-asset-generator/src/spring_bone.rs` — `SpringBoneParams` gains `chain_axis` + `explicit_tail`; new `spring_bone_v0_leaftail_sweep()`.
- `crates/vrm-asset-generator/src/humanoid.rs` — `append_spring_chain` placed along `chain_axis`.
- `crates/vrm-asset-generator/src/chain_mesh.rs` — `build_chain_cylinder` generalized to an arbitrary axis (was hard-coded −Y); stale module-header note corrected.
- `crates/vrm-asset-generator/src/emit.rs` — 3D inverse-bind matrices + axis-aware mesh top; `explicit_tail` appends a 7 cm `_end` node to the V1 `VRMC_springBone` joints.
- `crates/vrm-asset-generator/src/cli.rs` — `EmitSpringboneLeaftailSweep` subcommand + handler.
- `crates/vrm-asset-generator/src/vrm_ext.rs` — V1 spring joints emit (explicit-tail append point).
- `docs/methodology.md` — record the deliberate 2-factor-grid exception.
- `docs/findings.md` — cross-renderer result table (manual, Task 9).

---

## Task 1: Add `chain_axis` and `explicit_tail` to `SpringBoneParams`

**Files:**
- Modify: `crates/vrm-asset-generator/src/spring_bone.rs:10-89`
- Test: same file, `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `spring_bone.rs`:

```rust
#[test]
fn chain_axis_defaults_to_down_and_explicit_tail_false() {
    let p = SpringBoneParams::defaults("ct");
    assert_eq!(p.chain_axis, [0.0, -1.0, 0.0]);
    assert!(!p.explicit_tail);
}

#[test]
fn chain_axis_and_explicit_tail_roundtrip() {
    let mut p = SpringBoneParams::defaults("rt");
    p.chain_axis = [0.0, 0.0, 1.0];
    p.explicit_tail = true;
    let s = serde_json::to_string(&p).unwrap();
    let back: SpringBoneParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.chain_axis, [0.0, 0.0, 1.0]);
    assert!(back.explicit_tail);
}

#[test]
fn legacy_json_without_new_fields_deserializes_to_defaults() {
    // A pre-change serialized params blob has neither field.
    let legacy = r#"{"id":"x","spring_name":"x_chain","joint_count":4,
        "segment_length_m":0.05,"stiffness":0.5,"drag_force":0.5,
        "gravity_power":0.5,"gravity_dir":[0.0,-1.0,0.0],"hit_radius":0.02}"#;
    let p: SpringBoneParams = serde_json::from_str(legacy).unwrap();
    assert_eq!(p.chain_axis, [0.0, -1.0, 0.0]);
    assert!(!p.explicit_tail);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator chain_axis_defaults_to_down`
Expected: FAIL — `no field chain_axis on type SpringBoneParams`.

- [ ] **Step 3: Add the fields and defaults**

In `spring_bone.rs`, add to the struct (after `hit_radius_per_joint`, before the closing brace at line ~65):

```rust
    /// Unit direction the chain extends from its root, in the root bone's
    /// local space. Default [0,-1,0] (straight down) reproduces all
    /// pre-existing assets byte-for-byte. Off-vertical axes exercise the
    /// direction-dependent VRM 0.x leaf-tail (7 cm) synthesis.
    #[serde(default = "default_chain_axis")]
    pub chain_axis: [f32; 3],

    /// VRM 1.0 only: when true, append an explicit `_end` joint 7 cm along
    /// `chain_axis` past the leaf (mirrors how VRoid 1.0 exports the tail).
    /// V0 emit ignores this — 0.x always synthesizes the 7 cm tail. Used to
    /// build 0.x(synthesized) ↔ 1.0(explicit) parity twins. Default false.
    #[serde(default)]
    pub explicit_tail: bool,
```

Add the default helper just above `impl SpringBoneParams` (~line 66):

```rust
fn default_chain_axis() -> [f32; 3] {
    [0.0, -1.0, 0.0]
}
```

In `SpringBoneParams::defaults` (the struct literal, ~line 73-88), add the two fields:

```rust
            hit_radius_per_joint: None,
            chain_axis: [0.0, -1.0, 0.0],
            explicit_tail: false,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator spring_bone::tests`
Expected: PASS (all three new tests + existing `per_joint_taper_roundtrips`).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/spring_bone.rs
git commit -m "feat(asset-gen): add chain_axis + explicit_tail to SpringBoneParams"
```

---

## Task 2: Place the spring chain along `chain_axis`

**Files:**
- Modify: `crates/vrm-asset-generator/src/humanoid.rs:185-242`
- Test: same file

- [ ] **Step 1: Write the failing test**

Add to `humanoid.rs` tests (create the module if absent):

```rust
#[cfg(test)]
mod chain_axis_tests {
    use super::*;

    #[test]
    fn chain_extends_along_given_axis() {
        let mut sk = minimal_skeleton();
        let head = sk.bone_to_node["head"];
        let idxs = append_spring_chain_axis(&mut sk, head, 3, 0.05, [0.0, 0.0, 1.0]);
        let nodes = sk.nodes_json.as_array().unwrap();
        for &i in &idxs {
            let t = nodes[i]["translation"].as_array().unwrap();
            assert!((t[0].as_f64().unwrap() - 0.0).abs() < 1e-6);
            assert!((t[1].as_f64().unwrap() - 0.0).abs() < 1e-6);
            assert!((t[2].as_f64().unwrap() - 0.05).abs() < 1e-6, "Z segment");
        }
        // leaf (last) has no children → forces 7cm synthesis in 0.x
        let leaf = *idxs.last().unwrap();
        assert!(nodes[leaf].get("children").is_none());
    }

    #[test]
    fn default_axis_still_points_down() {
        let mut sk = minimal_skeleton();
        let head = sk.bone_to_node["head"];
        let idxs = append_spring_chain(&mut sk, head, 2, 0.05);
        let nodes = sk.nodes_json.as_array().unwrap();
        let t = nodes[idxs[0]]["translation"].as_array().unwrap();
        assert!((t[1].as_f64().unwrap() - (-0.05)).abs() < 1e-6, "-Y");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator chain_extends_along_given_axis`
Expected: FAIL — `cannot find function append_spring_chain_axis`.

- [ ] **Step 3: Generalize the implementation**

In `humanoid.rs`, replace the body of `append_spring_chain` with a delegation, and add the axis-aware variant. Change the hard-coded translation at line 209.

Replace lines 193-242 with:

```rust
pub fn append_spring_chain(
    skeleton: &mut Skeleton,
    parent_node_index: usize,
    joint_count: u32,
    segment_length_m: f32,
) -> Vec<usize> {
    append_spring_chain_axis(
        skeleton,
        parent_node_index,
        joint_count,
        segment_length_m,
        [0.0, -1.0, 0.0],
    )
}

/// Like [`append_spring_chain`] but lays the chain along `chain_axis`
/// (a unit direction in the parent's local space) instead of straight −Y.
pub fn append_spring_chain_axis(
    skeleton: &mut Skeleton,
    parent_node_index: usize,
    joint_count: u32,
    segment_length_m: f32,
    chain_axis: [f32; 3],
) -> Vec<usize> {
    if joint_count == 0 {
        return Vec::new();
    }

    let nodes = skeleton
        .nodes_json
        .as_array_mut()
        .expect("skeleton nodes_json must be an array");
    let mut chain_indices = Vec::with_capacity(joint_count as usize);

    let segment_translation = json!([
        chain_axis[0] * segment_length_m,
        chain_axis[1] * segment_length_m,
        chain_axis[2] * segment_length_m,
    ]);

    for i in 0..joint_count {
        let my_idx = nodes.len();
        let mut node = json!({
            "name": format!("spring_joint_{i}"),
            "translation": segment_translation.clone(),
        });
        if i + 1 < joint_count {
            node["children"] = json!([my_idx + 1]);
        }
        nodes.push(node);
        chain_indices.push(my_idx);
    }

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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator chain_axis_tests`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/humanoid.rs
git commit -m "feat(asset-gen): place spring chain along configurable axis"
```

---

## Task 3: Generalize the cylinder mesh to an arbitrary axis (byte-identical default)

**Files:**
- Modify: `crates/vrm-asset-generator/src/chain_mesh.rs:49-132` (signature + body), `:1-31` (stale header), existing tests `:138-207`
- Test: same file

- [ ] **Step 1: Write the failing tests**

Replace the existing test calls that pass `top_world_y: f32` with a 3D start + axis, and add an off-axis test. Add/replace in the tests module:

```rust
    // Helper: default −Y axis reproduces the legacy vertical layout.
    fn down_cyl(joints: u32, seg: f32, r: f32, top_y: f32, segs: u32) -> SkinnedMeshData {
        build_chain_cylinder(joints, seg, r, [0.0, top_y, 0.0], [0.0, -1.0, 0.0], segs)
    }

    #[test]
    fn default_axis_reproduces_legacy_vertical_positions() {
        let m = down_cyl(4, 0.05, 0.02, 1.31, 8);
        // ring 0 at Y=1.31 on X/Z circle of radius 0.02
        for v in &m.positions[..8] {
            assert!((v[1] - 1.31).abs() < 1e-6, "ring0 Y={}", v[1]);
            let r = (v[0] * v[0] + v[2] * v[2]).sqrt();
            assert!((r - 0.02).abs() < 1e-6);
        }
        // tail ring at Y = 1.31 - 4*0.05 = 1.11
        let last = m.positions.len() - 8;
        for v in &m.positions[last..] {
            assert!((v[1] - 1.11).abs() < 1e-6, "tail Y={}", v[1]);
        }
    }

    #[test]
    fn forward_axis_walks_along_z() {
        // top at head height, axis +Z, 2 joints @ 0.05
        let m = build_chain_cylinder(2, 0.05, 0.02, [0.0, 1.16, 0.0], [0.0, 0.0, 1.0], 8);
        // ring 0 centered at Z=0.0 (top), tail ring (ring 2) at Z = 2*0.05 = 0.10
        let last = m.positions.len() - 8;
        // ring centers: average of the ring's vertices
        let cz: f32 = m.positions[last..].iter().map(|v| v[2]).sum::<f32>() / 8.0;
        assert!((cz - 0.10).abs() < 1e-5, "tail ring center Z={cz}");
        // and the ring lies in the X/Y plane (perpendicular to +Z): all verts share Z≈0.10
        for v in &m.positions[last..] {
            assert!((v[2] - 0.10).abs() < 1e-5, "tail vert Z={}", v[2]);
        }
    }
```

Update the four pre-existing tests (`vertex_count_matches_rings_times_segments`, `index_count_matches_quads_per_ring_gap`, `ring_zero_sits_at_top_world_y`, `tail_ring_extends_to_chain_tip`, `each_ring_is_hard_weighted_to_its_joint`, `ring_vertices_are_at_correct_radius`) to call `down_cyl(...)` instead of `build_chain_cylinder(joints, seg, r, top_y, segs)`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator chain_mesh`
Expected: FAIL — arity mismatch / `forward_axis_walks_along_z` unresolved until signature changes.

- [ ] **Step 3: Generalize `build_chain_cylinder`**

Replace the signature and body (lines 59-132). The key idea: build each ring around a center walking along `axis`, with the ring circle in the plane spanned by an orthonormal basis `(u, v)` perpendicular to `axis`. For an axis parallel to ±Y, pin `u = +X`, `v = +Z` so the default reproduces the legacy layout exactly.

```rust
/// Build a cylinder of `joint_count + 1` rings starting at `top_world` and
/// stepping `segment_length_m` along `axis` per ring. Each ring is a circle
/// of `ring_segments` verts in the plane perpendicular to `axis`, hard-weighted
/// to its joint (ring N reuses joint N-1 so the tail caps cleanly).
///
/// For `axis` parallel to ±Y the in-plane basis is pinned to (+X, +Z) so the
/// historical vertical layout is reproduced byte-for-byte.
pub fn build_chain_cylinder(
    joint_count: u32,
    segment_length_m: f32,
    radius: f32,
    top_world: [f32; 3],
    axis: [f32; 3],
    ring_segments: u32,
) -> SkinnedMeshData {
    assert!(joint_count > 0, "chain mesh needs at least 1 joint");
    assert!(ring_segments >= 3, "ring needs at least 3 verts");

    let n_rings = joint_count as usize + 1;
    let n_segs = ring_segments as usize;
    let n_verts = n_rings * n_segs;

    let a = Vec3::from_array(axis).normalize();
    let top = Vec3::from_array(top_world);
    let (u, v) = perp_basis(a);

    let mut positions = Vec::with_capacity(n_verts);
    let mut normals = Vec::with_capacity(n_verts);
    let mut uvs = Vec::with_capacity(n_verts);
    let mut joints = Vec::with_capacity(n_verts);
    let mut weights = Vec::with_capacity(n_verts);

    for ring in 0..n_rings {
        let center = top + a * (ring as f32 * segment_length_m);
        let weighted_joint = ring.min(joint_count as usize - 1) as u16;

        for seg in 0..n_segs {
            let phi = (seg as f32) * 2.0 * std::f32::consts::PI / (n_segs as f32);
            let radial = u * phi.cos() + v * phi.sin();
            let p = center + radial * radius;
            let uv = Vec2::new(
                (seg as f32) / (n_segs as f32),
                (ring as f32) / (n_rings as f32 - 1.0).max(1.0),
            );

            positions.push(p.into());
            normals.push(radial.into());
            uvs.push(uv.into());
            joints.push([weighted_joint, 0, 0, 0]);
            weights.push([1.0, 0.0, 0.0, 0.0]);
        }
    }

    let mut indices = Vec::with_capacity(2 * n_segs * (n_rings - 1) * 3);
    for r in 0..n_rings - 1 {
        for s in 0..n_segs {
            let s_next = (s + 1) % n_segs;
            let i00 = (r * n_segs + s) as u32;
            let i01 = (r * n_segs + s_next) as u32;
            let i10 = ((r + 1) * n_segs + s) as u32;
            let i11 = ((r + 1) * n_segs + s_next) as u32;
            indices.extend_from_slice(&[i00, i10, i01, i01, i10, i11]);
        }
    }

    SkinnedMeshData { positions, normals, uvs, indices, joints, weights }
}

/// Orthonormal basis (u, v) spanning the plane perpendicular to unit `a`.
/// Pinned to (+X, +Z) when `a` is parallel to ±Y so the legacy vertical
/// cylinder is reproduced exactly.
fn perp_basis(a: Vec3) -> (Vec3, Vec3) {
    if a.x.abs() < 1e-6 && a.z.abs() < 1e-6 {
        // a parallel to ±Y → legacy layout: radial X = cos, Z = sin.
        (Vec3::X, Vec3::Z)
    } else {
        let u = a.cross(Vec3::Y).normalize();
        let v = a.cross(u).normalize();
        (u, v)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator chain_mesh`
Expected: PASS (legacy tests via `down_cyl`, plus `forward_axis_walks_along_z`).

- [ ] **Step 5: Correct the stale module header**

Replace `chain_mesh.rs:15-31` ("## Status: deferred infrastructure" block) with:

```rust
//! ## Status: wired
//!
//! The chain cylinder is emitted alongside the head sphere by both
//! `emit_vrm_with_spring_bone` (1.0) and `emit_vrm_with_spring_bone_v0` (0.x).
//! VRMMetalKit 0.13.1 closed the non-skinned-mesh-drop bug
//! ([VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181)),
//! so sphere + chain coexist across all renderers.
```

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/chain_mesh.rs
git commit -m "feat(asset-gen): build chain cylinder along arbitrary axis (byte-identical default)"
```

---

## Task 4: Axis-aware inverse-bind matrices + mesh top in emit paths

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs:788-825` (v1) and `:1007-1040` (v0)
- Test: `crates/vrm-asset-generator/src/emit.rs` tests, or a new integration test asserting default-asset byte identity (Task 5).

- [ ] **Step 1: Write the failing test (off-axis IBM)**

Add a unit test in `emit.rs` `#[cfg(test)]` that calls a small helper. First introduce the helper so it's testable; add near the spring emit fns:

```rust
/// World position of chain joint `i` (0-based) given the chain root world,
/// the unit `chain_axis`, and `segment_length_m`. Joint 0 sits one segment
/// from the root.
fn chain_joint_world(root: [f32; 3], axis: [f32; 3], seg: f32, i: u32) -> [f32; 3] {
    let step = (i + 1) as f32 * seg;
    [
        root[0] + axis[0] * step,
        root[1] + axis[1] * step,
        root[2] + axis[2] * step,
    ]
}

/// Column-major glTF Mat4 that is a pure inverse translation of `p`.
fn inv_translation_mat4(p: [f32; 3]) -> [f32; 16] {
    #[rustfmt::skip]
    let m = [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        -p[0], -p[1], -p[2], 1.0,
    ];
    m
}
```

Test:

```rust
#[test]
fn ibm_default_axis_matches_legacy_y_only() {
    let head = crate::humanoid::rest_pose_world_position("head");
    // legacy: jy = head_y - (i+1)*seg ; IBM translation = (0, -jy, 0)
    let seg = 0.05;
    for i in 0..4u32 {
        let p = chain_joint_world(head, [0.0, -1.0, 0.0], seg, i);
        let m = inv_translation_mat4(p);
        let jy = head[1] - ((i + 1) as f32) * seg;
        assert!((m[13] - (-jy)).abs() < 1e-6, "element 13");
        assert!(m[12].abs() < 1e-6 && m[14].abs() < 1e-6, "X/Z translation zero");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator ibm_default_axis_matches_legacy_y_only`
Expected: FAIL — `chain_joint_world` / `inv_translation_mat4` not defined.

- [ ] **Step 3: Wire the helpers into both emit paths**

In `emit_vrm_with_spring_bone` (v1), replace lines 788-825 with the following. Note `chain_joint_world(.., i)` returns joint `i`'s world position (joint `i` sits `(i+1)*seg` from head), so joint 0 — the chain/mesh top — is `chain_joint_world(.., 0)`:

```rust
    let chain_nodes = crate::humanoid::append_spring_chain_axis(
        &mut skeleton,
        head_node,
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        spring_bone.chain_axis,
    );

    let head_world = crate::humanoid::rest_pose_world_position("head");
    // Joint 0 (chain top) = head + axis * segment_length.
    let chain_top = chain_joint_world(
        head_world,
        spring_bone.chain_axis,
        spring_bone.segment_length_m,
        0,
    );

    let chain_mesh = crate::chain_mesh::build_chain_cylinder(
        spring_bone.joint_count,
        spring_bone.segment_length_m,
        /* radius */ 0.025,
        chain_top,
        spring_bone.chain_axis,
        /* ring_segments */ 12,
    );

    // Inverse-bind matrices: joint i bind-pose world = head + axis*(i+1)*seg.
    let inv_bind: Vec<[f32; 16]> = (0..spring_bone.joint_count)
        .map(|i| {
            let p = chain_joint_world(
                head_world,
                spring_bone.chain_axis,
                spring_bone.segment_length_m,
                i,
            );
            inv_translation_mat4(p)
        })
        .collect();
```

Apply the **same** replacement to `emit_vrm_with_spring_bone_v0` at lines 1007-1038, substituting the local variable name `spring` for `spring_bone` (that function uses `spring`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator ibm_default_axis_matches_legacy_y_only && cargo build -p vrm-asset-generator`
Expected: PASS + clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(asset-gen): axis-aware inverse-bind matrices and mesh top"
```

---

## Task 5: Byte-identity regression guard for the default asset

**Files:**
- Test: `crates/vrm-asset-generator/tests/leaftail_byte_identity.rs` (new)

This guards the spec invariant: `chain_axis=[0,-1,0]`, `explicit_tail=false` ⇒ output unchanged from before this feature.

- [ ] **Step 1: Capture the pre-change baseline (one-time, manual within this task)**

Before merging Task 1, the default `emit-springbone-sweep` output existed. Generate a baseline now from the current default path and store its BLAKE3:

Run:
```bash
cargo run -p vrm-asset-generator -- emit-default --id baseline_sb --output-dir /tmp/sb_base
# (use the spring-bone default emit; see Step 2 for the in-test approach)
```

Prefer the in-test approach below (no external file): emit the default spring asset to a temp path and assert the GLB bytes' BLAKE3 equals a constant captured from `main` before Task 1. To capture: `git stash`, build, emit, `blake3sum`, then `git stash pop`.

- [ ] **Step 2: Write the test**

```rust
use camino::Utf8PathBuf;
use vrm_asset_generator::emit::emit_vrm_with_spring_bone;
use vrm_asset_generator::mtoon::MToonParams;
use vrm_asset_generator::spring_bone::SpringBoneParams;

// BLAKE3 of the default spring-bone GLB captured from `main` prior to the
// chain_axis feature. If this test fails, the [0,-1,0] default path is no
// longer byte-identical — fix the geometry, do not update this hash casually.
const DEFAULT_SB_GLB_BLAKE3: &str = "<PASTE_HASH_FROM_STEP_1>";

#[test]
fn default_spring_bone_asset_is_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::from_path_buf(dir.path().join("sb.vrm")).unwrap();
    let mtoon = MToonParams::defaults("sb_identity");
    let spring = SpringBoneParams::defaults("sb_identity");
    emit_vrm_with_spring_bone(&mtoon, &spring, &out).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let got = blake3::hash(&bytes).to_hex().to_string();
    assert_eq!(got, DEFAULT_SB_GLB_BLAKE3, "default −Y output drifted");
}
```

> If `MToonParams`/`emit` are not exported from the crate root, add `pub use` in `lib.rs` or move this to an in-crate `#[cfg(test)]` module. `tempfile` and `blake3` are already workspace deps (used elsewhere); confirm with `cargo tree -p vrm-asset-generator | grep -E 'blake3|tempfile'` and add as `[dev-dependencies]` if missing.

- [ ] **Step 3: Fill the hash and run**

Replace `<PASTE_HASH_FROM_STEP_1>` with the captured BLAKE3.
Run: `cargo test -p vrm-asset-generator default_spring_bone_asset_is_byte_identical`
Expected: PASS. If FAIL, the −Y path drifted in Tasks 2-4; fix geometry until identical.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-asset-generator/tests/leaftail_byte_identity.rs
git commit -m "test(asset-gen): byte-identity guard for default spring-bone geometry"
```

---

## Task 6: `explicit_tail` appends a 7 cm `_end` joint in the V1 path

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (v1 emit — add the `_end` skeleton node) and `crates/vrm-asset-generator/src/vrm_ext.rs:481-540` (append the joint to `VRMC_springBone.springs[].joints`)
- Test: `crates/vrm-asset-generator/src/emit.rs` tests (assert node count + joints length)

The `_end` node is a spring-simulation-only joint (not mesh-weighted) placed exactly 7 cm along `chain_axis` past the leaf — matching the 0.x synthesized tail so the parity twin is geometrically equivalent.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn explicit_tail_v1_adds_end_joint_7cm_along_axis() {
    let dir = tempfile::tempdir().unwrap();
    let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("et.vrm")).unwrap();
    let mtoon = crate::mtoon::MToonParams::defaults("et");
    let mut spring = SpringBoneParams::defaults("et");
    spring.joint_count = 2;
    spring.chain_axis = [0.0, 0.0, 1.0];
    spring.explicit_tail = true;
    emit_vrm_with_spring_bone(&mtoon, &spring, &out).unwrap();

    // Parse the GLB JSON chunk and locate VRMC_springBone joints.
    let json = crate::test_support::read_glb_json(&out); // helper below
    let springs = &json["extensions"]["VRMC_springBone"]["springs"];
    let joints = springs[0]["joints"].as_array().unwrap();
    // 2 chain joints + 1 explicit _end = 3
    assert_eq!(joints.len(), 3, "explicit tail joint appended");

    // The _end node translation is chain_axis * 0.07.
    let end_node_idx = joints[2]["node"].as_u64().unwrap() as usize;
    let t = json["nodes"][end_node_idx]["translation"].as_array().unwrap();
    assert!((t[2].as_f64().unwrap() - 0.07).abs() < 1e-6, "7cm along +Z");
}
```

> If a GLB-JSON read helper does not already exist in the crate's tests, add a minimal one in a `#[cfg(test)] mod test_support` that reads the GLB header, finds the JSON chunk (`type 0x4E4F534A`), and `serde_json::from_slice`s it. Check first: `rg "read_glb_json|JSON chunk|0x4E4F534A" crates/vrm-asset-generator`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator explicit_tail_v1_adds_end_joint`
Expected: FAIL — joints length 2, no `_end` node.

- [ ] **Step 3: Add the `_end` node in v1 emit**

In `emit_vrm_with_spring_bone`, after `append_spring_chain_axis` returns `chain_nodes` and before assembling the `VRMC_springBone` extension, conditionally append the tail node and remember its index:

```rust
    // Optional explicit 7 cm tail (VRM 1.0 parity twin). Mirrors the 0.x
    // synthesized tail: a sim-only joint, not mesh-weighted.
    let explicit_tail_node: Option<usize> = if spring_bone.explicit_tail {
        let nodes_arr = skeleton.nodes_json.as_array_mut().unwrap();
        let end_idx = nodes_arr.len();
        nodes_arr.push(json!({
            "name": "spring_joint_end",
            "translation": [
                spring_bone.chain_axis[0] * 0.07,
                spring_bone.chain_axis[1] * 0.07,
                spring_bone.chain_axis[2] * 0.07,
            ],
        }));
        // Parent the _end node under the leaf joint.
        let leaf = *chain_nodes.last().unwrap();
        let leaf_node = &mut nodes_arr[leaf];
        let mut kids = leaf_node.get("children").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        kids.push(json!(end_idx));
        leaf_node["children"] = Value::Array(kids);
        Some(end_idx)
    } else {
        None
    };
```

Then thread `explicit_tail_node` into the spring-extension builder call. Locate where `emit_vrm_with_spring_bone` builds the `VRMC_springBone` joints (it passes `chain_nodes` to a `vrm_ext.rs` helper — confirm the exact call with `rg "VRMC_springBone|springs|joints" crates/vrm-asset-generator/src/emit.rs`). Construct the full joints node list:

```rust
    let mut spring_joint_nodes = chain_nodes.clone();
    if let Some(end) = explicit_tail_node {
        spring_joint_nodes.push(end);
    }
    // pass spring_joint_nodes (not chain_nodes) into the VRMC_springBone builder
```

- [ ] **Step 4: Append the joint in the V1 joints loop**

The joints array in `vrm_ext.rs:489-535` maps over the chain's node list. If `emit.rs` now passes `spring_joint_nodes` (chain + optional end), the existing loop already emits one joint per node — including the `_end` — with the params' single-value `hitRadius`/`stiffness`/etc. No per-joint override array covers the extra index, so confirm `joint_value` tolerates `j_idx == joint_count-1+1`: it falls back to the scalar when the per-joint vec is `None` (the leaftail variants set no per-joint arrays), so it is safe. Add an assertion comment only — no code change needed if `emit.rs` passes the extended node list.

> If instead the spring builder derives joint nodes internally (not from a passed list), add an `explicit_tail_node: Option<usize>` parameter to that builder and `joints_json.push(...)` the end joint after the loop, e.g.:
> ```rust
> if let Some(end) = explicit_tail_node {
>     joints_json.push(json!({
>         "node": end,
>         "hitRadius": params.hit_radius,
>         "stiffness": params.stiffness,
>         "gravityPower": params.gravity_power,
>         "gravityDir": params.gravity_dir,
>         "dragForce": params.drag_force,
>     }));
> }
> ```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator explicit_tail_v1_adds_end_joint`
Expected: PASS.

- [ ] **Step 6: Confirm V0 ignores the flag**

Add:

```rust
#[test]
fn explicit_tail_ignored_in_v0() {
    let dir = tempfile::tempdir().unwrap();
    let out = camino::Utf8PathBuf::from_path_buf(dir.path().join("v0.vrm")).unwrap();
    let mtoon = crate::mtoon::MToonParams::defaults("v0et");
    let mut spring = SpringBoneParams::defaults("v0et");
    spring.joint_count = 2;
    spring.explicit_tail = true; // must be ignored
    emit_vrm_with_spring_bone_v0(&mtoon, &spring, &out).unwrap();
    let json = crate::test_support::read_glb_json(&out);
    let bones = json["extensions"]["VRM"]["secondaryAnimation"]["boneGroups"][0]["bones"]
        .as_array().unwrap();
    assert_eq!(bones.len(), 1, "0.x lists only the root regardless of explicit_tail");
}
```

Run: `cargo test -p vrm-asset-generator explicit_tail_ignored_in_v0`
Expected: PASS (V0 emit never reads `explicit_tail`).

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/src/vrm_ext.rs
git commit -m "feat(asset-gen): explicit 7cm _end tail joint for VRM 1.0 parity twin"
```

---

## Task 7: `spring_bone_v0_leaftail_sweep()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/spring_bone.rs` (new sweep fn near `spring_bone_coupling_sweep`)
- Test: same file

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn leaftail_sweep_cells_are_zero_gravity_and_cover_orientation_and_length() {
    let v = spring_bone_v0_leaftail_sweep();
    // every cell is zero-gravity (isolates synthesis from gravity)
    assert!(v.iter().all(|p| p.gravity_power == 0.0), "all gravity=0");
    // orientation coverage: the 6 cardinals present among short cells
    let axes: Vec<[f32; 3]> = v.iter().map(|p| p.chain_axis).collect();
    for a in [[0.0,-1.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0],[0.0,0.0,-1.0],[1.0,0.0,0.0],[-1.0,0.0,0.0]] {
        assert!(axes.contains(&a), "missing axis {a:?}");
    }
    // length coverage on +Z: joint counts 2,4,8 all appear with +Z
    for jc in [2u32, 4, 8] {
        assert!(v.iter().any(|p| p.chain_axis == [0.0,0.0,1.0] && p.joint_count == jc),
            "missing +Z len {jc}");
    }
    // the #306 anchor exists and is labeled
    assert!(v.iter().any(|p| p.id == "sb0_leaftail_axis_posZ"));
    // parity cells request explicit_tail (consumed only by V1 emit)
    assert!(v.iter().any(|p| p.id.contains("parity") && p.explicit_tail));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator leaftail_sweep_cells`
Expected: FAIL — `spring_bone_v0_leaftail_sweep` not found.

- [ ] **Step 3: Implement the sweep**

Add to `spring_bone.rs`:

```rust
/// VRM 0.x leaf-tail (7 cm) rest-stability sweep — VMK #306 gap area.
///
/// The spec (`VRMC_springBone-1.0/README.md:137-153`) synthesizes a 7 cm tail
/// along the leaf bone's local axis for childless 0.x chains. The error class
/// is direction-dependent: vertical chains tolerate a wrong synthesis (leaf
/// axis ≈ gravity axis); off-vertical chains collapse. This sweep spans the
/// orientation × length interaction at zero gravity (so rest = pure synthesis,
/// no gravity confound), plus parity cells whose V1 twins carry an explicit
/// 7 cm `_end` (set `explicit_tail`). `sb0_leaftail_axis_posZ` is the #306
/// bust-analog anchor.
pub fn spring_bone_v0_leaftail_sweep() -> Vec<SpringBoneParams> {
    let mut out = Vec::new();

    // helper: a zero-gravity short (2-joint) cell along a given axis.
    let cell = |id: &str, axis: [f32; 3]| {
        let mut p = SpringBoneParams::defaults(id);
        p.joint_count = 2;
        p.gravity_power = 0.0;
        p.chain_axis = axis;
        p
    };

    // Axis A — orientation (short chain): 6 cardinals + 2 diagonals.
    out.push(cell("sb0_leaftail_axis_negY", [0.0, -1.0, 0.0])); // control
    out.push(cell("sb0_leaftail_axis_posY", [0.0, 1.0, 0.0]));
    out.push(cell("sb0_leaftail_axis_posZ", [0.0, 0.0, 1.0])); // #306 anchor
    out.push(cell("sb0_leaftail_axis_negZ", [0.0, 0.0, -1.0]));
    out.push(cell("sb0_leaftail_axis_posX", [1.0, 0.0, 0.0]));
    out.push(cell("sb0_leaftail_axis_negX", [-1.0, 0.0, 0.0]));
    let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
    out.push(cell("sb0_leaftail_axis_diagYZ", [0.0, inv_sqrt2, inv_sqrt2]));
    out.push(cell("sb0_leaftail_axis_diagXZ", [inv_sqrt2, 0.0, inv_sqrt2]));

    // Axis B — length interaction on +Z (the 2-joint +Z cell already exists
    // above as the anchor; add 4 and 8).
    for jc in [4u32, 8] {
        let mut p = SpringBoneParams::defaults(format!("sb0_leaftail_len_{jc}"));
        p.joint_count = jc;
        p.gravity_power = 0.0;
        p.chain_axis = [0.0, 0.0, 1.0];
        out.push(p);
    }

    // Axis C — 0.x↔1.0 parity. Same geometry; V0 synthesizes, V1 emits the
    // explicit 7 cm _end (explicit_tail). Emit this sweep under BOTH
    // --spec-version 0.x and --spec-version 1.0 to get the twins.
    let mut short = cell("sb0_leaftail_parity_short", [0.0, 0.0, 1.0]);
    short.explicit_tail = true;
    out.push(short);
    let mut long = SpringBoneParams::defaults("sb0_leaftail_parity_long");
    long.joint_count = 8;
    long.gravity_power = 0.0;
    long.chain_axis = [0.0, 0.0, 1.0];
    long.explicit_tail = true;
    out.push(long);

    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator leaftail_sweep_cells`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/spring_bone.rs
git commit -m "feat(asset-gen): spring_bone_v0_leaftail_sweep (orientation × length + parity)"
```

---

## Task 8: `emit-springbone-leaftail-sweep` subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (enum variant ~line 238 region; handler ~line 1006 region)
- Test: a CLI smoke test (run the binary, assert assets + sidecars emitted)

- [ ] **Step 1: Add the subcommand enum variant**

In `cli.rs`, next to `EmitSpringboneSwingSweep` (~line 238), add:

```rust
    /// Emit the VRM 0.x leaf-tail rest-stability sweep (orientation × length
    /// + parity; 12 cells). All cells are zero-gravity static (settle 30,
    /// no animation). Run under --spec-version 0.x for the synthesized-tail
    /// test, and again under 1.0 for the explicit-tail parity twins.
    EmitSpringboneLeaftailSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long, default_value = "0.x", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 2: Add the handler**

In the `match cmd` block (next to `Cmd::EmitSpringboneSweep`, ~line 1006), add:

```rust
        Cmd::EmitSpringboneLeaftailSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::emit::{emit_with_sidecars_spring_bone, emit_with_sidecars_spring_bone_v0};
            use crate::spring_bone::spring_bone_v0_leaftail_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_v0_leaftail_sweep();
            let total = variants.len();
            let mut emitted = Vec::new();
            for (i, spring) in variants.iter().enumerate() {
                if emit_json {
                    eprintln!("{}", serde_json::to_string(&json!({
                        "event": "progress", "op": "emit-springbone-leaftail-sweep",
                        "index": i, "total": total, "id": spring.id
                    }))?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, spring.id);
                }
                let stem = output_dir.join(&spring.id);
                let mtoon = MToonParams::defaults(&spring.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => {
                        emit_with_sidecars_spring_bone_v0(&mtoon, spring, &stem)?
                    }
                    vrm_ops::SpecVersion::V1 => {
                        emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?
                    }
                }
                emitted.push(stem);
            }
            if emit_json {
                println!("{}", serde_json::to_string(&json!({
                    "ok": true, "count": emitted.len(),
                    "output_dir": output_dir, "assets": emitted
                }))?);
            } else {
                println!("emitted {} leaftail assets to {}", emitted.len(), output_dir);
            }
            Ok(())
        }
```

- [ ] **Step 3: Verify it builds and the describe catalog includes it**

Run: `cargo build -p vrm-asset-generator && cargo run -p vrm-asset-generator -- describe --format json | grep -i leaftail`
Expected: build clean; `describe` lists `emit-springbone-leaftail-sweep` (clap derives this automatically — if `describe` is hand-maintained, add the entry; check with `rg "emit-springbone-swing-sweep" crates/vrm-asset-generator/src`).

- [ ] **Step 4: Smoke test**

Run:
```bash
cargo run -p vrm-asset-generator -- emit-springbone-leaftail-sweep --output-dir /tmp/leaftail-v0 --spec-version 0.x
ls /tmp/leaftail-v0/*.vrm | wc -l        # expect 12
ls /tmp/leaftail-v0/*.test.yaml | wc -l  # expect 12
cargo run -p vrm-asset-generator -- emit-springbone-leaftail-sweep --output-dir /tmp/leaftail-v1 --spec-version 1.0
```
Expected: 12 `.vrm` + 12 `.test.yaml` + 12 `.meta.json` per dir; no errors.

- [ ] **Step 5: Validate the emitted VRMs (validator-gated)**

Run (if `.tools/vrm-validator-cli` installed):
```bash
for f in /tmp/leaftail-v0/*.vrm; do cargo run -p vrm-validator-wrap -- "$f" --json | grep -q '"valid":true' || echo "INVALID: $f"; done
```
Expected: no `INVALID` lines.

- [ ] **Step 6: Confirm sidecar pins (gravity=0 in asset, settle 30, tone_mapping none, no animation)**

Run: `grep -E "settle_steps|tone_mapping|animation|render_sequence" /tmp/leaftail-v0/sb0_leaftail_axis_posZ.test.yaml`
Expected: `settle_steps: 30`, `tone_mapping: none` (or `None`), and **no** `animation:`/`render_sequence:` keys.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(cli): emit-springbone-leaftail-sweep subcommand"
```

---

## Task 9: Methodology exception + lint/format gate

**Files:**
- Modify: `docs/methodology.md` (spring-bone section, near lines 93-101)

- [ ] **Step 1: Record the 2-factor-grid exception**

Append to the spring-bone methodology section:

```markdown
### Exception: VRM 0.x leaf-tail sweep is a 2-factor grid

The basic sweeps are one-axis-at-a-time to keep regressions un-confounded. The
`sb0_leaftail_*` family (`spring_bone_v0_leaftail_sweep`) is a deliberate
**orientation × length** grid: the VRM 0.x leaf-tail (7 cm) synthesis error
this family targets is *defined* by that interaction (off-vertical short chains
collapse; vertical or long chains tolerate the same error). The confound is the
object of study. All cells are zero-gravity static (settle 30, no animation,
`tone_mapping: none`) so the measured signal is pure synthesis rest, not
gravity. Spec: `upstream-specs/.../VRMC_springBone-1.0/README.md:137-153`.
Surfacing symptom: [VRMMetalKit#306](https://github.com/arkavo-org/VRMMetalKit/issues/306).
```

- [ ] **Step 2: Run the full gate**

Run:
```bash
cargo fmt --all -- --check
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo test -p vrm-asset-generator
```
Expected: all clean (CI mirrors these).

- [ ] **Step 3: Commit**

```bash
git add docs/methodology.md
git commit -m "docs(methodology): record leaftail 2-factor-grid exception"
```

---

## Task 10 (manual, cross-renderer): goldens, consensus, parity, findings

Not a TDD task — produces the renderer evidence and the VMK-facing deliverable. Requires local real adapters (UniVRM, godot-vrm, three-vrm, vrm-metal-kit).

- [ ] **Step 1: Bootstrap goldens for the leaftail corpus through every available adapter**

Run (per `scripts/bootstrap-goldens.sh` conventions; scope to the leaftail assets):
```bash
GOLDENS_DIR=/tmp/leaftail-goldens scripts/bootstrap-goldens.sh   # or per-adapter execute-test-plan loops over /tmp/leaftail-v0 and /tmp/leaftail-v1
```

- [ ] **Step 2: Per-cell consensus across renderers (0.x synthesized)**

Run `consensus-diff` per `sb0_leaftail_*` cell over the 0.x renders:
```bash
cargo run -p vrm-runner -- consensus-diff --plan /tmp/leaftail-v0/sb0_leaftail_axis_posZ.test.yaml \
  --render univrm=<png> --render godot=<png> --render three-vrm=<png> --render vrm-metal-kit=<png>
```
Expected pattern (hypothesis to confirm): spec-correct renderers cluster on every cell; a synthesis-error renderer (e.g. VMK) diverges on off-vertical **short** cells (`axis_posZ/negZ/posX/negX`, `parity_short`) and agrees on `axis_negY` and on **long** cells (`len_8`, `parity_long`). The pattern localizes the defect.

- [ ] **Step 3: Per-renderer 0.x↔1.0 parity SSIM**

For each renderer, diff its `parity_short` 0.x render vs its `parity_short` 1.0 render (same for `parity_long`):
```bash
cargo run -p vrm-runner -- diff --plan /tmp/leaftail-v0/sb0_leaftail_parity_short.test.yaml \
  --render /tmp/leaftail-v0/<renderer>/parity_short.png \
  --reference /tmp/leaftail-v1/<renderer>/parity_short.png --renderer-name <renderer> --json
```
Expected: high SSIM per renderer (0.x synthesis reproduces its own 1.0 explicit tail). A low parity SSIM is itself a finding even if the renderer is self-consistent across cells.

- [ ] **Step 4: Write the findings entry**

Append a dated entry to `docs/findings.md` with: the orientation × length consensus table, the per-renderer parity SSIMs, the verdict (which renderer mis-synthesizes and on which cells), citing #306 as the surfacing symptom and pointing the VMK team at their own rest-stability unit test (`assert chain tail == authored rest when gravityPower=0`) as the fix-side check. Per CLAUDE.md, `findings.md` is a deliverable — full result tables, not a summary.

- [ ] **Step 5: Commit**

```bash
git add docs/findings.md
git commit -m "docs(findings): VRM 0.x leaf-tail rest-stability cross-renderer table (#306)"
```

---

## Self-Review Notes

- **Spec coverage:** orientation axis (Task 7 Axis A), length axis (Axis B), 0.x↔1.0 parity (Axis C + Task 6 explicit_tail), zero-gravity pin (sweep sets `gravity_power=0`; sidecar settle/tone_mapping in Task 8 Step 6), byte-identity invariant (Task 5), methodology exception (Task 9), findings deliverable (Task 10). All design-doc sections map to a task.
- **Type consistency:** `chain_axis: [f32;3]`, `explicit_tail: bool`, `append_spring_chain_axis`, `build_chain_cylinder(.., top_world:[f32;3], axis:[f32;3], ..)`, `chain_joint_world`, `inv_translation_mat4`, `spring_bone_v0_leaftail_sweep`, `EmitSpringboneLeaftailSweep` — names used consistently across tasks.
- **Known investigation points flagged inline (verify before coding the step, not placeholders):** exact `VRMC_springBone` joints call site in `emit.rs`/`vrm_ext.rs` (Task 6 Step 3-4), presence of a GLB-JSON test helper (Task 6 Step 1), whether `describe` is clap-derived or hand-maintained (Task 8 Step 3), and `blake3`/`tempfile` dev-dep presence (Task 5 Step 2). Each has an `rg`/`cargo tree` command to resolve it.
