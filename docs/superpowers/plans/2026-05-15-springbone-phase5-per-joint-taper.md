# VRMC_springBone Phase 5 — Per-joint Taper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Support per-joint parameter taper (stiffness, drag, gravity_power, hit_radius can each vary along the chain) and ship a 14-plan sweep that exercises tapered chains.

**Architecture deviation from spec:** The spec proposed a `JointVec<T>` enum (`Uniform(T) | PerJoint(Vec<T>)`). I'm using **optional parallel fields** instead — additive, no caller churn:

```rust
pub struct SpringBoneParams {
    // ... existing scalar fields (stiffness, drag_force, gravity_power, hit_radius) ...
    pub stiffness_per_joint: Option<Vec<f32>>,
    pub drag_force_per_joint: Option<Vec<f32>>,
    pub gravity_power_per_joint: Option<Vec<f32>>,
    pub hit_radius_per_joint: Option<Vec<f32>>,
}
```

When the optional vector is `Some`, its length must equal `joint_count` and each joint emits that joint's value; otherwise all joints emit the scalar. The `JointVec<T>` refactor can be revisited if phase 6 multi-chain forces a bigger API churn.

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §7.

---

## File map

- `crates/vrm-asset-generator/src/spring_bone.rs` — add 4 optional per-joint fields, update `defaults`
- `crates/vrm-asset-generator/src/vrm_ext.rs` — `vrmc_spring_bone_scene` reads per-joint vectors when present
- `crates/vrm-asset-generator/src/sweep.rs` — `spring_bone_taper_sweep()` (7 variants)
- `crates/vrm-asset-generator/src/sidecar.rs` — `meta.json` carries the per-joint vectors when set (reflective)
- `crates/vrm-asset-generator/src/cli.rs` — `emit-springbone-taper-sweep` subcommand (14 plans)
- `docs/findings.md` — phase 5 entry

---

## Task 1: Add optional per-joint fields + serde + tests

**Files:** `crates/vrm-asset-generator/src/spring_bone.rs`

- [ ] **Step 1: Tests:**

```rust
#[cfg(test)]
mod per_joint_tests {
    use super::*;

    #[test]
    fn defaults_leave_per_joint_vectors_none() {
        let p = SpringBoneParams::defaults("t");
        assert!(p.stiffness_per_joint.is_none());
        assert!(p.drag_force_per_joint.is_none());
        assert!(p.gravity_power_per_joint.is_none());
        assert!(p.hit_radius_per_joint.is_none());
    }

    #[test]
    fn serialized_json_omits_none_per_joint_fields() {
        let p = SpringBoneParams::defaults("t");
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("stiffness_per_joint").is_none());
        assert!(v.get("drag_force_per_joint").is_none());
        assert!(v.get("gravity_power_per_joint").is_none());
        assert!(v.get("hit_radius_per_joint").is_none());
    }

    #[test]
    fn per_joint_taper_roundtrips() {
        let mut p = SpringBoneParams::defaults("t");
        p.joint_count = 4;
        p.stiffness_per_joint = Some(vec![1.0, 0.8, 0.4, 0.1]);
        let s = serde_json::to_string(&p).unwrap();
        let back: SpringBoneParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.stiffness_per_joint, Some(vec![1.0, 0.8, 0.4, 0.1]));
    }
}
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Add the fields** with `#[serde(default, skip_serializing_if = "Option::is_none")]`. Update `SpringBoneParams::defaults` to initialize all four to `None`.

- [ ] **Step 4: Run + lint + commit.**
  ```
  cargo test -p vrm-asset-generator per_joint_tests
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/spring_bone.rs && git commit -m "feat(vrm-asset-generator): optional per-joint taper vectors on SpringBoneParams"
  ```

---

## Task 2: vrm_ext.rs emits per-joint values when vectors are set

**Files:** `crates/vrm-asset-generator/src/vrm_ext.rs`

- [ ] **Step 1: Tests:**

