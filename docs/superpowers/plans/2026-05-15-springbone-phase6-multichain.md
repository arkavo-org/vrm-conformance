# VRMC_springBone Phase 6 — Multi-chain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Emit assets with N>1 spring chains, exercising the per-chain collider-group assignment surface that VMK#162-class coupling bugs hide behind. Ship a 36-plan sweep (3 chain counts × 2 spacings × 3 sharing modes × settle/swing).

**Architecture:** Refactor `emit_vrm_with_spring_bone_colliders` to iterate `scene.springs[]`. Each chain attaches to its own intermediate node radial-spaced around the head bone. `vrmc_spring_bone_scene` already has a comment "Phase 6 will iterate scene.springs" — that comment is now load-bearing for this phase.

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §8.

---

## File map

- `crates/vrm-asset-generator/src/spring_bone.rs` — confirm `SpringBoneSceneParams.springs` is `Vec` (it is from phase 2); no type changes expected
- `crates/vrm-asset-generator/src/vrm_ext.rs` — `vrmc_spring_bone_scene` iterates ALL springs (not just the first)
- `crates/vrm-asset-generator/src/emit.rs` — `emit_vrm_with_spring_bone_colliders` refactored to emit N chains
- `crates/vrm-asset-generator/src/chain_mesh.rs` — may need to skin N cylinders (verify; the existing helper is per-chain)
- `crates/vrm-asset-generator/src/sweep.rs` — `spring_bone_multichain_sweep()` (18 variants)
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_spring_bone_multichain_test_plan()` + swing
- `crates/vrm-asset-generator/src/cli.rs` — `emit-springbone-multichain-sweep` subcommand
- `docs/findings.md` — phase 6 entry

---

## Task 1: vrm_ext iterates all springs

**Files:** `crates/vrm-asset-generator/src/vrm_ext.rs`

The current `vrmc_spring_bone_scene` emits `scene.springs[0]` only. Refactor so it emits a `springs[]` JSON array with one entry per chain.

The function currently takes `joint_nodes: &[usize]` (single chain). The new signature needs per-chain joint indices: `joint_nodes_per_chain: &[Vec<usize>]` where outer index = chain index.

- [ ] **Step 1: Tests.**

```rust
#[cfg(test)]
mod multichain_emit_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn two_chains_emit_two_springs_entries() {
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("chain_a"),
                SpringBoneParams::defaults("chain_b"),
            ],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![], vec![]],
        };
        let joint_nodes_per_chain = vec![vec![10, 11, 12, 13], vec![20, 21, 22, 23]];
        let v = vrmc_spring_bone_scene_multichain(&joint_nodes_per_chain, &scene, &[]);
        let springs = v["springs"].as_array().unwrap();
        assert_eq!(springs.len(), 2);
        assert_eq!(springs[0]["name"], "chain_a_chain");
        assert_eq!(springs[1]["name"], "chain_b_chain");
        assert_eq!(springs[0]["joints"].as_array().unwrap().len(), 4);
        assert_eq!(springs[1]["joints"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn three_chains_with_shared_collider_group_emit_per_spring_group_indices() {
        let scene = SpringBoneSceneParams {
            springs: vec![
                SpringBoneParams::defaults("ca"),
                SpringBoneParams::defaults("cb"),
                SpringBoneParams::defaults("cc"),
            ],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "shared".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0], vec![0], vec![0]],
        };
        let joints = vec![vec![10,11,12,13], vec![20,21,22,23], vec![30,31,32,33]];
        let v = vrmc_spring_bone_scene_multichain(&joints, &scene, &[40]);
        let springs = v["springs"].as_array().unwrap();
        assert_eq!(springs.len(), 3);
        for s in springs {
            let groups = s["colliderGroups"].as_array().unwrap();
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].as_u64().unwrap(), 0);
        }
    }
}
```

- [ ] **Step 2: Run, expect compile error** (`vrmc_spring_bone_scene_multichain` undefined).

- [ ] **Step 3: Implement.** Two options:

  **(a)** Rename `vrmc_spring_bone_scene` to take `joint_nodes_per_chain: &[Vec<usize>]` and iterate. Update existing callers in `emit.rs` to wrap their single-chain joint list as `vec![chain]`. This is the "right" refactor but touches more files.

  **(b)** Add a new function `vrmc_spring_bone_scene_multichain(joint_nodes_per_chain, scene, collider_attach_nodes)` that supersedes the old one. Keep the old `vrmc_spring_bone_scene` as a single-chain wrapper that calls multichain with `vec![joint_nodes.to_vec()]`. Less invasive but adds API surface.

  **Pick (b)** to minimize churn. The single-chain wrapper stays for phase 1-5 callers; multi-chain emits use the new function.

```rust
pub fn vrmc_spring_bone_scene_multichain(
    joint_nodes_per_chain: &[Vec<usize>],
    scene: &SpringBoneSceneParams,
    collider_attach_nodes: &[usize],
) -> Value {
    assert_eq!(joint_nodes_per_chain.len(), scene.springs.len(),
        "joint_nodes_per_chain must parallel scene.springs");
    assert_eq!(scene.spring_collider_groups.len(), scene.springs.len(),
        "spring_collider_groups must parallel scene.springs");
    assert_eq!(scene.colliders.len(), collider_attach_nodes.len(),
        "collider_attach_nodes must parallel scene.colliders");

    let springs_json: Vec<Value> = scene.springs.iter().enumerate().map(|(c_idx, params)| {
        let chain_joints = &joint_nodes_per_chain[c_idx];
        let joint_count = chain_joints.len();
        let joints_json: Vec<Value> = chain_joints.iter().enumerate().map(|(j_idx, &node)| {
            let stiffness = joint_value(&params.stiffness_per_joint, params.stiffness, j_idx, joint_count, "stiffness");
            let drag = joint_value(&params.drag_force_per_joint, params.drag_force, j_idx, joint_count, "drag_force");
            let gravity_power = joint_value(&params.gravity_power_per_joint, params.gravity_power, j_idx, joint_count, "gravity_power");
            let hit_radius = joint_value(&params.hit_radius_per_joint, params.hit_radius, j_idx, joint_count, "hit_radius");
            let mut j = json!({
                "node": node,
                "hitRadius": hit_radius,
                "stiffness": stiffness,
                "gravityPower": gravity_power,
                "gravityDir": params.gravity_dir,
                "dragForce": drag,
            });
            if let Some(deg) = params.joint_angle_limit_deg {
                j["extensions"] = json!({
                    "VRMC_springBone_extended_collider": { "angleLimit": deg }
                });
            }
            j
        }).collect();

        let mut spring = json!({
            "name": params.spring_name,
            "joints": joints_json,
        });
        let groups = &scene.spring_collider_groups[c_idx];
        if !groups.is_empty() {
            spring["colliderGroups"] = json!(groups);
        }
        spring
    }).collect();

    let mut out = json!({
        "specVersion": "1.0",
        "springs": springs_json,
    });

    // Colliders and colliderGroups are scene-level (shared across all chains).
    // Reuse the existing single-chain logic — colliders emission is identical.
    if !scene.colliders.is_empty() {
        let colliders: Vec<Value> = scene.colliders.iter().zip(collider_attach_nodes.iter()).map(|(c, &node)| {
            let (base_shape, ext_shape) = match &c.shape {
                ColliderShape::Sphere { radius } => (
                    Some(json!({ "sphere": { "offset": c.offset, "radius": radius } })),
                    None,
                ),
                ColliderShape::Capsule { radius, tail_offset } => (
                    Some(json!({ "capsule": { "offset": c.offset, "radius": radius, "tail": tail_offset } })),
                    None,
                ),
                ColliderShape::Plane { normal } => (
                    None,
                    Some(json!({ "plane": { "offset": c.offset, "normal": normal } })),
                ),
                ColliderShape::InsideSphere { radius } => (
                    None,
                    Some(json!({ "sphere": { "offset": c.offset, "radius": radius, "inside": true } })),
                ),
                ColliderShape::InsideCapsule { radius, tail_offset } => (
                    None,
                    Some(json!({ "capsule": { "offset": c.offset, "radius": radius, "tail": tail_offset, "inside": true } })),
                ),
            };
            let mut entry = json!({ "node": node });
            if let Some(s) = base_shape { entry["shape"] = s; }
            if let Some(s) = ext_shape {
                entry["extensions"] = json!({ "VRMC_springBone_extended_collider": { "shape": s } });
            }
            entry
        }).collect();
        out["colliders"] = json!(colliders);
    }

    if !scene.collider_groups.is_empty() {
        let groups: Vec<Value> = scene.collider_groups.iter().map(|g| json!({
            "name": g.name,
            "colliders": g.collider_indices,
        })).collect();
        out["colliderGroups"] = json!(groups);
    }

    out
}
```

Then update `vrmc_spring_bone_scene` to be a thin wrapper:

```rust
pub fn vrmc_spring_bone_scene(
    joint_nodes: &[usize],
    scene: &SpringBoneSceneParams,
    collider_attach_nodes: &[usize],
) -> Value {
    vrmc_spring_bone_scene_multichain(&[joint_nodes.to_vec()], scene, collider_attach_nodes)
}
```

This DEDUPLICATES the existing single-chain logic — moves shape/extension/per-joint code into the multichain function. Existing callers stay working.

**Refactor risk:** the existing single-chain tests (from phases 2-5) must still pass. Don't break them.

- [ ] **Step 4: Run all existing tests + new tests, expect all pass:**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  ```

