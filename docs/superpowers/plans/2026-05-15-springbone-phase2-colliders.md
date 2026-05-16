# VRMC_springBone Phase 2 — Colliders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development.

**Goal:** Land `VRMC_springBone` collider emission (sphere + capsule) in the asset generator and ship a 24-variant Cartesian sweep + 24 swing variants = 48 plans where chain-skinned cylinders deflect off colliders so cross-renderer divergence shows up in pixels and (via phase 1's `dump_bone_positions`) in joint positions.

**Architecture:** New types in `spring_bone.rs` (ColliderShape, ColliderAttach, ColliderParams, ColliderGroupParams, SpringBoneSceneParams). `vrm_ext.rs::vrmc_spring_bone()` learns to optionally emit `colliders`, `colliderGroups`, and per-spring `colliderGroups`. `emit.rs` gains `emit_vrm_with_spring_bone_colliders()` that places a sphere or capsule collider in the chain's path. `sweep.rs` adds `spring_bone_collider_sweep()`. `sidecar.rs` generates matching settle + swing test plans.

**Tech Stack:** Rust 1.88, `serde_json`, the existing `chain_mesh.rs` skinning infrastructure.

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §4.

**Out of scope (deferred):**
- `avatarA_bosom_collider` humanoid plan — requires authoring `avatarA_collider_1_0.vrm` in Blender; not code work. Document as phase-2 follow-up in `docs/findings.md`.
- Extended colliders (planes, inverted sphere/capsule, angle limits) — phase 3.

---

## File map

**Modify:**
- `crates/vrm-asset-generator/src/spring_bone.rs` — new types
- `crates/vrm-asset-generator/src/vrm_ext.rs` — `vrmc_spring_bone()` accepts collider data
- `crates/vrm-asset-generator/src/emit.rs` — new emit function with collider nodes
- `crates/vrm-asset-generator/src/sweep.rs` — `spring_bone_collider_sweep()` (24 variants)
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_spring_bone_collider_test_plan()` settle + swing
- `crates/vrm-asset-generator/src/cli.rs` — `emit-springbone-collider-sweep` subcommand

---

## Task 1: Collider types in spring_bone.rs

**Files:** `crates/vrm-asset-generator/src/spring_bone.rs` (inline tests)

- [ ] **Step 1: Write failing tests.** Append to `spring_bone.rs`:

```rust
#[cfg(test)]
mod collider_tests {
    use super::*;

    #[test]
    fn sphere_collider_default_is_at_origin_with_unit_radius() {
        let c = ColliderParams {
            shape: ColliderShape::Sphere { radius: 0.05 },
            offset: [0.0, 0.0, 0.0],
            attach: ColliderAttach::Head,
        };
        match c.shape {
            ColliderShape::Sphere { radius } => assert!((radius - 0.05).abs() < 1e-6),
            _ => panic!("expected sphere"),
        }
    }

    #[test]
    fn capsule_collider_has_tail_offset() {
        let c = ColliderParams {
            shape: ColliderShape::Capsule {
                radius: 0.03,
                tail_offset: [0.0, -0.1, 0.0],
            },
            offset: [0.0, 0.0, 0.0],
            attach: ColliderAttach::Head,
        };
        match c.shape {
            ColliderShape::Capsule { tail_offset, .. } => {
                assert_eq!(tail_offset, [0.0, -0.1, 0.0]);
            }
            _ => panic!("expected capsule"),
        }
    }

    #[test]
    fn collider_group_holds_indices() {
        let g = ColliderGroupParams {
            name: "head_colliders".into(),
            collider_indices: vec![0, 2, 3],
        };
        assert_eq!(g.collider_indices.len(), 3);
    }

    #[test]
    fn scene_params_aggregates_springs_and_colliders() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test_chain")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g0".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        assert_eq!(scene.springs.len(), 1);
        assert_eq!(scene.colliders.len(), 1);
        assert_eq!(scene.collider_groups.len(), 1);
        assert_eq!(scene.spring_collider_groups[0], vec![0]);
    }
}
```

- [ ] **Step 2: Run test, expect failure.**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator collider_tests
  ```

