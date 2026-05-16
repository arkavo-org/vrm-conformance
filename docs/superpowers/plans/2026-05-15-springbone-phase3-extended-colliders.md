# VRMC_springBone Phase 3 — Extended Colliders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Add `VRMC_springBone_extended_collider` support (planes + inverted sphere/capsule + joint angle limits) and a 36-plan sweep so renderers that don't implement the extension are flagged by cross-renderer diff.

**Architecture:** Three new `ColliderShape` variants (`Plane`, `InsideSphere`, `InsideCapsule`) emitted under `extensions.VRMC_springBone_extended_collider.shape` on each collider. `SpringBoneParams.joint_angle_limit_deg: Option<f32>` emitted under `extensions.VRMC_springBone_extended_collider.angleLimit` per joint. New sweep `spring_bone_extended_collider_sweep()` produces 18 base × settle/swing = 36 plans.

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §5. Companion VRM extension: `https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone_extended_collider-1.0`.

---

## File map

**Modify:**
- `crates/vrm-asset-generator/src/spring_bone.rs` — extend `ColliderShape` enum, add `joint_angle_limit_deg` field
- `crates/vrm-asset-generator/src/vrm_ext.rs` — emit extended_collider extension on each collider + per-joint angleLimit
- `crates/vrm-asset-generator/src/sweep.rs` — `spring_bone_extended_collider_sweep()` (18 variants)
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_spring_bone_extended_test_plan()` + swing variant
- `crates/vrm-asset-generator/src/emit.rs` — reuse `emit_vrm_with_spring_bone_colliders` if shape is general enough; otherwise new `emit_vrm_with_spring_bone_extended_colliders`
- `crates/vrm-asset-generator/src/cli.rs` — `emit-springbone-extended-sweep` subcommand

---

## Task 1: Extend `ColliderShape` enum with extended variants

**Files:** `crates/vrm-asset-generator/src/spring_bone.rs`

- [ ] **Step 1: Tests.** Append:

```rust
#[cfg(test)]
mod extended_collider_tests {
    use super::*;

    #[test]
    fn plane_collider_has_normal_vector() {
        let s = ColliderShape::Plane { normal: [0.0, 1.0, 0.0] };
        if let ColliderShape::Plane { normal } = s {
            assert_eq!(normal, [0.0, 1.0, 0.0]);
        } else {
            panic!("expected plane");
        }
    }

    #[test]
    fn inside_sphere_collider() {
        let s = ColliderShape::InsideSphere { radius: 0.25 };
        if let ColliderShape::InsideSphere { radius } = s {
            assert!((radius - 0.25).abs() < 1e-6);
        } else { panic!("expected inside sphere"); }
    }

    #[test]
    fn inside_capsule_collider() {
        let s = ColliderShape::InsideCapsule { radius: 0.10, tail_offset: [0.0, 0.20, 0.0] };
        if let ColliderShape::InsideCapsule { radius, tail_offset } = s {
            assert!((radius - 0.10).abs() < 1e-6);
            assert_eq!(tail_offset, [0.0, 0.20, 0.0]);
        } else { panic!("expected inside capsule"); }
    }

    #[test]
    fn spring_bone_params_carries_optional_joint_angle_limit() {
        let mut p = SpringBoneParams::defaults("t");
        assert!(p.joint_angle_limit_deg.is_none());
        p.joint_angle_limit_deg = Some(45.0);
        assert_eq!(p.joint_angle_limit_deg, Some(45.0));
    }
}
```

- [ ] **Step 2: Run, expect failure.**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator extended_collider_tests
  ```

- [ ] **Step 3: Extend the enum** (add to existing `ColliderShape`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Capsule { radius: f32, tail_offset: [f32; 3] },
    // Extended (VRMC_springBone_extended_collider-1.0):
    Plane { normal: [f32; 3] },
    InsideSphere { radius: f32 },
    InsideCapsule { radius: f32, tail_offset: [f32; 3] },
}
```

Add `joint_angle_limit_deg: Option<f32>` to `SpringBoneParams` (default `None`, omitted from JSON via `#[serde(default, skip_serializing_if = "Option::is_none")]`). Update `SpringBoneParams::defaults` to initialize to `None`.

- [ ] **Step 4: Run + lint + commit.**
  ```
  cargo test -p vrm-asset-generator extended_collider_tests
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/spring_bone.rs && git commit -m "feat(vrm-asset-generator): extended collider shapes + per-joint angle limit"
  ```

---

## Task 2: Emit extended_collider extension JSON