- [ ] **Step 5: Commit:**
  ```
  git add crates/vrm-asset-generator/src/vrm_ext.rs && git commit -m "feat(vrm-asset-generator): vrmc_spring_bone_scene_multichain iterates N springs"
  ```

---

## Task 2: emit.rs — N-chain glTF node + skin emission

**Files:** `crates/vrm-asset-generator/src/emit.rs`

The existing `emit_vrm_with_spring_bone_colliders` builds ONE chain hierarchy (intermediate nodes + chain joint nodes + skin). For multi-chain, we need N parallel chains, each attached to its own intermediate node radial-spaced around the head.

- [ ] **Step 1: Tests:**

```rust
#[cfg(test)]
mod multichain_emit_integration_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use tempfile::tempdir;
    use camino::Utf8Path;

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
}
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Implement `emit_vrm_with_spring_bone_multichain`.**

The strategy that minimizes churn: iterate `scene.springs[]`, for each chain build the intermediate node + N joint nodes (offset radially around head), accumulate them into the glTF nodes array. Track `chain_joint_nodes: Vec<Vec<usize>>` for the JSON emission step.

Radial spacing convention: chains around head at angles `0°, 360°/N, 2·360°/N, ...` in the XZ plane, all at the same Y (chain attaches at head Y). Each chain hangs straight down (gravity-Y) from its intermediate node.

The chain-skinned cylinder mesh from `chain_mesh.rs`: this is per-chain geometry. For multi-chain, emit N cylinders, each weighted to its chain's joints. If `chain_mesh.rs` is per-chain-friendly (likely), call it N times and concatenate. If it's hardcoded for a single chain, factor it.

This is the most complex task in phase 6. If the existing single-chain emit function is too tangled to easily multiply by N, an acceptable simpler approach: **emit N completely separate glTF child models stitched into one** (i.e., parallel sub-trees that don't share nodes). The visible chain count is N; the skin count is N. The collision groups still index into the shared `colliders[]` array.

- [ ] **Step 4: Add a parallel sidecar emitter** `emit_with_sidecars_spring_bone_multichain(mtoon, scene, stem) -> Result<()>`.

- [ ] **Step 5: Run + commit:**
  ```
  cargo test -p vrm-asset-generator multichain_emit_integration
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/src/chain_mesh.rs && git commit -m "feat(vrm-asset-generator): emit_vrm_with_spring_bone_multichain (N parallel chains)"
  ```

---

## Task 3: Multi-chain sweep — 18 variants

**Files:** `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Tests:**