- [ ] **Step 3: Add the types** after `SpringBoneParams` in `spring_bone.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Capsule { radius: f32, tail_offset: [f32; 3] },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderAttach {
    Head,
    NewIntermediateNode { y_offset: f32, z_offset: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColliderParams {
    pub shape: ColliderShape,
    pub offset: [f32; 3],
    pub attach: ColliderAttach,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColliderGroupParams {
    pub name: String,
    pub collider_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringBoneSceneParams {
    /// `Vec` so multi-chain (phase 6) plugs in here without API churn.
    pub springs: Vec<SpringBoneParams>,
    pub colliders: Vec<ColliderParams>,
    pub collider_groups: Vec<ColliderGroupParams>,
    /// Per-spring index list into `collider_groups`.
    pub spring_collider_groups: Vec<Vec<usize>>,
}

impl SpringBoneSceneParams {
    /// Single-chain, no-collider scene constructed from a SpringBoneParams.
    /// Backward-compat for callers that don't need colliders.
    pub fn single_spring(s: SpringBoneParams) -> Self {
        Self {
            springs: vec![s],
            colliders: Vec::new(),
            collider_groups: Vec::new(),
            spring_collider_groups: vec![Vec::new()],
        }
    }
}
```

- [ ] **Step 4: Run + lint + commit.**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator collider_tests
  cd /Users/arkavo/Projects/vrm-conformance && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cd /Users/arkavo/Projects/vrm-conformance && cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/spring_bone.rs && git commit -m "feat(vrm-asset-generator): collider types for VRMC_springBone"
  ```

---

## Task 2: vrm_ext.rs emits colliders + colliderGroups

**Files:** `crates/vrm-asset-generator/src/vrm_ext.rs`

- [ ] **Step 1: Read existing `vrmc_spring_bone()`** at `vrm_ext.rs:133`. It currently emits only `springs[].joints[]`. We're extending the same function (NOT adding a sibling — backward compat via `SpringBoneSceneParams::single_spring`).

- [ ] **Step 2: Write failing test in vrm_ext.rs:**

```rust
#[cfg(test)]
mod collider_emission_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn no_colliders_omitted_from_json() {
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("c"));
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[]);
        assert!(v.get("colliders").is_none());
        assert!(v.get("colliderGroups").is_none());
        assert!(v.get("springs").unwrap().as_array().unwrap()[0].get("colliderGroups").is_none());
    }

    #[test]
    fn sphere_collider_emits_correct_json_shape() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Sphere { radius: 0.05 },
                offset: [0.0, -0.04, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g0".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        // attach_nodes parallels colliders by index — for Head attach we pass the head node idx.
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]); // 10 = head node idx
        let colliders = v.get("colliders").unwrap().as_array().unwrap();
        assert_eq!(colliders.len(), 1);
        let c0 = &colliders[0];
        assert_eq!(c0["node"].as_u64().unwrap(), 10);
        let shape = c0["shape"].as_object().unwrap();
        assert!(shape.contains_key("sphere"), "expected sphere shape, got {shape:?}");
        let sphere = &shape["sphere"];
        assert!((sphere["radius"].as_f64().unwrap() - 0.05).abs() < 1e-6);
        let off = sphere["offset"].as_array().unwrap();
        assert_eq!(off.len(), 3);
        assert!((off[1].as_f64().unwrap() - (-0.04)).abs() < 1e-6);

        let groups = v.get("colliderGroups").unwrap().as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["name"], "g0");
        assert_eq!(groups[0]["colliders"][0], 0);

        let spring = &v["springs"].as_array().unwrap()[0];
        let cg = spring["colliderGroups"].as_array().unwrap();
        assert_eq!(cg.len(), 1);
        assert_eq!(cg[0].as_u64().unwrap(), 0);
    }

    #[test]
    fn capsule_collider_emits_tail_field() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Capsule {
                    radius: 0.03,
                    tail_offset: [0.0, -0.08, 0.0],
                },
                offset: [0.0, 0.0, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams {
                name: "g0".into(),
                collider_indices: vec![0],
            }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0, 1, 2, 3], &scene, &[10]);
        let shape = v["colliders"][0]["shape"].as_object().unwrap();
        assert!(shape.contains_key("capsule"));
        let cap = &shape["capsule"];
        let tail = cap["tail"].as_array().unwrap();
        assert!((tail[1].as_f64().unwrap() - (-0.08)).abs() < 1e-6);
    }
}
```

- [ ] **Step 3: Run, expect compile error** (`vrmc_spring_bone_scene` undefined).
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator collider_emission_tests
  ```