```rust
#[cfg(test)]
mod taper_emit_tests {
    use super::*;
    use crate::spring_bone::*;

    #[test]
    fn uniform_stiffness_emits_same_value_on_all_joints() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        p.stiffness = 0.5;
        let scene = SpringBoneSceneParams::single_spring(p);
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[]);
        let joints = v["springs"][0]["joints"].as_array().unwrap();
        for j in joints {
            assert!((j["stiffness"].as_f64().unwrap() - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn per_joint_stiffness_emits_taper() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        p.stiffness = 0.5; // ignored when per-joint set
        p.stiffness_per_joint = Some(vec![1.0, 0.7, 0.4, 0.1]);
        let scene = SpringBoneSceneParams::single_spring(p);
        let v = vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[]);
        let joints = v["springs"][0]["joints"].as_array().unwrap();
        let stiffnesses: Vec<f64> = joints.iter().map(|j| j["stiffness"].as_f64().unwrap()).collect();
        assert!((stiffnesses[0] - 1.0).abs() < 1e-6);
        assert!((stiffnesses[1] - 0.7).abs() < 1e-6);
        assert!((stiffnesses[2] - 0.4).abs() < 1e-6);
        assert!((stiffnesses[3] - 0.1).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "stiffness_per_joint length")]
    fn per_joint_length_mismatch_panics() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 4;
        p.stiffness_per_joint = Some(vec![1.0, 0.5]); // only 2, not 4
        let scene = SpringBoneSceneParams::single_spring(p);
        // This should panic at emission time — length mismatch is a programmer error.
        vrmc_spring_bone_scene(&[0,1,2,3], &scene, &[]);
    }

    #[test]
    fn per_joint_drag_force_emits_taper() {
        let mut p = SpringBoneParams::defaults("c");
        p.joint_count = 3;
        p.drag_force_per_joint = Some(vec![0.9, 0.5, 0.1]);
        let scene = SpringBoneSceneParams::single_spring(p);
        let v = vrmc_spring_bone_scene(&[0,1,2], &scene, &[]);
        let drags: Vec<f64> = v["springs"][0]["joints"]
            .as_array().unwrap()
            .iter()
            .map(|j| j["dragForce"].as_f64().unwrap())
            .collect();
        assert!((drags[0] - 0.9).abs() < 1e-6);
        assert!((drags[2] - 0.1).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Update `vrmc_spring_bone_scene`** to look up per-joint values. Build per-joint resolver helpers:

```rust
fn joint_value(per_joint: &Option<Vec<f32>>, uniform: f32, joint_idx: usize, joint_count: usize, field_name: &str) -> f32 {
    if let Some(v) = per_joint {
        assert_eq!(v.len(), joint_count,
            "{}_per_joint length {} must match joint_count {}",
            field_name, v.len(), joint_count);
        v[joint_idx]
    } else {
        uniform
    }
}
```

In the joint emission loop, replace `params.stiffness` with `joint_value(&params.stiffness_per_joint, params.stiffness, i, joint_count, "stiffness")` and similar for the other three fields. `joint_count` here is `joint_nodes.len()`.

- [ ] **Step 4: Run + lint + commit:**
  ```
  cargo test -p vrm-asset-generator taper_emit_tests
  cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
  cargo fmt -p vrm-asset-generator -- --check
  git add crates/vrm-asset-generator/src/vrm_ext.rs && git commit -m "feat(vrm-asset-generator): emit per-joint taper values from spring params"
  ```

---

## Task 3: Taper sweep — 7 variants

**Files:** `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Tests.**

```rust
#[cfg(test)]
mod taper_sweep_tests {
    use super::*;

    #[test]
    fn taper_sweep_produces_7_variants() {
        // 4 stiffness shapes + 3 drag shapes = 7.
        let variants = spring_bone_taper_sweep();
        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn taper_sweep_each_variant_has_a_per_joint_vector_set() {
        let variants = spring_bone_taper_sweep();
        for p in &variants {
            let has_per_joint = p.stiffness_per_joint.is_some()
                || p.drag_force_per_joint.is_some()
                || p.gravity_power_per_joint.is_some()
                || p.hit_radius_per_joint.is_some();
            assert!(has_per_joint, "{}: must have at least one per-joint vector", p.id);
        }
    }

    #[test]
    fn taper_sweep_vector_lengths_match_joint_count() {
        let variants = spring_bone_taper_sweep();
        for p in &variants {
            if let Some(v) = &p.stiffness_per_joint {
                assert_eq!(v.len() as u32, p.joint_count, "{}: stiffness vector len mismatch", p.id);
            }
            if let Some(v) = &p.drag_force_per_joint {
                assert_eq!(v.len() as u32, p.joint_count, "{}: drag vector len mismatch", p.id);
            }
        }
    }

    #[test]
    fn taper_sweep_unique_ids() {
        let variants = spring_bone_taper_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 7);
    }
}
```

- [ ] **Step 2: Implement:**