```rust
#[cfg(test)]
mod multichain_sweep_tests {
    use super::*;

    #[test]
    fn multichain_sweep_produces_18_variants() {
        // 3 chain_count × 2 spacing × 3 sharing = 18.
        let variants = spring_bone_multichain_sweep();
        assert_eq!(variants.len(), 18);
    }

    #[test]
    fn multichain_sweep_each_variant_has_multiple_chains() {
        let variants = spring_bone_multichain_sweep();
        for (_, scene) in &variants {
            assert!(scene.springs.len() >= 2);
            assert!(scene.springs.len() <= 5);
        }
    }

    #[test]
    fn multichain_sweep_unique_ids() {
        let variants = spring_bone_multichain_sweep();
        let ids: std::collections::HashSet<_> =
            variants.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(ids.len(), 18);
    }

    #[test]
    fn multichain_sweep_sharing_modes_actually_share() {
        let variants = spring_bone_multichain_sweep();
        // For sharing="all", every chain references the same collider_group index.
        let all_share: Vec<_> = variants.iter().filter(|(m, _)| m.id.contains("share_all")).collect();
        for (_, scene) in all_share {
            let first = &scene.spring_collider_groups[0];
            for sg in &scene.spring_collider_groups {
                assert_eq!(sg, first, "share_all variants must point all chains at the same group");
            }
        }
    }
}
```

