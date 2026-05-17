# VRMA Phase 2 — Diff Engine + Test Plan Schema + Manifest + Runner Integration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land all the substrate needed for VRMA tests to flow end-to-end through the runner. After this phase, a test plan with an `animation.vrma` block and `pose_tolerance` fields drives the runner through `load_vrma → apply_vrma_at_time → dump_*` ops, computes pose-vector diff against a reference renderer, and emits a structured pass/fail. Until phase 3 lands real assets, the runner is testable end-to-end against a fixture .vrma.

**Architecture:**
- `vrm-diff-engine`: new `pose_diff` module exporting `PoseDiffReport` + a `diff_pose` function (parallel to existing `positions::diff_positions`).
- `vrm-test-plan`: extend `AnimationConfig` with optional `vrma: VrmaAnimation` block; extend `Diff` with optional `pose_tolerance: PoseTolerance` block.
- `vrm-s3`: extend `ManifestEntry` with optional `vrma_url` + `vrma_blake3` fields (mirroring existing `positions_url` / `positions_blake3` pattern).
- `vrm-runner`: extend `ExecuteOptions` + `ExecuteResult`, wire 5 new ops into `plan_to_ops`, run pose-diff in `diff.rs`, add new CLI flags.

**Tech Stack:** Rust workspace only. No adapter-side work in phase 2 — phase 4 (UniVRM) and phase 5 (three-vrm) make the adapters real.

**Spec:** [`docs/superpowers/specs/2026-05-17-vrma-conformance-design.md`](../specs/2026-05-17-vrma-conformance-design.md) — Diff math, Test plan schema extension, Op sequence sections.

**Builds on:** [`docs/superpowers/plans/2026-05-17-vrma-phase1-op-surface.md`](./2026-05-17-vrma-phase1-op-surface.md) — phase 1 added the 5 op types in `vrm-ops/`.

---

## File structure

**Create:**
- `crates/vrm-diff-engine/src/pose_diff.rs` — pose-vector diff math + `PoseDiffReport`

**Modify:**
- `crates/vrm-diff-engine/src/lib.rs` — export `pose_diff` module
- `crates/vrm-test-plan/src/lib.rs` — add `VrmaAnimation`, `PoseTolerance` types; extend `AnimationConfig` and `Diff`
- `crates/vrm-s3/src/manifest.rs` — add `vrma_url`, `vrma_blake3` to `ManifestEntry`
- `crates/vrm-runner/src/execute.rs` — extend `ExecuteOptions` + `ExecuteResult`
- `crates/vrm-runner/src/plan_to_ops.rs` — emit VRMA op sequence when `animation.vrma` present
- `crates/vrm-runner/src/diff.rs` — invoke pose_diff alongside SSIM
- `crates/vrm-runner/src/cli.rs` — new flags: `--reference-pose-json`, `--vrma`, `--apply-at-time`

**Test files added/updated:** existing test files in each crate gain new tests for the new types/functions.

---

## Task 1: pose_diff module skeleton + PoseDiffReport struct

**Files:**
- Create: `crates/vrm-diff-engine/src/pose_diff.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs`

- [ ] **Step 1.1: Write the failing module-import test**

Add to `crates/vrm-diff-engine/src/lib.rs` exports (after the existing `pub mod positions;` line):

```rust
pub mod pose_diff;
```

Then verify by running: `cargo build -p vrm-diff-engine`
Expected: FAIL with `file not found for module 'pose_diff'`.

- [ ] **Step 1.2: Create pose_diff.rs with the PoseDiffReport struct + a stub diff_pose function**

Create `crates/vrm-diff-engine/src/pose_diff.rs`:

```rust
//! Pose-vector diff for VRMA conformance: per-bone quaternion geodesic
//! distance + hips translation + expression weight deltas + lookAt angle
//! deltas. Returns a structured report; pass/fail decided by per-channel
//! tolerances supplied by the caller.

use serde::{Deserialize, Serialize};
use vrm_ops::tools::{
    DumpExpressionWeightsResult, DumpHumanoidPoseResult, DumpLookAtStateResult,
};

/// Per-channel tolerances. All in their respective units (radians, meters,
/// scalar deltas, degrees). A pass requires every per-channel max to be
/// at or below its tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseTolerances {
    pub per_bone_quaternion_radians: f32,
    pub hips_translation_m: f32,
    pub per_preset_expression: f32,
    pub per_custom_expression: f32,
    pub look_at_yaw_pitch_degrees: f32,
    pub offset_from_head_bone_m: f32,
}

impl Default for PoseTolerances {
    /// v1 defaults from the design spec.
    fn default() -> Self {
        Self {
            per_bone_quaternion_radians: 0.010,
            hips_translation_m: 0.005,
            per_preset_expression: 0.005,
            per_custom_expression: 0.005,
            look_at_yaw_pitch_degrees: 1.0,
            offset_from_head_bone_m: 0.001,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoseDiffReport {
    pub per_bone_rotation_max_rad: f32,
    pub per_bone_rotation_worst_bone: Option<String>,
    pub hips_translation_m: f32,
    pub per_preset_expression_max_delta: f32,
    pub per_preset_expression_worst: Option<String>,
    pub per_custom_expression_max_delta: f32,
    pub per_custom_expression_worst: Option<String>,
    pub look_at_yaw_delta_deg: f32,
    pub look_at_pitch_delta_deg: f32,
    pub offset_from_head_bone_m: f32,
    pub overall_passed: bool,
}

/// Diff the actual pose against a reference. Returns a structured report.
/// Pass requires every per-channel max to be at or below its tolerance.
///
/// Channels are diffed independently:
/// - Humanoid bone rotations: quaternion geodesic distance `2·acos(|q·q'|)`
///   (sign-invariant by construction). Bones in `actual.bones_missing` or
///   `reference.bones_missing` are excluded (methodology hazard #3).
/// - Hips translation: Euclidean distance.
/// - Expression weights: scalar abs-delta. Presets and custom kept
///   separate.
/// - LookAt: yaw and pitch abs-delta in degrees (each compared
///   independently; the worse of the two contributes to pass/fail).
/// - offsetFromHeadBone: Euclidean distance.
pub fn diff_pose(
    actual_humanoid: &DumpHumanoidPoseResult,
    reference_humanoid: &DumpHumanoidPoseResult,
    actual_expressions: &DumpExpressionWeightsResult,
    reference_expressions: &DumpExpressionWeightsResult,
    actual_look_at: &DumpLookAtStateResult,
    reference_look_at: &DumpLookAtStateResult,
    tolerances: &PoseTolerances,
) -> PoseDiffReport {
    // Stub for now — will be filled in by tasks 2-5.
    todo!("filled in by subsequent tasks")
}
```

- [ ] **Step 1.3: Verify it compiles (but tests fail on the todo!())**

Run: `cargo build -p vrm-diff-engine`
Expected: SUCCESS (compiles fine — `todo!()` is a build-time-valid macro).

Run: `cargo clippy -p vrm-diff-engine --all-targets -- -D warnings`
Expected: clean (the `todo!()` is fine; unused-import warnings from the imports we'll need shortly should be allowed via the `#[allow(unused_imports)]` you may need to add at the top of `pose_diff.rs` until the function body lands).

If clippy flags unused imports on `DumpExpressionWeightsResult`/`DumpLookAtStateResult`/`DumpHumanoidPoseResult`, add this single line at the top of the imports:

```rust
#![allow(unused_imports)]  // imports populated by subsequent tasks
```

Remove this allow when task 5 completes.

- [ ] **Step 1.4: Commit**

```bash
git add crates/vrm-diff-engine/src/pose_diff.rs crates/vrm-diff-engine/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): add pose_diff module skeleton + PoseDiffReport

Phase 2.1 of VRMA closure. Adds PoseTolerances (with v1 defaults from
design spec) and PoseDiffReport types; diff_pose function stubbed with
todo!() to be filled in by tasks 2-5 (per-bone rotation, hips
translation, expressions, lookAt, integration).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Per-bone quaternion geodesic distance + hips translation diff

**Files:**
- Modify: `crates/vrm-diff-engine/src/pose_diff.rs`

- [ ] **Step 2.1: Write the failing test**

Append to `crates/vrm-diff-engine/src/pose_diff.rs` (in a `#[cfg(test)] mod tests` block at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vrm_ops::tools::{HumanoidBoneRotation, LookAtAppliedVia};

    fn empty_expressions() -> DumpExpressionWeightsResult {
        DumpExpressionWeightsResult {
            presets: std::collections::BTreeMap::new(),
            custom: std::collections::BTreeMap::new(),
        }
    }

    fn identity_look_at() -> DumpLookAtStateResult {
        DumpLookAtStateResult {
            gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            applied_via: LookAtAppliedVia::Off,
            offset_from_head_bone: [0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn identical_pose_passes() {
        let pose = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "leftUpperArm".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &pose,
            &pose,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(report.overall_passed);
        assert_eq!(report.per_bone_rotation_max_rad, 0.0);
        assert_eq!(report.hips_translation_m, 0.0);
    }

    #[test]
    fn quaternion_geodesic_is_sign_invariant() {
        // q and -q represent the same orientation. Identity at +w=1 vs -w=-1
        // should report zero geodesic distance.
        let p_pos = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let p_neg = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, -1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &p_pos,
            &p_neg,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(
            report.per_bone_rotation_max_rad < 1e-5,
            "expected ~0, got {}",
            report.per_bone_rotation_max_rad
        );
    }

    #[test]
    fn hips_translation_fails_outside_tolerance() {
        let pose_a = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let pose_b = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.020, 0.0, 0.0],  // 20mm — exceeds 5mm default
            bones_missing: vec![],
        };
        let report = diff_pose(
            &pose_a,
            &pose_b,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(!report.overall_passed);
        assert!((report.hips_translation_m - 0.020).abs() < 1e-5);
    }

    #[test]
    fn missing_bones_excluded_from_diff() {
        let actual = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec!["leftThumbDistal".into()],
        };
        let reference = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &actual,
            &reference,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        // leftThumbDistal isn't in actual.bones; it must not contribute to diff
        // (missing on actual side is fine — the .vrm just doesn't have that bone).
        assert!(report.overall_passed);
    }
}
```

- [ ] **Step 2.2: Verify tests fail with the todo!() panic**

Run: `cargo test -p vrm-diff-engine pose_diff`
Expected: FAIL — `panicked at 'not yet implemented'`.

- [ ] **Step 2.3: Implement humanoid pose diff in diff_pose**

Replace the `todo!("filled in by subsequent tasks")` body of `diff_pose` with the bones + hips part (expressions and lookAt stay `todo!()` for now via partial fields):

```rust
pub fn diff_pose(
    actual_humanoid: &DumpHumanoidPoseResult,
    reference_humanoid: &DumpHumanoidPoseResult,
    actual_expressions: &DumpExpressionWeightsResult,
    reference_expressions: &DumpExpressionWeightsResult,
    actual_look_at: &DumpLookAtStateResult,
    reference_look_at: &DumpLookAtStateResult,
    tolerances: &PoseTolerances,
) -> PoseDiffReport {
    // Per-bone rotation: quaternion geodesic distance, sign-invariant.
    // Bones in either bones_missing list are skipped — methodology
    // hazard #3 (a renderer that doesn't expose the bone shouldn't
    // contribute to diff).
    let missing_actual: std::collections::HashSet<&String> =
        actual_humanoid.bones_missing.iter().collect();
    let missing_reference: std::collections::HashSet<&String> =
        reference_humanoid.bones_missing.iter().collect();

    let reference_by_name: std::collections::HashMap<&String, &[f32; 4]> = reference_humanoid
        .bones
        .iter()
        .map(|b| (&b.name, &b.local_rotation_quat))
        .collect();

    let mut per_bone_max = 0.0_f32;
    let mut worst_bone: Option<String> = None;

    for bone in &actual_humanoid.bones {
        if missing_actual.contains(&bone.name) || missing_reference.contains(&bone.name) {
            continue;
        }
        let Some(ref_quat) = reference_by_name.get(&bone.name) else {
            // The reference doesn't have this bone — treat as excluded.
            continue;
        };
        let geodesic = quaternion_geodesic_rad(&bone.local_rotation_quat, ref_quat);
        if geodesic > per_bone_max {
            per_bone_max = geodesic;
            worst_bone = Some(bone.name.clone());
        }
    }

    // Hips translation: Euclidean distance.
    let hips = euclidean_distance(
        &actual_humanoid.hips_translation,
        &reference_humanoid.hips_translation,
    );

    // Expressions + lookAt stay zero for now (tasks 3-4 fill in).
    let _ = (
        actual_expressions,
        reference_expressions,
        actual_look_at,
        reference_look_at,
    );

    let humanoid_pass =
        per_bone_max <= tolerances.per_bone_quaternion_radians
            && hips <= tolerances.hips_translation_m;

    PoseDiffReport {
        per_bone_rotation_max_rad: per_bone_max,
        per_bone_rotation_worst_bone: worst_bone,
        hips_translation_m: hips,
        per_preset_expression_max_delta: 0.0,
        per_preset_expression_worst: None,
        per_custom_expression_max_delta: 0.0,
        per_custom_expression_worst: None,
        look_at_yaw_delta_deg: 0.0,
        look_at_pitch_delta_deg: 0.0,
        offset_from_head_bone_m: 0.0,
        overall_passed: humanoid_pass,
    }
}

/// Geodesic distance between two quaternions: `2·acos(|dot|)`. Sign-
/// invariant — `q` and `-q` yield identical results (same orientation).
fn quaternion_geodesic_rad(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    // Clamp into [-1, 1] to absorb float rounding above the legal range.
    let clamped = dot.abs().min(1.0);
    2.0 * clamped.acos()
}

fn euclidean_distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}
```

Remove the `#![allow(unused_imports)]` line from Task 1 if you added it — `DumpExpressionWeightsResult` and `DumpLookAtStateResult` are now referenced (even if their values are unused via the `_ = (...)` discard).