- [ ] **Step 4: Add `vrmc_spring_bone_scene` function** in `vrm_ext.rs`. Keep the existing `vrmc_spring_bone(joint_nodes, params)` as a thin wrapper that delegates to the new scene-form with `SpringBoneSceneParams::single_spring()` — preserves callers in `emit.rs`. New function:

```rust
/// Emit VRMC_springBone extension JSON for a scene with optional colliders.
///
/// `joint_nodes` is in chain order, head-to-tail (phase 1 single-chain only;
/// phase 6 multi-chain will accept Vec<Vec<usize>>).
///
/// `collider_attach_nodes[i]` is the glTF node index that collider `i` is
/// attached to. The caller resolves Head / NewIntermediateNode → node index
/// during emit.
pub fn vrmc_spring_bone_scene(
    joint_nodes: &[usize],
    scene: &SpringBoneSceneParams,
    collider_attach_nodes: &[usize],
) -> Value {
    assert_eq!(
        scene.colliders.len(),
        collider_attach_nodes.len(),
        "collider_attach_nodes must parallel scene.colliders"
    );

    // Phase 1: single spring (scene.springs[0]) for now.
    // Phase 6 will iterate scene.springs.
    let params = &scene.springs[0];
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

    let mut spring = json!({
        "name": params.spring_name,
        "joints": joints,
    });

    let spring_groups = scene.spring_collider_groups.first().cloned().unwrap_or_default();
    if !spring_groups.is_empty() {
        spring["colliderGroups"] = json!(spring_groups);
    }

    let mut out = json!({
        "specVersion": "1.0",
        "springs": [spring],
    });

    if !scene.colliders.is_empty() {
        let colliders: Vec<Value> = scene.colliders.iter().zip(collider_attach_nodes.iter()).map(|(c, &node)| {
            let shape = match &c.shape {
                ColliderShape::Sphere { radius } => json!({
                    "sphere": {
                        "offset": c.offset,
                        "radius": radius,
                    }
                }),
                ColliderShape::Capsule { radius, tail_offset } => json!({
                    "capsule": {
                        "offset": c.offset,
                        "radius": radius,
                        "tail": tail_offset,
                    }
                }),
            };
            json!({
                "node": node,
                "shape": shape,
            })
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

Update the existing `vrmc_spring_bone()` to delegate:

```rust
pub fn vrmc_spring_bone(joint_nodes: &[usize], params: &SpringBoneParams) -> Value {
    let scene = SpringBoneSceneParams::single_spring(params.clone());
    vrmc_spring_bone_scene(joint_nodes, &scene, &[])
}
```

This keeps `emit.rs` callers working unchanged.

- [ ] **Step 5: Run tests + lint + commit.**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator
  cd /Users/arkavo/Projects/vrm-conformance && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cd /Users/arkavo/Projects/vrm-conformance && cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/vrm_ext.rs && git commit -m "feat(vrm-asset-generator): VRMC_springBone collider + colliderGroup emission"
  ```

---

## Task 3: emit.rs — new emit function with colliders

**Files:** `crates/vrm-asset-generator/src/emit.rs`

- [ ] **Step 1: Read `emit_vrm_with_spring_bone`** at `emit.rs:134`. Understand:
  - How chain joints are computed (Y positions below head).
  - How `chain_top_y` and `inv_bind` are derived.
  - How the existing chain-skinned cylinder mesh (or sphere mesh) is packed into the glTF.

- [ ] **Step 2: Write failing integration test** in `emit.rs`:

```rust
#[cfg(test)]
mod collider_emit_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use tempfile::tempdir;
    use camino::Utf8Path;

    #[test]
    fn emit_with_sphere_collider_produces_loadable_glb_with_collider_json() {
        let mtoon = MToonParams::defaults("test_collider");
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
            vrmc["springs"][0]["colliderGroups"].as_array().unwrap().len(),
            1
        );
    }
}
```

If `crate::glb::extract_json_chunk` doesn't exist, add a small helper or inspect the bytes directly with `read_glb_json` from existing tests (look for any test that already reads back a GLB).

- [ ] **Step 3: Run, expect failure.**