**Files:** `crates/vrm-asset-generator/src/vrm_ext.rs`

The spec convention: each `colliders[i]` entry has a base `shape` field (sphere/capsule per VRMC_springBone-1.0) OR an `extensions.VRMC_springBone_extended_collider.shape` field for extended types. When using an extended shape, the base `shape` field is omitted; the renderer reads only the extension shape.

Per-joint angle limits go under `springs[].joints[].extensions.VRMC_springBone_extended_collider.angleLimit` (in degrees).

The top-level extension list (`extensions["VRMC_springBone_extended_collider"]: {}`) must also be declared in the glTF document's `extensionsUsed` so loaders recognize it. **The asset generator's glTF doc construction sets `extensionsUsed`; verify it adds the new extension name when any extended shape is emitted.**

- [ ] **Step 1: Tests.** Append to `vrm_ext.rs`:

```rust
#[cfg(test)]
mod extended_emit_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn plane_collider_emits_extension_shape() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Plane { normal: [0.0, 1.0, 0.0] },
                offset: [0.0, -0.10, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams { name: "g".into(), collider_indices: vec![0] }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[10]);
        let c0 = &v["colliders"][0];
        // Base shape MUST be omitted when extended is used:
        assert!(c0.get("shape").is_none() || c0["shape"].as_object().map(|o| o.is_empty()).unwrap_or(false),
            "base shape must be omitted when using extended shape, got {c0}");
        let ext = &c0["extensions"]["VRMC_springBone_extended_collider"]["shape"];
        assert!(ext["plane"].is_object(), "expected plane extended shape: {c0}");
        let normal = ext["plane"]["normal"].as_array().unwrap();
        assert!((normal[1].as_f64().unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn inside_sphere_emits_extension_shape() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::InsideSphere { radius: 0.20 },
                offset: [0.0, 0.0, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams { name: "g".into(), collider_indices: vec![0] }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[10]);
        let ext = &v["colliders"][0]["extensions"]["VRMC_springBone_extended_collider"]["shape"];
        assert!(ext["sphere"].is_object());
        assert_eq!(ext["sphere"]["inside"], true);
        assert!((ext["sphere"]["radius"].as_f64().unwrap() - 0.20).abs() < 1e-6);
    }

    #[test]
    fn inside_capsule_emits_inside_true() {
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("c")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::InsideCapsule { radius: 0.10, tail_offset: [0.0, 0.30, 0.0] },
                offset: [0.0, 0.0, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams { name: "g".into(), collider_indices: vec![0] }],
            spring_collider_groups: vec![vec![0]],
        };
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[10]);
        let ext = &v["colliders"][0]["extensions"]["VRMC_springBone_extended_collider"]["shape"];
        assert!(ext["capsule"].is_object());
        assert_eq!(ext["capsule"]["inside"], true);
    }

    #[test]
    fn joint_angle_limit_emits_under_extension() {
        let mut spring = SpringBoneParams::defaults("c");
        spring.joint_angle_limit_deg = Some(60.0);
        let scene = SpringBoneSceneParams::single_spring(spring);
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[]);
        let joints = v["springs"][0]["joints"].as_array().unwrap();
        for j in joints {
            let limit = &j["extensions"]["VRMC_springBone_extended_collider"]["angleLimit"];
            assert!((limit.as_f64().unwrap() - 60.0).abs() < 1e-6,
                "expected angleLimit=60 on every joint, got {j}");
        }
    }

    #[test]
    fn no_angle_limit_does_not_emit_extension_on_joints() {
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("c"));
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[]);
        let j0 = &v["springs"][0]["joints"][0];
        assert!(j0.get("extensions").is_none() || j0["extensions"].as_object().unwrap().is_empty(),
            "joint with no angle limit must not carry extensions block, got {j0}");
    }
}
```

- [ ] **Step 2: Extend `vrmc_spring_bone_scene`** to:
  - For Sphere/Capsule (base shapes): emit `shape` field as today.
  - For Plane/InsideSphere/InsideCapsule: omit `shape`, emit `extensions.VRMC_springBone_extended_collider.shape`. The extension shape JSON shape per VRMC_springBone_extended_collider-1.0:
    - `{ "plane": { "offset": [..], "normal": [..] } }`
    - `{ "sphere": { "offset": [..], "radius": .., "inside": true } }` (base sphere stays inside:false implicit by omission)
    - `{ "capsule": { "offset": [..], "radius": .., "tail": [..], "inside": true } }`
  - For per-joint angle limits: when `params.joint_angle_limit_deg.is_some()`, emit `extensions.VRMC_springBone_extended_collider.angleLimit` on EVERY joint of that spring. (The spec applies angle limits per joint; for our sweep, the value is uniform across joints in a chain.)