- [ ] **Step 2.4: Verify tests pass**

Run: `cargo test -p vrm-diff-engine pose_diff`
Expected: all 4 tests pass.

- [ ] **Step 2.5: Verify clippy clean**

Run: `cargo clippy -p vrm-diff-engine --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 2.6: Commit**

```bash
git add crates/vrm-diff-engine/src/pose_diff.rs
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): humanoid pose diff (rotation geodesic + hips)

Quaternion geodesic distance is sign-invariant by construction
(2·acos(|dot|) with absolute value). Bones in either bones_missing list
are excluded from diff per methodology hazard #3. Hips translation is
Euclidean distance. Expressions + lookAt diff land in tasks 3-4.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Expression weights diff (preset + custom)

**Files:**
- Modify: `crates/vrm-diff-engine/src/pose_diff.rs`

- [ ] **Step 3.1: Write the failing test**

Add tests to the `mod tests` block:

```rust
    #[test]
    fn preset_expression_max_delta_picked() {
        let mut actual_presets = std::collections::BTreeMap::new();
        actual_presets.insert("happy".into(), 0.5_f32);
        actual_presets.insert("blink".into(), 0.0_f32);
        let actual = DumpExpressionWeightsResult {
            presets: actual_presets,
            custom: Default::default(),
        };

        let mut ref_presets = std::collections::BTreeMap::new();
        ref_presets.insert("happy".into(), 0.4_f32);  // delta 0.1 (fails)
        ref_presets.insert("blink".into(), 0.0_f32);  // delta 0.0
        let reference = DumpExpressionWeightsResult {
            presets: ref_presets,
            custom: Default::default(),
        };

        let bones = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &bones,
            &bones,
            &actual,
            &reference,
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(!report.overall_passed);
        assert!((report.per_preset_expression_max_delta - 0.1).abs() < 1e-5);
        assert_eq!(report.per_preset_expression_worst.as_deref(), Some("happy"));
    }

    #[test]
    fn missing_preset_treated_as_zero() {
        // Actual has happy=0.5; reference has no entry for happy.
        // Renderers that don't carry an expression are treated as weight 0.
        let mut actual_presets = std::collections::BTreeMap::new();
        actual_presets.insert("happy".into(), 0.5);
        let actual = DumpExpressionWeightsResult {
            presets: actual_presets,
            custom: Default::default(),
        };
        let reference = DumpExpressionWeightsResult {
            presets: Default::default(),
            custom: Default::default(),
        };
        let bones = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &bones,
            &bones,
            &actual,
            &reference,
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!((report.per_preset_expression_max_delta - 0.5).abs() < 1e-5);
    }
```

- [ ] **Step 3.2: Verify tests fail**

Run: `cargo test -p vrm-diff-engine pose_diff::tests::preset`
Expected: FAIL — assertions fail because expression delta is hardcoded 0.0.

- [ ] **Step 3.3: Implement expression diff**

Replace the `let _ = (actual_expressions, ...)` line in `diff_pose` and the hardcoded zeros with real computation:

```rust
    // Expressions: preset + custom kept separate per spec. A missing
    // entry on either side is treated as weight 0 (the renderer doesn't
    // apply the expression). Pick the worst per category.
    let (preset_max, preset_worst) = max_expression_delta(
        &actual_expressions.presets,
        &reference_expressions.presets,
    );
    let (custom_max, custom_worst) = max_expression_delta(
        &actual_expressions.custom,
        &reference_expressions.custom,
    );

    // lookAt still pending — task 4.
    let _ = (actual_look_at, reference_look_at);
```

Then update the report-construction at the bottom:

```rust
    let expressions_pass =
        preset_max <= tolerances.per_preset_expression
            && custom_max <= tolerances.per_custom_expression;

    PoseDiffReport {
        per_bone_rotation_max_rad: per_bone_max,
        per_bone_rotation_worst_bone: worst_bone,
        hips_translation_m: hips,
        per_preset_expression_max_delta: preset_max,
        per_preset_expression_worst: preset_worst,
        per_custom_expression_max_delta: custom_max,
        per_custom_expression_worst: custom_worst,
        look_at_yaw_delta_deg: 0.0,
        look_at_pitch_delta_deg: 0.0,
        offset_from_head_bone_m: 0.0,
        overall_passed: humanoid_pass && expressions_pass,
    }
```

And add this helper function inside the module (next to `quaternion_geodesic_rad`):

```rust
fn max_expression_delta(
    actual: &std::collections::BTreeMap<String, f32>,
    reference: &std::collections::BTreeMap<String, f32>,
) -> (f32, Option<String>) {
    let mut keys: std::collections::BTreeSet<&String> = actual.keys().collect();
    keys.extend(reference.keys());

    let mut max = 0.0_f32;
    let mut worst: Option<String> = None;
    for k in keys {
        let a = actual.get(k).copied().unwrap_or(0.0);
        let r = reference.get(k).copied().unwrap_or(0.0);
        let d = (a - r).abs();
        if d > max {
            max = d;
            worst = Some(k.clone());
        }
    }
    (max, worst)
}
```

- [ ] **Step 3.4: Verify tests pass**

Run: `cargo test -p vrm-diff-engine pose_diff`
Expected: all 6 tests pass (4 existing + 2 new).

- [ ] **Step 3.5: Commit**