- [ ] **Step 4: Implement `emit_vrm_with_spring_bone_colliders`** as a parallel to `emit_vrm_with_spring_bone`. Differences:
  - Takes `&SpringBoneSceneParams` instead of `&SpringBoneParams`.
  - Resolves `ColliderAttach::Head` → the head node index (same node the chain attaches under in the existing path).
  - Resolves `ColliderAttach::NewIntermediateNode { y_offset, z_offset }` → emit a new node parented under head at that offset, return its index.
  - Builds `collider_attach_nodes: Vec<usize>` parallel to `scene.colliders`.
  - Calls `vrmc_spring_bone_scene(&joint_nodes, scene, &collider_attach_nodes)` instead of `vrmc_spring_bone`.

Inside `emit.rs`, factor out shared chain-setup code so the new function isn't a 200-line copy-paste of the existing one. If the existing function is too entangled to factor cleanly, accept the duplication for phase 2 and flag in a comment "factor shared chain setup in phase 6 multi-chain refactor".

Also expose a parallel `emit_with_sidecars_spring_bone_colliders(mtoon, scene, stem)` that follows the same pattern as `emit_with_sidecars_spring_bone`.

- [ ] **Step 5: Run tests + lint + commit.**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator collider_emit
  cd /Users/arkavo/Projects/vrm-conformance && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cd /Users/arkavo/Projects/vrm-conformance && cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/emit.rs && git commit -m "feat(vrm-asset-generator): emit_vrm_with_spring_bone_colliders + sidecar wrapper"
  ```

---

## Task 4: sweep.rs — 24-variant Cartesian sweep

**Files:** `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Write failing test:**

```rust
#[cfg(test)]
mod collider_sweep_tests {
    use super::*;

    #[test]
    fn collider_sweep_produces_24_variants() {
        let variants = spring_bone_collider_sweep();
        assert_eq!(variants.len(), 24,
            "Cartesian: 2 shapes × 4 offsets × 3 radii = 24");
    }

    #[test]
    fn collider_sweep_variants_are_uniquely_named() {
        let variants = spring_bone_collider_sweep();
        let names: std::collections::HashSet<_> =
            variants.iter().map(|(mtoon, _scene)| mtoon.id.clone()).collect();
        assert_eq!(names.len(), 24, "all variant IDs must be unique");
    }

    #[test]
    fn collider_sweep_uses_default_mtoon_constant_across_variants() {
        let variants = spring_bone_collider_sweep();
        let baseline_color = variants[0].0.base_color;
        for (m, _) in &variants {
            assert_eq!(m.base_color, baseline_color,
                "MToon must be held constant across collider sweep");
        }
    }

    #[test]
    fn collider_sweep_each_variant_has_exactly_one_collider_group_per_spring() {
        let variants = spring_bone_collider_sweep();
        for (id, scene) in variants.iter().map(|(m, s)| (m.id.clone(), s)) {
            assert_eq!(scene.colliders.len(), 1, "{id}: expected 1 collider");
            assert_eq!(scene.collider_groups.len(), 1, "{id}: expected 1 group");
            assert_eq!(scene.spring_collider_groups[0], vec![0], "{id}: spring must reference group 0");
        }
    }
}
```

- [ ] **Step 2: Implement `spring_bone_collider_sweep()`** returning `Vec<(MToonParams, SpringBoneSceneParams)>` since each variant needs both the MToon constant and the spring scene:

```rust
use crate::params::MToonParams;
use crate::spring_bone::{
    ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape, SpringBoneParams,
    SpringBoneSceneParams,
};

pub fn spring_bone_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(24);

    // Cartesian: 2 shapes × 4 offsets_y × 3 radii.
    // Shapes hit chain at different geometric profiles.
    // offset_y values: chain hangs from y_head=1.36 downward, segment 0.05 × 4 joints = 0.20 m.
    // So offsets are taken relative to chain root (head_local). Negative Y = below head, in chain path.
    let offsets = [-0.08_f32, -0.04, 0.0, 0.04];
    let radii = [0.03_f32, 0.05, 0.10];

    for shape_kind in ["sphere", "capsule"].iter() {
        for &off_y in offsets.iter() {
            for &radius in radii.iter() {
                let id = format!(
                    "springbone_collider_{}_y{}_r{}",
                    shape_kind,
                    fmt_signed(off_y),
                    fmt_num(radius),
                );
                let shape = match *shape_kind {
                    "sphere" => ColliderShape::Sphere { radius },
                    "capsule" => ColliderShape::Capsule {
                        radius,
                        tail_offset: [0.0, -0.05, 0.0],
                    },
                    _ => unreachable!(),
                };
                let collider = ColliderParams {
                    shape,
                    offset: [0.0, off_y, 0.0],
                    attach: ColliderAttach::Head,
                };
                let scene = SpringBoneSceneParams {
                    springs: vec![SpringBoneParams::defaults(&id)],
                    colliders: vec![collider],
                    collider_groups: vec![ColliderGroupParams {
                        name: "head_g".into(),
                        collider_indices: vec![0],
                    }],
                    spring_collider_groups: vec![vec![0]],
                };
                let mtoon = MToonParams::defaults(&id);
                out.push((mtoon, scene));
            }
        }
    }

    out
}

fn fmt_signed(v: f32) -> String {
    if v < 0.0 {
        format!("neg{}", fmt_num(-v))
    } else {
        fmt_num(v)
    }
}
```

Reuse `fmt_num` from the existing spring_bone sweep (or import it from `spring_bone.rs`).