- [ ] **Step 3: Document `extensionsUsed` requirement.** Find where the glTF doc's `extensionsUsed` is built (likely `emit.rs` glTF assembly). When any collider uses an extended shape OR any joint has an angle limit, ensure `extensionsUsed` includes both `VRMC_springBone` and `VRMC_springBone_extended_collider`. If `emit.rs` doesn't yet handle this conditional, add the logic in Task 3.

- [ ] **Step 4: Run + lint + commit.**
  ```
  cargo test -p vrm-asset-generator extended_emit
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/vrm_ext.rs && git commit -m "feat(vrm-asset-generator): VRMC_springBone_extended_collider shape + angle limit emission"
  ```

---

## Task 3: emit.rs — wire extensionsUsed + reuse collider emit path

**Files:** `crates/vrm-asset-generator/src/emit.rs`

- [ ] **Step 1:** Find where `extensionsUsed` is built in the glTF doc. Add a helper or extend the existing logic so the asset declares `"VRMC_springBone_extended_collider"` in `extensionsUsed` when the scene contains any extended-shape collider or angle limit. Use a function like `scene_uses_extended_collider(scene: &SpringBoneSceneParams) -> bool`.

- [ ] **Step 2:** Write a glb-readback test for an extended-shape emission:

```rust
#[cfg(test)]
mod extended_emit_integration_tests {
    use super::*;
    use crate::params::MToonParams;
    use crate::spring_bone::*;
    use tempfile::tempdir;
    use camino::Utf8Path;

    #[test]
    fn emitted_glb_with_plane_collider_declares_extended_collider_in_extensionsUsed() {
        let mtoon = MToonParams::defaults("test_plane");
        let scene = SpringBoneSceneParams {
            springs: vec![SpringBoneParams::defaults("test")],
            colliders: vec![ColliderParams {
                shape: ColliderShape::Plane { normal: [0.0, 1.0, 0.0] },
                offset: [0.0, -0.10, 0.0],
                attach: ColliderAttach::Head,
            }],
            collider_groups: vec![ColliderGroupParams { name: "g".into(), collider_indices: vec![0] }],
            spring_collider_groups: vec![vec![0]],
        };
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        emit_vrm_with_spring_bone_colliders(&mtoon, &scene, &vrm_path).unwrap();
        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let used = doc["extensionsUsed"].as_array().unwrap();
        let names: Vec<&str> = used.iter().filter_map(|v| v.as_str()).collect();
        assert!(names.contains(&"VRMC_springBone"), "extensionsUsed must declare VRMC_springBone: {names:?}");
        assert!(names.contains(&"VRMC_springBone_extended_collider"),
            "extensionsUsed must declare VRMC_springBone_extended_collider when plane shape used: {names:?}");
    }
}
```

- [ ] **Step 3:** Implement the helper + extensionsUsed update. Reuse `emit_vrm_with_spring_bone_colliders` from phase 2 — it already takes `SpringBoneSceneParams`. The only change is the conditional extensionsUsed addition.

- [ ] **Step 4: Run + lint + commit.**
  ```
  cargo test -p vrm-asset-generator extended_emit_integration
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  git add crates/vrm-asset-generator/src/emit.rs && git commit -m "feat(vrm-asset-generator): declare VRMC_springBone_extended_collider in extensionsUsed"
  ```

---

## Task 4: Extended sweep — 18 variants

**Files:** `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Tests.**

```rust
#[cfg(test)]
mod extended_sweep_tests {
    use super::*;

    #[test]
    fn extended_sweep_produces_18_variants() {
        // 3 shapes × 3 placements = 9 (default angle, no limit)
        // 3 shapes × 3 angle_limits (30, 60, 90) at default placement = 9
        // Total: 18 base variants.
        let variants = spring_bone_extended_collider_sweep();
        assert_eq!(variants.len(), 18);
    }

    #[test]
    fn extended_sweep_unique_names() {
        let variants = spring_bone_extended_collider_sweep();
        let names: std::collections::HashSet<_> =
            variants.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(names.len(), 18);
    }