```bash
git add crates/vrm-diff-engine/src/pose_diff.rs
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): expression weight diff (preset + custom separate)

Missing-entry treated as weight 0 (renderer doesn't apply that
expression). Preset and custom kept structurally separate per VRMA
spec; either category can independently fail tolerance.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: LookAt diff (yaw + pitch + offset)

**Files:**
- Modify: `crates/vrm-diff-engine/src/pose_diff.rs`

- [ ] **Step 4.1: Write the failing test**

Add to `mod tests`:

```rust
    #[test]
    fn look_at_yaw_delta_exceeds_tolerance() {
        let actual_look = DumpLookAtStateResult {
            gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
            yaw_deg: 30.0,
            pitch_deg: 0.0,
            applied_via: LookAtAppliedVia::Bone,
            offset_from_head_bone: [0.0, 0.06, 0.0],
        };
        let ref_look = DumpLookAtStateResult {
            gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
            yaw_deg: 0.0,  // 30° delta — exceeds 1° default
            pitch_deg: 0.0,
            applied_via: LookAtAppliedVia::Bone,
            offset_from_head_bone: [0.0, 0.06, 0.0],
        };
        let bones = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &bones,
            &bones,
            &empty_expressions(),
            &empty_expressions(),
            &actual_look,
            &ref_look,
            &PoseTolerances::default(),
        );
        assert!(!report.overall_passed);
        assert!((report.look_at_yaw_delta_deg - 30.0).abs() < 1e-5);
        assert_eq!(report.look_at_pitch_delta_deg, 0.0);
    }

    #[test]
    fn offset_from_head_bone_diff() {
        let look_a = DumpLookAtStateResult {
            gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            applied_via: LookAtAppliedVia::Bone,
            offset_from_head_bone: [0.0, 0.06, 0.0],
        };
        let look_b = DumpLookAtStateResult {
            gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            applied_via: LookAtAppliedVia::Bone,
            offset_from_head_bone: [0.0, 0.065, 0.0],  // 5mm — exceeds 1mm default
        };
        let bones = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &bones,
            &bones,
            &empty_expressions(),
            &empty_expressions(),
            &look_a,
            &look_b,
            &PoseTolerances::default(),
        );
        assert!(!report.overall_passed);
        assert!((report.offset_from_head_bone_m - 0.005).abs() < 1e-5);
    }
```

- [ ] **Step 4.2: Verify tests fail**

Run: `cargo test -p vrm-diff-engine pose_diff::tests::look_at`
Expected: FAIL — current code hardcodes lookAt to zero.

- [ ] **Step 4.3: Implement lookAt diff**

Replace the `let _ = (actual_look_at, reference_look_at)` line with:

```rust
    // LookAt: yaw and pitch independent abs-deltas in degrees;
    // offsetFromHeadBone is Euclidean distance.
    let yaw_delta = (actual_look_at.yaw_deg - reference_look_at.yaw_deg).abs();
    let pitch_delta = (actual_look_at.pitch_deg - reference_look_at.pitch_deg).abs();
    let offset_delta = euclidean_distance(
        &actual_look_at.offset_from_head_bone,
        &reference_look_at.offset_from_head_bone,
    );
```

Update the report-construction:

```rust
    let look_at_pass =
        yaw_delta <= tolerances.look_at_yaw_pitch_degrees
            && pitch_delta <= tolerances.look_at_yaw_pitch_degrees
            && offset_delta <= tolerances.offset_from_head_bone_m;

    PoseDiffReport {
        per_bone_rotation_max_rad: per_bone_max,
        per_bone_rotation_worst_bone: worst_bone,
        hips_translation_m: hips,
        per_preset_expression_max_delta: preset_max,
        per_preset_expression_worst: preset_worst,
        per_custom_expression_max_delta: custom_max,
        per_custom_expression_worst: custom_worst,
        look_at_yaw_delta_deg: yaw_delta,
        look_at_pitch_delta_deg: pitch_delta,
        offset_from_head_bone_m: offset_delta,
        overall_passed: humanoid_pass && expressions_pass && look_at_pass,
    }
```

- [ ] **Step 4.4: Verify tests pass**

Run: `cargo test -p vrm-diff-engine`
Expected: all 8 pose_diff tests pass + existing tests still pass.

Run: `cargo clippy -p vrm-diff-engine --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4.5: Commit**

```bash
git add crates/vrm-diff-engine/src/pose_diff.rs
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): lookAt diff (yaw, pitch, offsetFromHeadBone)

Yaw and pitch are independent abs-deltas; either exceeding tolerance
fails the channel. offsetFromHeadBone is Euclidean. Closes the
pose_diff module — all 4 channels (bones, hips, expressions, lookAt)
now contribute to overall_passed.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Test plan schema — VrmaAnimation + PoseTolerance

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs`

- [ ] **Step 5.1: Write the failing parse test**

Tests in `vrm-test-plan` live in the same file under `#[cfg(test)] mod tests`. Find that block and append:

```rust
    #[test]
    fn plan_with_vrma_animation_parses() {
        let yaml = r#"
id: test_vrma_humanoid
vrm: /tmp/x.vrm
animation:
  vrma: /tmp/x.vrma
  apply_at_time: 0.5
diff:
  reference_renderer: univrm
  threshold: 0.95
  pose_tolerance:
    per_bone_quaternion_radians: 0.020
    hips_translation_m: 0.010
    per_preset_expression: 0.010
    per_custom_expression: 0.010
    look_at_yaw_pitch_degrees: 2.0
    offset_from_head_bone_m: 0.002
camera:
  position: [0.0, 1.5, 1.2]
  target: [0.0, 1.5, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_deg: 30.0
output:
  width: 1024
  height: 1024
"#;
        let plan: TestPlan = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(plan.id, "test_vrma_humanoid");
        let vrma = plan.animation.as_ref().unwrap().vrma.as_ref().unwrap();
        assert_eq!(vrma.path, "/tmp/x.vrma");
        assert!((vrma.apply_at_time - 0.5).abs() < 1e-5);
        let tol = plan.diff.pose_tolerance.as_ref().unwrap();
        assert!((tol.per_bone_quaternion_radians - 0.020).abs() < 1e-5);
    }

    #[test]
    fn plan_without_vrma_animation_parses_with_default() {
        // Existing plans without an animation.vrma block must keep parsing.
        let yaml = r#"
id: existing_plan
vrm: /tmp/x.vrm
diff:
  reference_renderer: univrm
  threshold: 0.95
camera:
  position: [0.0, 1.5, 1.2]
  target: [0.0, 1.5, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_deg: 30.0
output:
  width: 1024
  height: 1024
"#;
        let plan: TestPlan = serde_yaml::from_str(yaml).unwrap();
        assert!(plan.animation.is_none());
        assert!(plan.diff.pose_tolerance.is_none());
    }
```

The exact field names (e.g. `camera:`, `output:`) need to match the existing `TestPlan` schema fields — check `crates/vrm-test-plan/src/lib.rs` for the canonical YAML shape used in other test plans (e.g. look at any existing test in the file or any plan in `test-plans/manual/humanoid/`).