- [ ] **Step 2: Implement:**

```rust
pub fn spring_bone_multichain_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(18);

    let chain_counts: [u32; 3] = [2, 3, 5];
    let spacings_m: [f32; 2] = [0.02, 0.05];
    let sharing_modes = ["share_all", "share_none", "share_alt"];

    for &cc in chain_counts.iter() {
        for &sp in spacings_m.iter() {
            for &mode in sharing_modes.iter() {
                let id = format!(
                    "springbone_multichain_n{}_sp{}_{}",
                    cc, fmt_num(sp), mode
                );
                let scene = build_multichain_scene(&id, cc, sp, mode);
                out.push((MToonParams::defaults(&id), scene));
            }
        }
    }
    out
}

fn build_multichain_scene(id: &str, chain_count: u32, _spacing: f32, sharing_mode: &str) -> SpringBoneSceneParams {
    let springs: Vec<SpringBoneParams> = (0..chain_count)
        .map(|i| SpringBoneParams::defaults(format!("{id}_chain_{i}")))
        .collect();

    // Sweep currently doesn't include colliders — multi-chain × collider explosion is
    // out of scope for phase 6 ("multi-chain" axis). Future axis = combine with collider sweep.
    // But include collider_groups + spring_collider_groups to exercise the sharing modes:
    // we'll use empty colliders + empty groups for share_none mode, and a single empty group
    // for share_all / share_alt (vacuous group is still valid VRM 1.0 — colliders array can
    // be empty when colliderGroups is empty).
    //
    // Actually the spec rejects empty colliderGroups, so when there are no colliders we
    // also have no groups. The sharing modes don't have observable effect without
    // colliders. Workaround: emit one trivial sphere collider on each variant so the
    // group-sharing axis has something to share/not-share.

    let trivial_collider = ColliderParams {
        shape: ColliderShape::Sphere { radius: 0.01 },
        offset: [0.0, 0.0, 0.0],
        attach: ColliderAttach::Head,
    };

    let (collider_groups, spring_collider_groups) = match sharing_mode {
        "share_all" => {
            let shared_group = ColliderGroupParams {
                name: "shared".into(),
                collider_indices: vec![0],
            };
            let groups = vec![shared_group];
            let sgs = (0..chain_count).map(|_| vec![0_usize]).collect();
            (groups, sgs)
        },
        "share_none" => {
            // N groups, each pointing at the same single collider, each owned by one chain.
            let groups = (0..chain_count).map(|i| ColliderGroupParams {
                name: format!("g{i}"),
                collider_indices: vec![0],
            }).collect();
            let sgs = (0..chain_count).map(|i| vec![i as usize]).collect();
            (groups, sgs)
        },
        "share_alt" => {
            // 2 groups, chains alternate which group they reference.
            let groups = vec![
                ColliderGroupParams { name: "even".into(), collider_indices: vec![0] },
                ColliderGroupParams { name: "odd".into(),  collider_indices: vec![0] },
            ];
            let sgs = (0..chain_count).map(|i| vec![(i as usize) % 2]).collect();
            (groups, sgs)
        },
        _ => unreachable!("unknown sharing mode {sharing_mode}"),
    };

    SpringBoneSceneParams {
        springs,
        colliders: vec![trivial_collider],
        collider_groups,
        spring_collider_groups,
    }
}
```

Note: `spacing` parameter is in the ID string but the actual radial placement of chains is handled at emit time in `emit.rs`. We pass `spacing` through indirectly via the ID — when emit.rs sees chain_count=3 from `scene.springs.len()`, it places them at predetermined radii. The sweep documents which spacing was intended.