    #[test]
    fn extended_sweep_angle_limit_variants_actually_set_the_limit() {
        let variants = spring_bone_extended_collider_sweep();
        let limited: Vec<_> = variants
            .iter()
            .filter(|(_, s)| s.springs[0].joint_angle_limit_deg.is_some())
            .collect();
        assert_eq!(limited.len(), 9, "9 variants should carry angle limits");
    }
}
```

- [ ] **Step 2: Implement.**

```rust
pub fn spring_bone_extended_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(18);

    // First 9: 3 shapes × 3 placements, no angle limit.
    let shapes_for_placement: [(&str, fn(usize) -> ColliderShape, [f32; 3]); 3] = [
        ("plane",   make_plane_shape,   [0.0, 0.0, 0.0]),
        ("isphere", make_inside_sphere, [0.0, -0.10, 0.0]),
        ("icaps",   make_inside_capsule, [0.0, -0.10, 0.0]),
    ];

    let placement_keys = ["tight", "med", "loose"];
    for (shape_name, shape_fn, default_offset) in shapes_for_placement.iter() {
        for (p_idx, p_key) in placement_keys.iter().enumerate() {
            let id = format!("springbone_extended_{}_p{}", shape_name, p_key);
            let shape = shape_fn(p_idx);
            let collider = ColliderParams {
                shape,
                offset: *default_offset,
                attach: ColliderAttach::Head,
            };
            let scene = build_scene(&id, collider, None);
            out.push((MToonParams::defaults(&id), scene));
        }
    }

    // Second 9: 3 shapes × 3 angle limits (30, 60, 90), at default placement (medium).
    for (shape_name, shape_fn, default_offset) in shapes_for_placement.iter() {
        for &deg in [30.0_f32, 60.0, 90.0].iter() {
            let id = format!("springbone_extended_{}_anglelimit_{}", shape_name, deg as i32);
            let shape = shape_fn(1); // medium placement
            let collider = ColliderParams {
                shape,
                offset: *default_offset,
                attach: ColliderAttach::Head,
            };
            let scene = build_scene(&id, collider, Some(deg));
            out.push((MToonParams::defaults(&id), scene));
        }
    }

    out
}

fn make_plane_shape(p_idx: usize) -> ColliderShape {
    // Tight / med / loose: plane Y at -0.04 / -0.08 / -0.15 below chain root.
    let _ys = [-0.04_f32, -0.08, -0.15]; // captured via offset, not normal — normal stays +Y.
    // Actually for plane the normal IS what we vary if we want; but spec example uses
    // a plane at a Y offset. Keep normal +Y constant; vary the offset[1] in the placement
    // step. We'll handle that by overriding the offset in the loop instead. Simplest:
    // just return the same shape, let the caller set offset.
    ColliderShape::Plane { normal: [0.0, 1.0, 0.0] }
}

fn make_inside_sphere(p_idx: usize) -> ColliderShape {
    let rs = [0.10_f32, 0.20, 0.40];
    ColliderShape::InsideSphere { radius: rs[p_idx] }
}

fn make_inside_capsule(p_idx: usize) -> ColliderShape {
    let rs = [0.10_f32, 0.20, 0.40];
    ColliderShape::InsideCapsule { radius: rs[p_idx], tail_offset: [0.0, 0.30, 0.0] }
}