- [ ] **Step 5.2: Verify tests fail**

Run: `cargo test -p vrm-test-plan plan_with_vrma`
Expected: FAIL with `unknown field 'vrma'` or `unknown field 'pose_tolerance'`.

- [ ] **Step 5.3: Add the new types and extend AnimationConfig + Diff**

Edit `crates/vrm-test-plan/src/lib.rs`. Find the `AnimationConfig` struct (around line 49) and extend it with a `vrma` field. Find the `Diff` struct (around line 148) and extend it with `pose_tolerance`. Add two new types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VrmaAnimation {
    /// Path to the `.vrma` file.
    pub path: String,
    /// Sample time in seconds for `apply_vrma_at_time`.
    pub apply_at_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoseTolerance {
    pub per_bone_quaternion_radians: f32,
    pub hips_translation_m: f32,
    pub per_preset_expression: f32,
    pub per_custom_expression: f32,
    pub look_at_yaw_pitch_degrees: f32,
    pub offset_from_head_bone_m: f32,
}
```

Modify `AnimationConfig` to add the `vrma` field:

```rust
pub struct AnimationConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_transform: Option<RootTransformAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma: Option<VrmaAnimation>,
}
```

Modify `Diff` to add the `pose_tolerance` field:

```rust
pub struct Diff {
    pub reference_renderer: String,
    pub threshold: f32,
    // ... existing fields ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose_tolerance: Option<PoseTolerance>,
    pub conformance_status: ConformanceStatus,
    // ... other existing fields ...
}
```

(Preserve any existing fields in `Diff` — only ADD `pose_tolerance`; don't rewrite the whole struct from scratch.)

The `vrma` field of `VrmaAnimation` may collide with the struct name in some renderers — if so, rename the field to `vrma_path` and the struct to `VrmaClip` instead. The test uses `vrma.path` and `vrma.apply_at_time`, so just keep the struct field as `path` (a struct named `VrmaAnimation` having a field called `path` is fine — common Rust pattern).

- [ ] **Step 5.4: Verify tests pass**

Run: `cargo test -p vrm-test-plan`
Expected: all pass including the two new tests.

Run: `cargo clippy -p vrm-test-plan --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5.5: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(vrm-test-plan): add VrmaAnimation + PoseTolerance schema fields

AnimationConfig gains optional vrma block; Diff gains optional
pose_tolerance block. Both serde-default to None so existing plans
parse unchanged. Mirrors the design spec's test plan schema example.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Manifest fields — vrma_url + vrma_blake3

**Files:**
- Modify: `crates/vrm-s3/src/manifest.rs`

- [ ] **Step 6.1: Write the failing test**

Find the existing test for `positions_url`/`positions_blake3` in `crates/vrm-s3/src/manifest.rs` (around line 100 — the existing `e` constructions with `positions_url: None`). Add a new test next to it:

```rust
    #[test]
    fn manifest_entry_roundtrips_vrma_url() {
        let e = ManifestEntry {
            test_id: "vrma_humanoid_x".into(),
            renderer_name: "univrm".into(),
            renderer_version: "v0.131.0".into(),
            git_hash: "abc".into(),
            host: SubmissionMetadata {
                os: "darwin".into(),
                os_version: "26.0".into(),
                gpu_vendor: "Apple".into(),
                gpu_model: "M4 Max".into(),
            },
            image_url: "s3://b/x.png".into(),
            blake3: "blake3:img".into(),
            width: 1024,
            height: 1024,
            positions_url: None,
            positions_blake3: None,
            vrma_url: Some("s3://b/x.vrma".into()),
            vrma_blake3: Some("blake3:vrma".into()),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.vrma_url.as_deref(), Some("s3://b/x.vrma"));
        assert_eq!(back.vrma_blake3.as_deref(), Some("blake3:vrma"));
        assert!(json.contains(r#""vrma_url":"s3://b/x.vrma""#));
    }

    #[test]
    fn manifest_entry_omits_vrma_fields_when_none() {
        let e = ManifestEntry {
            test_id: "mtoon_default".into(),
            renderer_name: "univrm".into(),
            renderer_version: "v0.131.0".into(),
            git_hash: "abc".into(),
            host: SubmissionMetadata {
                os: "darwin".into(),
                os_version: "26.0".into(),
                gpu_vendor: "Apple".into(),
                gpu_model: "M4 Max".into(),
            },
            image_url: "s3://b/x.png".into(),
            blake3: "blake3:img".into(),
            width: 1024,
            height: 1024,
            positions_url: None,
            positions_blake3: None,
            vrma_url: None,
            vrma_blake3: None,
        };
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert!(v.get("vrma_url").is_none());
        assert!(v.get("vrma_blake3").is_none());
    }
```

- [ ] **Step 6.2: Verify tests fail**

Run: `cargo test -p vrm-s3 manifest_entry_roundtrips_vrma`
Expected: FAIL with `unknown field 'vrma_url'` or struct construction error.

- [ ] **Step 6.3: Add the fields**