```rust
pub fn spring_bone_taper_sweep() -> Vec<SpringBoneParams> {
    let mut out = Vec::with_capacity(7);
    let n: u32 = 4; // joint_count used throughout this sweep

    // Stiffness tapers (4 shapes):
    let stiffness_shapes: [(&str, Vec<f32>); 4] = [
        ("stiffness_flat",        vec![0.5, 0.5, 0.5, 0.5]),
        ("stiffness_high_to_low", vec![1.0, 0.7, 0.4, 0.1]),
        ("stiffness_low_to_high", vec![0.1, 0.4, 0.7, 1.0]),
        ("stiffness_expdecay",    vec![1.0, 0.5, 0.25, 0.125]),
    ];
    for (suffix, vec) in stiffness_shapes {
        let mut p = SpringBoneParams::defaults(format!("springbone_taper_{}", suffix));
        p.joint_count = n;
        p.stiffness_per_joint = Some(vec);
        out.push(p);
    }

    // Drag tapers (3 shapes — flat omitted since it would duplicate the stiffness_flat baseline):
    let drag_shapes: [(&str, Vec<f32>); 3] = [
        ("drag_flat",        vec![0.5, 0.5, 0.5, 0.5]),
        ("drag_high_to_low", vec![1.0, 0.7, 0.4, 0.1]),
        ("drag_expdecay",    vec![1.0, 0.5, 0.25, 0.125]),
    ];
    for (suffix, vec) in drag_shapes {
        let mut p = SpringBoneParams::defaults(format!("springbone_taper_{}", suffix));
        p.joint_count = n;
        p.drag_force_per_joint = Some(vec);
        out.push(p);
    }

    out
}
```

- [ ] **Step 3: Run + lint + commit:**
  ```
  cargo test -p vrm-asset-generator taper_sweep_tests
  git add crates/vrm-asset-generator/src/sweep.rs && git commit -m "feat(vrm-asset-generator): 7-variant per-joint taper sweep"
  ```

---

## Task 4: CLI subcommand + sidecar reflection of per-joint vectors

**Files:** `crates/vrm-asset-generator/src/cli.rs`, `sidecar.rs` (verify meta.json carries the vectors)

- [ ] **Step 1: Verify `write_meta_json` correctly serializes per-joint vectors.** The existing `meta["spring_bone"] = serde_json::to_value(sb)?` will automatically include the optional vectors when set (since SpringBoneParams derives Serialize). Sanity check by running the sweep and `cat`ing a `.meta.json`. If something is missing, fix `sidecar.rs`.

- [ ] **Step 2: Add `emit-springbone-taper-sweep` subcommand** following the pattern of `emit-springbone-swing-sweep`: iterate `spring_bone_taper_sweep()`, emit settle AND swing per variant = 14 plans total. Reuse existing `emit_with_sidecars_spring_bone` and `..._swing` (no new emit functions needed; per-joint vectors are SpringBoneParams fields).

- [ ] **Step 3: Smoke test:**
  ```
  cd /Users/arkavo/Projects/vrm-conformance
  cargo build -p vrm-asset-generator --release
  mkdir -p /tmp/phase5-sweep && ./target/release/vrm-asset-generator emit-springbone-taper-sweep --output-dir /tmp/phase5-sweep
  ls /tmp/phase5-sweep/*.vrm | wc -l       # expect 14
  ls /tmp/phase5-sweep/*.test.yaml | wc -l # expect 14
  cat /tmp/phase5-sweep/springbone_taper_stiffness_high_to_low_settle.meta.json | python3 -m json.tool | grep -A 6 stiffness_per_joint
  ```

- [ ] **Step 4: Validate one asset** if `.tools/vrm-validator-cli` exists.

- [ ] **Step 5: Commit:**
  ```
  git add crates/vrm-asset-generator/src/cli.rs && git commit -m "feat(vrm-asset-generator): emit-springbone-taper-sweep subcommand (14 plans)"
  ```

---

## Task 5: docs/findings.md phase-5 entry

```markdown
## Phase 5 — per-joint taper sweep landed (14 plans)

**Trigger:** Phase 4 gravityDir sweep merged. Phase 5 closes the per-joint variation axis: real hair tapers stiffness toward the tip; uniform scalars hide adapter-level discretization bugs that only manifest on non-uniform chains.

**Shipped:** Four optional per-joint vectors on `SpringBoneParams`:
- `stiffness_per_joint: Option<Vec<f32>>`
- `drag_force_per_joint: Option<Vec<f32>>`
- `gravity_power_per_joint: Option<Vec<f32>>`
- `hit_radius_per_joint: Option<Vec<f32>>`

When `Some(v)`, `v.len() == joint_count` is required; the per-joint vector overrides the scalar. `emit-springbone-taper-sweep` produces 14 plans (4 stiffness shapes + 3 drag shapes × settle/swing).

**Deliberate architecture deviation:** the spec proposed a `JointVec<T>` enum (`Uniform | PerJoint`). The optional-parallel-field shape is additively cheaper and avoids churn through existing callers — equivalent expressiveness for this phase's needs. Revisit if phase 6 multi-chain forces a bigger API refactor.

**Forward:** Phase 6 — multi-chain emission.
```

- [ ] Commit:
  ```
  git add docs/findings.md && git commit -m "docs(findings): phase 5 per-joint taper sweep (14 plans) landed"
  ```

---

## Acceptance

- [ ] All tests green
- [ ] clippy + fmt clean
- [ ] 14 vrms + 14 test plans emitted
- [ ] `.meta.json` carries `stiffness_per_joint` (or `drag_force_per_joint`) array for variants that set it
- [ ] findings.md entry present