fn build_scene(id: &str, collider: ColliderParams, angle_limit: Option<f32>) -> SpringBoneSceneParams {
    let mut spring = SpringBoneParams::defaults(id);
    spring.joint_angle_limit_deg = angle_limit;
    SpringBoneSceneParams {
        springs: vec![spring],
        colliders: vec![collider],
        collider_groups: vec![ColliderGroupParams { name: "ext_g".into(), collider_indices: vec![0] }],
        spring_collider_groups: vec![vec![0]],
    }
}
```

Adjust the plane-placement to actually vary the offset (placement_keys index → offset_y). The function-pointer pattern above doesn't handle plane offsets well; refactor as needed for clarity (a `make_shape_with_placement(name, p_idx) -> (ColliderShape, [f32;3])` is cleaner).

- [ ] **Step 3: Run + lint + commit.**
  ```
  cargo test -p vrm-asset-generator extended_sweep
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/sweep.rs && git commit -m "feat(vrm-asset-generator): 18-variant extended collider sweep"
  ```

---

## Task 5: sidecar.rs — extended test plan builder

**Files:** `crates/vrm-asset-generator/src/sidecar.rs`

- [ ] **Step 1: Test.** Mirror the collider plan tests from phase 2 but for extended:

```rust
#[cfg(test)]
mod extended_plan_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn extended_plan_settle_has_60_settle_steps() {
        let mtoon = MToonParams::defaults("test_ext");
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("t"));
        let plan = build_spring_bone_extended_test_plan(&mtoon, &scene, "out.vrm");
        assert_eq!(plan.physics.unwrap().settle_steps, 60);
        assert!(plan.animation.is_none());
    }

    #[test]
    fn extended_plan_spec_section_names_both_extensions() {
        let mtoon = MToonParams::defaults("test_ext");
        let scene = SpringBoneSceneParams::single_spring(SpringBoneParams::defaults("t"));
        let plan = build_spring_bone_extended_test_plan(&mtoon, &scene, "out.vrm");
        assert!(plan.spec_section.contains("VRMC_springBone_extended_collider"),
            "spec_section should name the extension: {}", plan.spec_section);
    }
}
```

- [ ] **Step 2: Implement.** Same shape as `build_spring_bone_collider_test_plan` (phase 2) but with `spec_section: "VRMC_springBone + VRMC_springBone_extended_collider"`. Swing variant identical to phase 2's swing.

- [ ] **Step 3: Commit.**
  ```
  git add crates/vrm-asset-generator/src/sidecar.rs && git commit -m "feat(vrm-asset-generator): build_spring_bone_extended_test_plan settle + swing"
  ```

---

## Task 6: CLI subcommand `emit-springbone-extended-sweep`

**Files:** `crates/vrm-asset-generator/src/cli.rs`, `emit.rs` (sidecar emit wrapper)

- [ ] **Step 1:** Add `emit-springbone-extended-sweep --output-dir <dir>` subcommand following the same pattern as `emit-springbone-collider-sweep` (phase 2). Loop body emits both settle and swing — 18 × 2 = 36 plans.

- [ ] **Step 2:** Run the subcommand:
  ```
  cd /Users/arkavo/Projects/vrm-conformance
  cargo build -p vrm-asset-generator --release
  ./target/release/vrm-asset-generator emit-springbone-extended-sweep --output-dir /tmp/phase3-sweep
  ls /tmp/phase3-sweep/*.vrm | wc -l    # expect 36
  ls /tmp/phase3-sweep/*.test.yaml | wc -l    # expect 36
  ```

- [ ] **Step 3:** Validate with mrxz/vrm-validator if installed:
  ```
  ls .tools/vrm-validator-cli 2>/dev/null && \
    .tools/vrm-validator-cli /tmp/phase3-sweep/springbone_extended_plane_ptight_settle.vrm
  ```

- [ ] **Step 4: Commit.**
  ```
  git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/src/emit.rs && \
    git commit -m "feat(vrm-asset-generator): emit-springbone-extended-sweep subcommand (36 plans)"
  ```

---

## Task 7: docs/findings.md phase-3 entry

```markdown
## Phase 3 — VRMC_springBone_extended_collider sweep landed

**Trigger:** Phase 2 base-collider sweep merged. Phase 3 adds the companion extension `VRMC_springBone_extended_collider-1.0`: planes, inverted (inside) sphere/capsule, and per-joint angleLimit.

**Shipped:**
- ColliderShape variants: `Plane { normal }`, `InsideSphere { radius }`, `InsideCapsule { radius, tail_offset }`.
- `SpringBoneParams.joint_angle_limit_deg: Option<f32>` — emitted under `joints[].extensions.VRMC_springBone_extended_collider.angleLimit` (degrees, per-joint).
- glTF `extensionsUsed` correctly declares `VRMC_springBone_extended_collider` only when extended shapes or angle limits are present.
- `emit-springbone-extended-sweep` subcommand emits 36 plans (3 shapes × 3 placements + 3 shapes × 3 angle limits = 18 cartesian × settle/swing).

**Adapter coverage:** the extension is conformance-tested via cross-renderer diff in subsequent corpus runs. Adapters that don't support it should diff loudly. Known status: three-vrm and VRMMetalKit may have partial support (VMK#67 is the open angle-limit verification ticket); godot-vrm coverage depends on V-Sekai/godot-vrm's spec_extended state.

**Forward:** Phase 4 adds gravityDir variation.
```

- [ ] **Commit:**
  ```
  git add docs/findings.md && git commit -m "docs(findings): phase 3 extended collider sweep (36 plans) landed"
  ```

---

## Final acceptance

- [ ] All tests green, clippy clean, fmt clean.
- [ ] 36 vrms + 36 test plans emitted from the new subcommand.
- [ ] At least one emitted vrm passes mrxz/vrm-validator (if installed locally).
- [ ] findings.md entry present.