Find the existing `pub struct ManifestEntry` definition (around line 10). After the `positions_blake3` field, add:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_blake3: Option<String>,
```

Any other `ManifestEntry { ... }` construction in the crate (look in `push_pull.rs` and existing tests) may need the new fields added with `: None`. Search for `ManifestEntry {` and fix every site.

- [ ] **Step 6.4: Verify tests pass**

Run: `cargo test -p vrm-s3`
Expected: all pass.

Run: `cargo clippy -p vrm-s3 --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6.5: Commit**

```bash
git add crates/vrm-s3/
git commit -m "$(cat <<'EOF'
feat(vrm-s3): add vrma_url + vrma_blake3 to ManifestEntry

Optional fields mirroring the existing positions_url / positions_blake3
pattern. .vrma files in the goldens corpus are content-addressed and
follow the same S3 ↔ manifest flow as .png and dump_bone_positions
output.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Runner ExecuteOptions + ExecuteResult extensions

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs`

- [ ] **Step 7.1: Find the ExecuteOptions and ExecuteResult structs**

Run: `grep -nE "^pub struct Execute(Options|Result)" crates/vrm-runner/src/execute.rs`
Expected: shows the line numbers of both structs.

- [ ] **Step 7.2: Add new fields to ExecuteOptions**

Add these fields to `ExecuteOptions`:

```rust
    /// If set, the runner loads the .vrma and calls apply_vrma_at_time
    /// before render. None means no VRMA application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_path: Option<String>,

    /// Sample time for apply_vrma_at_time. Ignored when vrma_path is None.
    #[serde(default)]
    pub apply_at_time: f32,

    /// Optional reference pose vector for pose_diff. If None, runner
    /// captures the pose from this run only (no diff computed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_pose_json: Option<String>,
```

Adapt placement / serde attributes to match the existing struct's style — if existing fields don't use serde attributes (because this struct is not deserialized from JSON), drop the serde attrs. Just preserve the field-shape.

- [ ] **Step 7.3: Add new fields to ExecuteResult**

Add to `ExecuteResult`:

```rust
    /// Pose-vector diff against the reference, if a reference_pose_json
    /// was provided to ExecuteOptions. None otherwise.
    pub pose_diff: Option<vrm_diff_engine::pose_diff::PoseDiffReport>,
```

- [ ] **Step 7.4: Update existing callers if needed**

Search for `ExecuteOptions {` and `ExecuteResult {` across the workspace:

```bash
grep -rn "ExecuteOptions {" crates/vrm-runner/src/ | head -10
grep -rn "ExecuteResult {" crates/vrm-runner/src/ | head -10
```

Add the new fields with their defaults (`vrma_path: None, apply_at_time: 0.0, reference_pose_json: None` for options; `pose_diff: None` for result) at every existing call site.

- [ ] **Step 7.5: Verify**

Run: `cargo build -p vrm-runner`
Expected: SUCCESS — all callers updated.

Run: `cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 7.6: Commit**

```bash
git add crates/vrm-runner/src/
git commit -m "$(cat <<'EOF'
feat(vrm-runner): ExecuteOptions/ExecuteResult VRMA fields

ExecuteOptions: vrma_path + apply_at_time + reference_pose_json.
ExecuteResult: pose_diff: Option<PoseDiffReport>.

No op sequencing yet — task 8 wires plan_to_ops to emit the 5 VRMA
ops when vrma_path is set; task 9 invokes diff_pose in diff.rs.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: plan_to_ops emits the VRMA op sequence

**Files:**
- Modify: `crates/vrm-runner/src/plan_to_ops.rs`

- [ ] **Step 8.1: Read the current plan_to_ops to understand its shape**

Run: `head -80 crates/vrm-runner/src/plan_to_ops.rs`
Expected: shows the existing function that translates a `TestPlan` into a sequence of op invocations.

- [ ] **Step 8.2: Add a failing test**

If `plan_to_ops.rs` has a `#[cfg(test)]` block at the bottom, add a test there. Otherwise add a `#[cfg(test)] mod tests {}` block.

```rust
#[cfg(test)]
mod vrma_tests {
    use super::*;
    use vrm_test_plan::*;

    fn minimal_plan_with_vrma() -> TestPlan {
        let yaml = r#"
id: test_vrma
vrm: /tmp/x.vrm
animation:
  vrma: /tmp/x.vrma
  apply_at_time: 0.5
diff:
  reference_renderer: univrm
  threshold: 0.95
camera:
  position: [0.0, 1.5, 1.2]
  target: [0.0, 1.5, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_deg: 30.0
output:
  width: 1024
  height: 1024
"#;
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn plan_with_vrma_emits_load_apply_dump_sequence() {
        let plan = minimal_plan_with_vrma();
        let ops = plan_to_ops(&plan).unwrap();
        let op_names: Vec<&str> = ops.iter().map(|o| o.method_name()).collect();

        // The exact sequence we expect after set_post_processing:
        let load_idx = op_names.iter().position(|m| *m == "load_vrma").expect("load_vrma");
        let apply_idx = op_names.iter().position(|m| *m == "apply_vrma_at_time").expect("apply_vrma_at_time");
        let dump_humanoid_idx = op_names.iter().position(|m| *m == "dump_humanoid_pose").expect("dump_humanoid_pose");
        let dump_expr_idx = op_names.iter().position(|m| *m == "dump_expression_weights").expect("dump_expression_weights");
        let dump_lookat_idx = op_names.iter().position(|m| *m == "dump_look_at_state").expect("dump_look_at_state");
        let render_idx = op_names.iter().position(|m| *m == "render").expect("render");

        assert!(load_idx < apply_idx);
        assert!(apply_idx < dump_humanoid_idx);
        assert!(apply_idx < dump_expr_idx);
        assert!(apply_idx < dump_lookat_idx);
        assert!(dump_humanoid_idx < render_idx);
        assert!(dump_expr_idx < render_idx);
        assert!(dump_lookat_idx < render_idx);
    }
}
```

The exact API of `plan_to_ops` (e.g. whether it returns a `Vec<Op>` or `Vec<(String, JsonValue)>` etc.) varies — read the existing function and adapt the test's call shape to match. The assertion shape (sequence ordering) is what matters; the `method_name()` accessor may need to be a method on the existing op enum, or the test can match against tuple/struct fields directly.

- [ ] **Step 8.3: Verify test fails**

Run: `cargo test -p vrm-runner plan_with_vrma_emits`
Expected: FAIL — current `plan_to_ops` doesn't emit VRMA ops.

- [ ] **Step 8.4: Implement the emission**

Modify `plan_to_ops` to insert the VRMA op sequence between `set_post_processing` and `reset_physics`/`render` whenever `plan.animation.as_ref().and_then(|a| a.vrma.as_ref()).is_some()`. The exact insertion shape depends on the existing code structure — typically it'll look like:

```rust
if let Some(vrma) = plan.animation.as_ref().and_then(|a| a.vrma.as_ref()) {
    ops.push(Op::LoadVrma(LoadVrmaParams {
        vrma_path: vrma.path.clone(),
    }));
    ops.push(Op::ApplyVrmaAtTime(ApplyVrmaAtTimeParams {
        session_id: session_id.clone(),
        vrma_handle: VRMA_HANDLE_PLACEHOLDER,  // filled by execute.rs based on adapter's load_vrma response
        vrm_handle: VRM_HANDLE_PLACEHOLDER,
        time_seconds: vrma.apply_at_time,
    }));
    ops.push(Op::DumpHumanoidPose(DumpHumanoidPoseParams { session_id: session_id.clone() }));
    ops.push(Op::DumpExpressionWeights(DumpExpressionWeightsParams { session_id: session_id.clone() }));
    ops.push(Op::DumpLookAtState(DumpLookAtStateParams { session_id: session_id.clone() }));
}
```

The actual `Op` enum / dispatch shape will dictate exact syntax. Read existing code and follow its pattern.

- [ ] **Step 8.5: Verify test passes**

Run: `cargo test -p vrm-runner plan_with_vrma_emits`
Expected: PASS.

- [ ] **Step 8.6: Commit**

```bash
git add crates/vrm-runner/src/plan_to_ops.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): plan_to_ops emits VRMA op sequence

When animation.vrma is set, plan_to_ops emits load_vrma →
apply_vrma_at_time → dump_humanoid_pose → dump_expression_weights →
dump_look_at_state in that order, after set_post_processing and before
reset_physics/render. Matches the op sequence from the design spec.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: diff.rs invokes pose_diff alongside SSIM

**Files:**
- Modify: `crates/vrm-runner/src/diff.rs`

- [ ] **Step 9.1: Find the existing diff orchestration**

Run: `head -80 crates/vrm-runner/src/diff.rs`
Expected: shows the function that takes an `ExecuteResult` and a reference, runs SSIM, and assembles the final diff report.

- [ ] **Step 9.2: Wire pose_diff in**

When `reference_pose_json` is provided in `ExecuteOptions`, parse it as a struct containing `humanoid: DumpHumanoidPoseResult`, `expressions: DumpExpressionWeightsResult`, `look_at: DumpLookAtStateResult`. Compare against the matching dump_* responses captured during the run, using `PoseTolerances` from the plan's `pose_tolerance` field (defaulting to `PoseTolerances::default()` if absent).

The exact code shape depends on existing diff.rs structure — read it and follow the existing SSIM-orchestration pattern. The skeleton:

```rust
// After SSIM is computed:
let pose_diff = if let Some(ref_path) = &opts.reference_pose_json {
    let ref_text = std::fs::read_to_string(ref_path)
        .map_err(|e| ExecuteError::Io(format!("reading reference_pose_json {ref_path}: {e}")))?;
    let reference: ReferencePoseFixture = serde_json::from_str(&ref_text)
        .map_err(|e| ExecuteError::Parse(format!("reference_pose_json {ref_path}: {e}")))?;
    let tolerances = plan.diff.pose_tolerance
        .as_ref()
        .map(pose_tolerance_to_pose_tolerances)
        .unwrap_or_default();
    Some(diff_pose(
        &captured.humanoid,
        &reference.humanoid,
        &captured.expressions,
        &reference.expressions,
        &captured.look_at,
        &reference.look_at,
        &tolerances,
    ))
} else {
    None
};

// Final assemble:
ExecuteResult {
    ssim,
    ssim_passed,
    pose_diff,
    overall_passed: ssim_passed && pose_diff.as_ref().map(|p| p.overall_passed).unwrap_or(true),
    // ...
}
```

`ReferencePoseFixture` is a new type — define it locally in `diff.rs` or alongside in a new module:

```rust
#[derive(Debug, Deserialize)]
struct ReferencePoseFixture {
    humanoid: DumpHumanoidPoseResult,
    expressions: DumpExpressionWeightsResult,
    look_at: DumpLookAtStateResult,
}
```

`pose_tolerance_to_pose_tolerances` is a one-line `From`-equivalent conversion between `vrm_test_plan::PoseTolerance` and `vrm_diff_engine::pose_diff::PoseTolerances` (same fields).

- [ ] **Step 9.3: Add a test fixture and test**

Create `crates/vrm-runner/tests/vrma_diff_fixture.json` (or wherever test fixtures live in the runner crate):

```json
{
  "humanoid": {
    "bones": [],
    "hips_translation": [0.0, 0.0, 0.0]
  },
  "expressions": {
    "presets": { "happy": 0.5 },
    "custom": {}
  },
  "look_at": {
    "gaze_direction_quat": [0.0, 0.0, 0.0, 1.0],
    "yaw_deg": 0.0,
    "pitch_deg": 0.0,
    "applied_via": "off",
    "offset_from_head_bone": [0.0, 0.0, 0.0]
  }
}
```

Then a test in the runner that takes this fixture as the reference and a constructed `ExecuteResult` with matching dumps, verifies `pose_diff` is `Some(report)` with `overall_passed: true`.

- [ ] **Step 9.4: Verify**

Run: `cargo test -p vrm-runner pose_diff`
Expected: PASS.

Run: `cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 9.5: Commit**

```bash
git add crates/vrm-runner/
git commit -m "$(cat <<'EOF'
feat(vrm-runner): wire pose_diff alongside SSIM in diff orchestration

When ExecuteOptions.reference_pose_json is set, the runner parses the
reference fixture, applies the plan's pose_tolerance (or defaults),
and invokes diff_pose. The resulting PoseDiffReport lands in
ExecuteResult.pose_diff. overall_passed gates on both SSIM and
pose_diff outcomes when pose_diff is Some.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: CLI flags — --vrma, --apply-at-time, --reference-pose-json

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs`

- [ ] **Step 10.1: Find the existing flags**

Run: `grep -nE "^\s*--(reference|plan|adapter)" crates/vrm-runner/src/cli.rs | head -10`
Expected: shows how existing flags are declared (likely clap derives or builders).

- [ ] **Step 10.2: Add three new flags**

Add to the existing `execute-test-plan` subcommand args:

```rust
    /// Path to a .vrma file. If set, the runner loads it and calls
    /// apply_vrma_at_time(t) before render.
    #[arg(long = "vrma")]
    pub vrma: Option<String>,

    /// Sample time for apply_vrma_at_time. Defaults to 0.0.
    #[arg(long = "apply-at-time", default_value = "0.0")]
    pub apply_at_time: f32,

    /// Path to a JSON fixture with reference humanoid pose + expressions
    /// + lookAt state. Enables pose_diff in the result.
    #[arg(long = "reference-pose-json")]
    pub reference_pose_json: Option<String>,
```

Adapt to match the existing clap derive style — if the crate uses `#[derive(Parser)]` and the subcommand args are in a separate struct, follow that pattern.

Map these into `ExecuteOptions` when constructing it inside the subcommand handler:

```rust
let opts = ExecuteOptions {
    // ... existing fields ...
    vrma_path: args.vrma,
    apply_at_time: args.apply_at_time,
    reference_pose_json: args.reference_pose_json,
};
```

- [ ] **Step 10.3: Update describe catalog if needed**

If `cli.rs` includes the `describe` subcommand that lists supported flags or methods, ensure the new flags appear there (search for `describe` blocks in `cli.rs` and add as appropriate). If the describe catalog references just method names (not CLI flags), no change needed.

- [ ] **Step 10.4: Verify**

Run: `cargo build -p vrm-runner`
Expected: SUCCESS.

Run: `cargo run -p vrm-runner -- execute-test-plan --help 2>&1 | grep -E "(vrma|apply-at-time|reference-pose-json)"`
Expected: shows all three new flags in the help output.

Run: `cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 10.5: Commit**

```bash
git add crates/vrm-runner/src/cli.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): add --vrma, --apply-at-time, --reference-pose-json CLI flags