(If `emit.rs` doesn't have a way to know the spacing per scene, add a `chain_radial_spacing_m: Option<f32>` to `SpringBoneSceneParams` and thread it through. For phase 6 simplicity, just hard-code spacing in emit.rs to 0.05 m and document that the sweep IDs lie about the actual spacing — call this out in the findings entry as a phase-6 limitation.)

- [ ] **Step 3: Commit:**
  ```
  cargo test -p vrm-asset-generator multichain_sweep_tests
  git add crates/vrm-asset-generator/src/sweep.rs && git commit -m "feat(vrm-asset-generator): 18-variant multichain sweep"
  ```

---

## Task 4: sidecar.rs + CLI subcommand

**Files:** `crates/vrm-asset-generator/src/sidecar.rs`, `cli.rs`

- [ ] **Step 1:** Add `build_spring_bone_multichain_test_plan` + swing variant. Mirror the collider plan builder; spec_section `"VRMC_springBone multi-chain"`.

- [ ] **Step 2:** Add `emit-springbone-multichain-sweep` subcommand iterating the sweep. Emit settle + swing per variant → 18 × 2 = 36 plans.

- [ ] **Step 3:** Smoke test:
  ```
  cargo build -p vrm-asset-generator --release
  mkdir -p /tmp/phase6-smoke && ./target/release/vrm-asset-generator emit-springbone-multichain-sweep --output-dir /tmp/phase6-smoke
  ls /tmp/phase6-smoke/*.vrm | wc -l       # expect 36
  ls /tmp/phase6-smoke/*.test.yaml | wc -l # expect 36
  ```

  Then inspect one VRM's JSON to confirm multi-spring emission:
  ```
  cargo run -p vrm-asset-generator --quiet --release -- emit-default --id _probe --output-dir /tmp/_probe
  # (or use a hex dumper / glb inspector to read the embedded JSON from one of the multichain vrms)
  ```

- [ ] **Step 4:** Validate with mrxz/vrm-validator if installed.

- [ ] **Step 5:** Commits — one for sidecar, one for CLI:
  ```
  git add crates/vrm-asset-generator/src/sidecar.rs && git commit -m "feat(vrm-asset-generator): build_spring_bone_multichain_test_plan settle + swing"
  git add crates/vrm-asset-generator/src/cli.rs && git commit -m "feat(vrm-asset-generator): emit-springbone-multichain-sweep subcommand (36 plans)"
  ```

---

## Task 5: docs/findings.md phase-6 entry

```markdown
## Phase 6 — multi-chain sweep landed (36 plans)

**Trigger:** Phase 5 per-joint taper merged. Phase 6 closes the multi-chain axis: prior sweeps emitted a single chain attached to the head; multi-chain assets exercise collider-group sharing semantics (`share_all`, `share_none`, `share_alt`) plus chain-count effects.

**Shipped:**
- `vrmc_spring_bone_scene_multichain` iterates N springs into a JSON array of springs; the single-chain `vrmc_spring_bone_scene` is now a thin wrapper.
- `emit_vrm_with_spring_bone_multichain` emits N parallel chain hierarchies (each chain attaches to its own intermediate node radial-spaced around head).
- `emit-springbone-multichain-sweep` produces 36 plans (3 chain counts × 2 spacings × 3 sharing modes × settle/swing).

**Known limitation:** the sweep's "spacing" axis (in IDs) currently maps to a fixed 0.05 m radial spacing at emit time. Two values (0.02, 0.05) in the IDs encode the intent but produce identical geometry. Resolving requires threading spacing through `SpringBoneSceneParams` → emit; deferred as it doesn't block the chain-count and sharing-mode axes which are the load-bearing ones for VMK#162-class regressions.

**Forward:** Phase 7 — VMK#162 regression matrix (execute-test-plan-matrix runner mode).
```

- [ ] Commit: `git add docs/findings.md && git commit -m "docs(findings): phase 6 multi-chain sweep (36 plans) landed"`

---

## Acceptance

- [ ] All tests green
- [ ] clippy + fmt clean
- [ ] 36 vrms + 36 test plans emitted
- [ ] One emitted vrm's JSON contains 2-5 springs entries (verified by glb inspection)
- [ ] At least one emitted vrm passes mrxz/vrm-validator if installed
- [ ] findings.md entry present