- [ ] **Step 3: Run + lint + commit.**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator collider_sweep
  cd /Users/arkavo/Projects/vrm-conformance && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  git add crates/vrm-asset-generator/src/sweep.rs && git commit -m "feat(vrm-asset-generator): 24-variant Cartesian collider sweep"
  ```

---

## Task 5: sidecar.rs — collider test plan builder (settle + swing)

**Files:** `crates/vrm-asset-generator/src/sidecar.rs`

- [ ] **Step 1: Write failing test.**

```rust
#[cfg(test)]
mod collider_plan_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn collider_plan_settle_has_60_settle_steps() {
        let mtoon = MToonParams::defaults("test_collider_settle");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![]],
        };
        let plan = build_spring_bone_collider_test_plan(&mtoon, &scene, "out.vrm");
        let physics = plan.physics.expect("plan must carry physics config");
        assert_eq!(physics.settle_steps, 60, "collider tests use 60-step settle (slower convergence under contact)");
        assert!(plan.animation.is_none(), "settle plan has no animation block");
    }

    #[test]
    fn collider_plan_swing_carries_animation_block() {
        let mtoon = MToonParams::defaults("test_collider_swing");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![],
            collider_groups: vec![],
            spring_collider_groups: vec![vec![]],
        };
        let plan = build_spring_bone_collider_swing_test_plan(&mtoon, &scene, "out.vrm");
        assert!(plan.animation.is_some());
        let anim = plan.animation.unwrap();
        let root = anim.root_transform.unwrap();
        assert!((root.duration_seconds - 0.25).abs() < 1e-6);
        assert_eq!(root.fps, 60);
    }
}
```

- [ ] **Step 2: Implement `build_spring_bone_collider_test_plan` and `_swing` variants.** Mirror existing `build_spring_bone_test_plan` and `build_spring_bone_swing_test_plan` but with:
  - `spec_section: "VRMC_springBone + colliders"` (or similar)
  - `physics.settle_steps: 60` (vs 30 default — colliders settle slower under contact)
  - Camera framing tweaked to view the chain + collider region (the chain still hangs from head; framing should show the collision zone)

- [ ] **Step 3: Run + lint + commit.**
  ```
  git add crates/vrm-asset-generator/src/sidecar.rs && git commit -m "feat(vrm-asset-generator): build_spring_bone_collider_test_plan settle + swing"
  ```

---

## Task 6: CLI subcommand `emit-springbone-collider-sweep`

**Files:** `crates/vrm-asset-generator/src/cli.rs`, `emit.rs` (sidecar emit helper)

- [ ] **Step 1: Read existing `emit-springbone-sweep` and `emit-springbone-swing-sweep` subcommand handlers.** Mirror their structure exactly. The new subcommand iterates `spring_bone_collider_sweep()` and for each pair `(mtoon, scene)` emits BOTH settle and swing variants — so 24 cartesian × 2 modes = 48 plans emitted.

- [ ] **Step 2: Add subcommand definition + dispatch.** Use the same `--output-dir` arg as existing sweeps. Include the new subcommand in the describe catalog if there's an explicit list.

- [ ] **Step 3: Smoke test by running it:**

```
cd /Users/arkavo/Projects/vrm-conformance
cargo build -p vrm-asset-generator --release
./target/release/vrm-asset-generator emit-springbone-collider-sweep --output-dir /tmp/collider-sweep
ls /tmp/collider-sweep/*.vrm | wc -l
# Expect 48
ls /tmp/collider-sweep/*.test.yaml | wc -l
# Expect 48
```

- [ ] **Step 4: Validate one of the emitted VRMs with the validator** if available:

```
ls .tools/vrm-validator-cli 2>/dev/null && \
  .tools/vrm-validator-cli /tmp/collider-sweep/springbone_collider_sphere_y0_r0p03_settle.vrm
```

If the validator is installed (it's gated behind `.tools/`), it should pass cleanly. If it errors on a real schema issue, fix the emission. If the validator isn't installed locally, skip this step and document.

- [ ] **Step 5: Commit.**
  ```
  git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/src/emit.rs && \
    git commit -m "feat(vrm-asset-generator): emit-springbone-collider-sweep subcommand (48 plans)"
  ```

---

## Task 7: Document phase-2 follow-up (humanoid plan + chain-skinned mesh confirm)

**Files:** `docs/findings.md`

- [ ] **Step 1:** Read the existing structure of `docs/findings.md` (it has dated trigger/finding sections). Add a phase-2 entry noting:
  1. Synthetic collider sweep (48 plans) lands in this branch.
  2. Humanoid `avatarA_bosom_collider` plan is deferred pending Blender authoring of `avatarA_collider_1_0.vrm` (one-off, ~half a day).
  3. The chain-skinned cylinder pattern from earlier findings runs (chain_mesh.rs is active per past run-3 findings).
  4. Next phase: extended_colliders.

```markdown
## Phase 2 — VRMC_springBone collider sweep landed (synthetic only)

**Trigger:** Phase 1 infrastructure (dump_bone_positions across four adapters, position-diff math, manifest + runner integration) merged. Phase 2 of the seven-phase springbone gap closure design adds collider emission to the asset generator and 48 test plans (24 Cartesian variants × settle/swing).

**Shipped:**
- Generator types: `ColliderShape::{Sphere, Capsule}`, `ColliderAttach`, `ColliderParams`, `ColliderGroupParams`, `SpringBoneSceneParams`.
- `vrm_ext.rs::vrmc_spring_bone_scene()` emits `colliders[]`, `colliderGroups[]`, per-spring `colliderGroups`.
- `emit-springbone-collider-sweep` subcommand → 48 `.vrm` + `.test.yaml` + `.meta.json` triplets.
- Sweep axes: shape (sphere, capsule), offset_y (-0.08, -0.04, 0, +0.04), radius (0.03, 0.05, 0.10). Cartesian, not one-axis-at-a-time, because collision response isn't separable on a single axis at this scale.

**Deferred:**
- `avatarA_bosom_collider` humanoid plan — requires authoring `avatarA_collider_1_0.vrm` in Blender (one head-mounted sphere collider intersecting the existing bust chain swing path). Estimated half-day of authoring; not code work. The 48-plan synthetic sweep is independent and ships now.
- The collider sweep currently does not run through `bootstrap-goldens.sh` — that's a separate task once renderers have rendered the new corpus at least once.

**Forward:** Phase 3 adds `VRMC_springBone_extended_collider` (planes, inverted sphere/capsule, joint angle limits).
```

- [ ] **Step 2: Commit.**
  ```
  git add docs/findings.md && git commit -m "docs(findings): phase 2 collider sweep landed (48 plans synthetic, humanoid deferred)"
  ```

---

## Final acceptance

- [ ] `cargo test --workspace` is green.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is green.
- [ ] `cargo fmt --all -- --check` is green.
- [ ] `target/release/vrm-asset-generator emit-springbone-collider-sweep --output-dir <dir>` produces 48 `.vrm` + 48 `.test.yaml` files.
- [ ] At least one of the emitted VRMs passes `mrxz/vrm-validator` (if installed locally) OR document that validator wasn't run.
- [ ] `docs/findings.md` has the phase-2 entry.

Once accepted, phase 3 (extended colliders) starts.