Surfaces ExecuteOptions VRMA fields on the execute-test-plan
subcommand. CLI consumers can now drive a .vrma through any real
adapter once asset generation (phase 3) and adapter wiring (phases
4-5) land.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Integration smoke test — VRMA test plan end-to-end through mock renderer

**Files:**
- Modify: `crates/vrm-mock-renderer/src/...` (if needed; mock renderer must answer the 5 VRMA ops with deterministic fixtures)
- Create: `test-plans/manual/vrma/synthetic_vrma_smoke.test.yaml`

- [ ] **Step 11.1: Add mock renderer support for the 5 VRMA ops**

Find the mock renderer's method dispatch (likely `crates/vrm-mock-renderer/src/main.rs` or similar). Add handlers for the 5 VRMA ops that return deterministic fixed values:

```rust
"load_vrma" => Ok(json!({
    "vrma_handle": 1,
    "channel_summary": {
        "humanoid_bones": 15,
        "expressions": 1,
        "has_look_at": true,
        "duration_seconds": 1.0
    }
})),
"apply_vrma_at_time" => Ok(json!({
    "channels_applied": {
        "humanoid_bones": 15,
        "expressions": 1,
        "look_at": true
    }
})),
"dump_humanoid_pose" => Ok(json!({
    "bones": [
        { "name": "head", "local_rotation_quat": [0.0, 0.0, 0.0, 1.0] },
        { "name": "leftUpperArm", "local_rotation_quat": [0.0, 0.0, 0.0, 1.0] }
    ],
    "hips_translation": [0.0, 0.0, 0.0]
})),
"dump_expression_weights" => Ok(json!({
    "presets": { "happy": 0.0 },
    "custom": {}
})),
"dump_look_at_state" => Ok(json!({
    "gaze_direction_quat": [0.0, 0.0, 0.0, 1.0],
    "yaw_deg": 0.0,
    "pitch_deg": 0.0,
    "applied_via": "off",
    "offset_from_head_bone": [0.0, 0.0, 0.0]
})),
```

- [ ] **Step 11.2: Author a smoke-test plan**

Create `test-plans/manual/vrma/synthetic_vrma_smoke.test.yaml`:

```yaml
id: synthetic_vrma_smoke
vrm: /tmp/synthetic.vrm  # not actually loaded by mock renderer
animation:
  vrma: /tmp/synthetic.vrma  # not actually loaded by mock renderer
  apply_at_time: 0.5
diff:
  reference_renderer: vrm-mock-renderer
  threshold: 0.95
  pose_tolerance:
    per_bone_quaternion_radians: 0.010
    hips_translation_m: 0.005
    per_preset_expression: 0.005
    per_custom_expression: 0.005
    look_at_yaw_pitch_degrees: 1.0
    offset_from_head_bone_m: 0.001
  conformance_status:
    kind: included
camera:
  position: [0.0, 1.5, 1.2]
  target: [0.0, 1.5, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_deg: 30.0
output:
  width: 256
  height: 256
```

- [ ] **Step 11.3: Run the plan end-to-end against mock renderer**

```bash
cargo build --release -p vrm-mock-renderer -p vrm-runner
target/release/vrm-runner execute-test-plan \
    --plan test-plans/manual/vrma/synthetic_vrma_smoke.test.yaml \
    --adapter-bin target/release/vrm-mock-renderer \
    --asset-dir /tmp \
    --output-dir /tmp/vrma-smoke \
    --renderer-name vrm-mock-renderer \
    --json
```

Expected: JSON output shows `ok: true`, `pose_diff: null` (no reference fixture passed), and the runner successfully called the 5 VRMA ops on the mock renderer.

- [ ] **Step 11.4: Commit**

```bash
git add crates/vrm-mock-renderer/ test-plans/manual/vrma/
git commit -m "$(cat <<'EOF'
test: VRMA smoke test plan + mock renderer fixtures

Mock renderer answers the 5 VRMA ops with deterministic fixtures.
End-to-end smoke runs the runner through load_vrma → apply_vrma_at_time
→ dump_* → render with no real adapter present. Verifies the runner
plumbing (plan parse → op emission → execute → diff) works in
isolation, before phases 4-5 wire up real adapter implementations.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Workspace fmt + clippy + test pass

**Files:** none directly; cleanup pass.

- [ ] **Step 12.1: Run cargo fmt --all**

Run: `cargo fmt --all`
Expected: no changes (or rustfmt applies minor wrapping/style fixes).

- [ ] **Step 12.2: Run cargo clippy with all-targets**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings, zero errors.

- [ ] **Step 12.3: Run cargo test workspace**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 12.4: Commit if fixes landed**

If fmt/clippy made changes:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt + clippy clean-up after VRMA phase 2

Final workspace pass after VRMA phase 2 (diff engine + test plan schema
+ manifest + runner integration). Zero clippy warnings, zero fmt diffs.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If no changes, skip this commit. Report **DONE — no fmt/clippy changes**.

---

## Phase 2 completion checklist

- [ ] `pose_diff.rs` in vrm-diff-engine exports `PoseDiffReport`, `PoseTolerances`, `diff_pose` with full 4-channel implementation (bones, hips, expressions, lookAt) and 8 tests passing
- [ ] `vrm-test-plan` parses `animation.vrma { path, apply_at_time }` and `diff.pose_tolerance { ... }` blocks
- [ ] `ManifestEntry` carries optional `vrma_url` + `vrma_blake3` fields
- [ ] `vrm-runner` `ExecuteOptions` accepts `vrma_path`, `apply_at_time`, `reference_pose_json`; `ExecuteResult` carries `pose_diff: Option<PoseDiffReport>`
- [ ] `plan_to_ops` emits `load_vrma → apply_vrma_at_time → dump_humanoid_pose → dump_expression_weights → dump_look_at_state` when `animation.vrma` is set
- [ ] `diff.rs` invokes `diff_pose` when `reference_pose_json` is set; gates `overall_passed` on both SSIM and pose-diff
- [ ] CLI exposes `--vrma`, `--apply-at-time`, `--reference-pose-json` flags
- [ ] Mock renderer answers all 5 VRMA ops with deterministic fixtures
- [ ] Smoke test plan runs end-to-end through mock renderer with `ok: true`
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` green

After this phase, the entire runner-side substrate is in place. Phase 3 will add the asset generator (`emit-vrma-humanoid-sweep`, `emit-vrma-expression-sweep`, `emit-vrma-lookat-sweep`) producing the real .vrma corpus. Phases 4-5 wire real adapter implementations (UniVRM, three-vrm). Phase 6 lands manual humanoid plans, bootstrap, findings, and upstream issues.
