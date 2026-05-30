# CCD (tunneling) Conformance Coverage — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect spring-bone↔collider tunneling under fast relative motion as a renderer-independent pass/fail, via per-frame joint-position capture + an absolute geometric non-penetration metric + a parametric CCD sweep.

**Architecture:** Reuse `animate_root_transform` (new fast regime) + a world-fixed collider so the chain sweeps through it. Capture per-frame joint positions by extending the existing `render_sequence` result (additive). Score with `vrm_diff_engine::penetration` (signed distance to the suite-authored collider; `max_penetration_depth ≤ ε` passes). Absolute metric ⇒ no oracle; UniVRM is a peer subject.

**Tech Stack:** Rust workspace (vrm-ops, vrm-diff-engine, vrm-runner, vrm-asset-generator, vrm-test-plan); adapter capture in Swift / TypeScript / GDScript / C#. CI gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`.

**Design doc:** `docs/superpowers/specs/2026-05-30-ccd-conformance-design.md`

**Key existing types (verified):**
- `vrm_ops::tools::SpringPositions { name: String, joint_positions: Vec<[f32;3]> }`
- `vrm_ops::tools::SequenceFrame { index: u32, timestamp_seconds: f32, path: String, blake3: String }`
- `vrm_ops::tools::RenderSequenceParams { session_id, width, height, output_dir, frame_count, frame_hz, physics_dt_seconds, color_space, msaa, output_type, output_format, animate_root_transform: Option<RootTransformAnimation>, apply_vrma: Option<VrmaPlaybackSpec> }`
- `vrm_ops::tools::RenderSequenceResult { frames: Vec<SequenceFrame>, duration_seconds, actual_color_space, frame_hz_achieved, muxed_path: Option<..> }`
- `vrm_diff_engine::positions::diff_positions` — the engine module pattern to mirror.

---

## File Structure

- `crates/vrm-ops/src/tools.rs` — `RenderSequenceParams.capture_positions: bool`; `SequenceFrame.spring_positions: Option<Vec<SpringPositions>>`.
- `crates/vrm-diff-engine/src/penetration.rs` (new) + `lib.rs` mod decl — `ColliderSpec`, signed distance, `PenetrationReport`, `worst_penetration`.
- `crates/vrm-test-plan/src/lib.rs` — `TestPlan.ccd_colliders: Option<Vec<ColliderWorldSpec>>` (world-fixed collider geometry the metric reads).
- `crates/vrm-asset-generator/src/sweep.rs` — `spring_bone_ccd_sweep()`.
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_spring_bone_ccd_test_plan()`.
- `crates/vrm-asset-generator/src/cli.rs` — `EmitSpringboneCcdSweep` subcommand.
- `crates/vrm-runner/src/execute.rs` — thread `capture_positions`; persist `<id>_<renderer>_positions.json`.
- `crates/vrm-runner/src/penetration_diff.rs` (new) + `cli.rs` — `penetration-diff` subcommand.
- `crates/vrm-mock-renderer/` — emit per-frame positions when requested (reference adapter; unblocks Rust-only E2E).
- `adapters/{vrm-metal-kit,three-vrm,godot-vrm,univrm}` — populate per-frame positions in their `render_sequence` loop.
- `docs/methodology.md`, `docs/findings.md`.

---

# PHASE 1 — Per-frame position capture (op contract + mock + runner)

## Task 1: Extend the op types (`capture_positions` + per-frame positions)

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs` (`SequenceFrame` ~line 392, `RenderSequenceParams` ~line 422)
- Test: same file `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Add to the tests module in `tools.rs`:

```rust
#[test]
fn render_sequence_params_capture_positions_defaults_false_and_roundtrips() {
    // Legacy JSON without the field deserializes to false.
    let legacy = r#"{"session_id":"s","width":4,"height":4,"output_dir":"/tmp",
        "frame_count":2,"frame_hz":30.0,"physics_dt_seconds":0.016666668,
        "color_space":"Linear","msaa":1,"output_type":"Color","output_format":"PngSequence"}"#;
    let p: RenderSequenceParams = serde_json::from_str(legacy).unwrap();
    assert!(!p.capture_positions);
}

#[test]
fn sequence_frame_positions_optional_roundtrip() {
    let f = SequenceFrame {
        index: 0,
        timestamp_seconds: 0.0,
        path: "0000.png".into(),
        blake3: "blake3:00".into(),
        spring_positions: Some(vec![SpringPositions {
            name: "chain".into(),
            joint_positions: vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]],
        }]),
    };
    let s = serde_json::to_string(&f).unwrap();
    let back: SequenceFrame = serde_json::from_str(&s).unwrap();
    assert_eq!(back.spring_positions, f.spring_positions);
    // omitted when None
    let f2 = SequenceFrame { spring_positions: None, ..f.clone() };
    assert!(!serde_json::to_string(&f2).unwrap().contains("spring_positions"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vrm-ops render_sequence_params_capture_positions`
Expected: FAIL (no field `capture_positions` / `spring_positions`).

- [ ] **Step 3: Add the fields**

In `RenderSequenceParams`, after `apply_vrma`:
```rust
    /// When true, the adapter additionally reports per-frame spring-bone joint
    /// world positions on each `SequenceFrame.spring_positions`. Default false
    /// so existing sequence tests are byte-unaffected. Used by CCD penetration
    /// tests. Adapters that cannot report positions MAY leave it null per frame.
    #[serde(default)]
    pub capture_positions: bool,
```
In `SequenceFrame`, after `blake3`:
```rust
    /// Per-spring joint world positions at this frame (metres), present only
    /// when `RenderSequenceParams.capture_positions` was set and the adapter
    /// supports it. Same shape as `dump_bone_positions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring_positions: Option<Vec<SpringPositions>>,
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vrm-ops` → PASS. Fix any other construction sites of `SequenceFrame` that now need `spring_positions: None` (search `SequenceFrame {`): `cargo build -p vrm-ops` will name them.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vrm-ops && cargo clippy -p vrm-ops --all-targets -- -D warnings
git add crates/vrm-ops/src/tools.rs
git commit -m "feat(ops): render_sequence capture_positions + per-frame spring_positions"
```

## Task 2: Mock renderer emits per-frame positions

**Files:**
- Modify: `crates/vrm-mock-renderer/src/` (the `render_sequence` handler)
- Test: a crate test asserting positions present when requested

The mock is the deterministic reference adapter; making it emit positions unblocks full Rust-only E2E of Phases 2–3 without a GPU.

- [ ] **Step 1: Locate the mock's render_sequence handler.** Run `rg -n "render_sequence|SequenceFrame|fn .*sequence" crates/vrm-mock-renderer/src`. Read how it builds each `SequenceFrame` and how it models the chain (it already produces deterministic spring-bone state for `dump_bone_positions` — find that path too: `rg -n "dump_bone_positions|SpringPositions|joint_positions" crates/vrm-mock-renderer/src`).

- [ ] **Step 2: Write the failing test** (in the mock crate, driving its handler):

```rust
#[test]
fn render_sequence_includes_positions_when_requested() {
    // Construct RenderSequenceParams with capture_positions = true (frame_count 3),
    // call the mock's render_sequence handler, assert every frame has
    // spring_positions = Some(non-empty) and joint_positions length == chain joints.
    // (Mirror the crate's existing render_sequence test setup.)
}
```

- [ ] **Step 3: Implement.** In the mock's per-frame loop, when `params.capture_positions`, compute the same deterministic joint positions the mock already produces for `dump_bone_positions` **at that frame's simulated time** and set `frame.spring_positions = Some(...)`. When false, leave `None` (byte-unchanged).

- [ ] **Step 4: Run** `cargo test -p vrm-mock-renderer` → PASS.

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vrm-mock-renderer && cargo clippy -p vrm-mock-renderer --all-targets -- -D warnings
git add crates/vrm-mock-renderer
git commit -m "feat(mock): emit per-frame spring positions when capture_positions set"
```

## Task 3: Runner threads `capture_positions` and persists positions JSON

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs` (the `render_sequence` dispatch in `execute_plan` ~line 333 and in `execute_plan_capturing_positions` ~line 597; the `render_sequence_params(...)` builder)
- Test: `crates/vrm-runner/tests/` integration test using the mock

- [ ] **Step 1: Read the dispatch.** `rg -n "render_sequence_params|RenderSequenceParams|render_sequence\"|rehash_frames" crates/vrm-runner/src/execute.rs`. Find where `RenderSequenceParams` is constructed and where the result frames are post-processed (`rehash_frames`).

- [ ] **Step 2: Write the failing integration test** (drive the mock through a sequence plan with capture):

```rust
// Build/load a plan with a render_sequence block; set capture on; run execute_plan
// with the mock adapter bin; assert a positions JSON exists at
// <output_dir>/<plan_id>_mock_positions.json and parses to per-frame positions.
```

- [ ] **Step 3: Implement.**
  - In `render_sequence_params(...)`, set `capture_positions` from a new field on the plan's `render_sequence` block (Task 7 adds it; until then thread a bool param). Pragmatic: add `capture_positions: bool` to `RenderSequenceBlock` in `vrm-test-plan` here (Task 7 wires the sidecar to set it) and read it in the params builder.
  - After the `render_sequence` result returns, if any frame carries `spring_positions`, serialize `[{ frame_index, timestamp_seconds, springs }]` to `<output_dir>/<plan_id>_<renderer>_positions.json`. Keep frames' PNG handling unchanged.

- [ ] **Step 4: Run** `cargo test -p vrm-runner` (with `cargo build --release -p vrm-mock-renderer` first if the test spawns the bin) → PASS.

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vrm-runner && cargo clippy -p vrm-runner --all-targets -- -D warnings
git add crates/vrm-runner crates/vrm-test-plan
git commit -m "feat(runner): thread capture_positions; persist per-frame positions JSON"
```

---

# PHASE 2 — Penetration metric (pure Rust, CI-gated)

## Task 4: `vrm_diff_engine::penetration`

**Files:**
- Create: `crates/vrm-diff-engine/src/penetration.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs` (add `pub mod penetration;`)
- Test: in-module `#[cfg(test)]`

- [ ] **Step 1: Write the failing tests** (create the file with tests first):

```rust
//! Absolute non-penetration metric for spring-bone vs world-fixed colliders.
//! A conformant solver keeps joints outside the collider surface. Tunneling =
//! a joint inside the surface beyond tolerance on any captured frame.
use serde::{Deserialize, Serialize};
use vrm_ops::tools::SpringPositions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColliderSpec {
    /// World-space sphere.
    Sphere { center: [f32; 3], radius: f32 },
    /// World-space capsule between two endpoints.
    Capsule { a: [f32; 3], b: [f32; 3], radius: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PenetrationReport {
    pub max_penetration_depth_m: f32,
    pub epsilon_m: f32,
    pub worst_frame: usize,
    pub worst_spring: usize,
    pub worst_joint: usize,
    pub passed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sp(joints: Vec<[f32;3]>) -> SpringPositions { SpringPositions { name: "c".into(), joint_positions: joints } }

    #[test]
    fn joint_outside_sphere_passes() {
        let c = ColliderSpec::Sphere { center: [0.0,0.0,0.0], radius: 0.05 };
        let frames = vec![vec![sp(vec![[0.10,0.0,0.0]])]]; // 0.10 from center, r=0.05 → outside
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(r.passed);
        assert!(r.max_penetration_depth_m <= 0.0);
    }

    #[test]
    fn joint_inside_sphere_beyond_epsilon_fails_and_locates() {
        let c = ColliderSpec::Sphere { center: [0.0,0.0,0.0], radius: 0.05 };
        // frame0 fine; frame1 joint at 0.02 from center → 0.03 m penetration
        let frames = vec![
            vec![sp(vec![[0.10,0.0,0.0]])],
            vec![sp(vec![[0.10,0.0,0.0],[0.02,0.0,0.0]])],
        ];
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.03).abs() < 1e-5);
        assert_eq!(r.worst_frame, 1);
        assert_eq!(r.worst_joint, 1);
    }

    #[test]
    fn shallow_penetration_within_epsilon_passes() {
        let c = ColliderSpec::Sphere { center: [0.0,0.0,0.0], radius: 0.05 };
        let frames = vec![vec![sp(vec![[0.049,0.0,0.0]])]]; // 1 mm in, ε=2 mm
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(r.passed);
    }

    #[test]
    fn capsule_distance_is_to_segment() {
        // capsule along Y from (0,-0.1,0) to (0,0.1,0), r=0.03; point at (0.02,0,0)
        // distance to segment = 0.02 → penetration 0.01
        let c = ColliderSpec::Capsule { a: [0.0,-0.1,0.0], b: [0.0,0.1,0.0], radius: 0.03 };
        let frames = vec![vec![sp(vec![[0.02,0.0,0.0]])]];
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.01).abs() < 1e-5);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vrm-diff-engine penetration` → FAIL (`worst_penetration` undefined). Add `pub mod penetration;` to `lib.rs` first so it compiles to the failure.

- [ ] **Step 3: Implement** (append to `penetration.rs`):

```rust
fn signed_distance(p: [f32; 3], c: &ColliderSpec) -> f32 {
    match c {
        ColliderSpec::Sphere { center, radius } => dist(p, *center) - radius,
        ColliderSpec::Capsule { a, b, radius } => dist_point_segment(p, *a, *b) - radius,
    }
}
fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (a[0]-b[0], a[1]-b[1], a[2]-b[2]);
    (dx*dx + dy*dy + dz*dz).sqrt()
}
fn dist_point_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = [b[0]-a[0], b[1]-a[1], b[2]-a[2]];
    let ap = [p[0]-a[0], p[1]-a[1], p[2]-a[2]];
    let ab2 = ab[0]*ab[0] + ab[1]*ab[1] + ab[2]*ab[2];
    let t = if ab2 <= 0.0 { 0.0 } else {
        ((ap[0]*ab[0] + ap[1]*ab[1] + ap[2]*ab[2]) / ab2).clamp(0.0, 1.0)
    };
    let proj = [a[0]+ab[0]*t, a[1]+ab[1]*t, a[2]+ab[2]*t];
    dist(p, proj)
}

/// `frames[f]` = the per-spring positions captured at frame f. Computes the
/// worst (deepest) penetration of any joint into any collider over all frames.
/// `max_penetration_depth_m` = max(0, −min signed_distance); passes iff that
/// depth ≤ `epsilon_m`.
pub fn worst_penetration(
    frames: &[Vec<SpringPositions>],
    colliders: &[ColliderSpec],
    epsilon_m: f32,
) -> PenetrationReport {
    let mut deepest = 0.0_f32; // penetration depth (positive = inside)
    let (mut wf, mut ws, mut wj) = (0usize, 0usize, 0usize);
    for (fi, springs) in frames.iter().enumerate() {
        for (si, spring) in springs.iter().enumerate() {
            for (ji, &p) in spring.joint_positions.iter().enumerate() {
                for c in colliders {
                    let depth = -signed_distance(p, c); // >0 means inside
                    if depth > deepest {
                        deepest = depth;
                        wf = fi; ws = si; wj = ji;
                    }
                }
            }
        }
    }
    PenetrationReport {
        max_penetration_depth_m: deepest,
        epsilon_m,
        worst_frame: wf,
        worst_spring: ws,
        worst_joint: wj,
        passed: deepest <= epsilon_m,
    }
}
```

- [ ] **Step 4: Run** `cargo test -p vrm-diff-engine penetration` → PASS (4 tests).

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vrm-diff-engine && cargo clippy -p vrm-diff-engine --all-targets -- -D warnings
git add crates/vrm-diff-engine/src/penetration.rs crates/vrm-diff-engine/src/lib.rs
git commit -m "feat(diff): penetration metric (signed distance to world-fixed collider)"
```

## Task 5: `ColliderWorldSpec` on TestPlan + `penetration-diff` runner subcommand

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs` (add `ccd_colliders: Option<Vec<ColliderWorldSpec>>` to `TestPlan`; define `ColliderWorldSpec` mirroring `ColliderSpec`)
- Create: `crates/vrm-runner/src/penetration_diff.rs`
- Modify: `crates/vrm-runner/src/cli.rs` (subcommand `PenetrationDiff`)
- Test: runner unit test

- [ ] **Step 1: Add `ColliderWorldSpec`** to `vrm-test-plan` (serde, `#[serde(default)]` on the `TestPlan` field so existing plans parse). It carries the same sphere/capsule world geometry; provide `impl From<&ColliderWorldSpec> for vrm_diff_engine::penetration::ColliderSpec` in the runner (or a small mapper) — keep `vrm-test-plan` free of a `vrm-diff-engine` dependency by converting in the runner.

- [ ] **Step 2: Write the failing test** for the conversion + a `penetration-diff` that reads a positions JSON + the plan's colliders and returns the report. 

- [ ] **Step 3: Implement** `penetration_diff.rs`: load the positions JSON (`Vec<{frame_index, springs}>` from Task 3) → `Vec<Vec<SpringPositions>>`; map `plan.ccd_colliders` → `Vec<ColliderSpec>`; call `worst_penetration`; print `PenetrationReport` as JSON. Add the `PenetrationDiff { --positions, --plan, --epsilon (default 0.002), --json }` subcommand; exit non-zero when `!passed` (mirror the `diff` subcommand's exit-gating).

- [ ] **Step 4: Run** `cargo test -p vrm-runner penetration` → PASS; `cargo build -p vrm-runner`.

- [ ] **Step 5: Commit**
```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/vrm-test-plan crates/vrm-runner
git commit -m "feat(runner): penetration-diff subcommand + ccd_colliders on TestPlan"
```

---

# PHASE 3 — CCD asset sweep (pure Rust, CI-gated)

## Task 6: `spring_bone_ccd_sweep()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs`
- Test: same file

- [ ] **Step 1: Write the failing test** — assert the sweep produces cells straddling the tunneling threshold: a thin collider (`radius 0.005`) with a fast sweep, and a thick collider (`0.05`) with a slow sweep; all use a `WorldCoordinates` collider; unique ids `ccd_*`; each carries the collider geometry needed for the metric.

```rust
#[test]
fn ccd_sweep_straddles_threshold_and_uses_world_collider() {
    let v = spring_bone_ccd_sweep();
    assert!(v.iter().all(|(_,s)| matches!(s.collider_groups.first(), Some(_))));
    assert!(v.iter().any(|(_,s)| s.id.contains("ccd_") && s.id.contains("thin")));
    assert!(v.iter().any(|(_,s)| s.id.contains("ccd_") && s.id.contains("thick")));
    let ids: std::collections::HashSet<_> = v.iter().map(|(_,s)| s.id.clone()).collect();
    assert_eq!(ids.len(), v.len(), "unique ids");
}
```
(Adapt field accessors to `SpringBoneSceneParams`'s actual shape — read `spring_bone_collider_sweep` first to mirror it.)

- [ ] **Step 2: Run** → FAIL (fn missing).

- [ ] **Step 3: Implement** `spring_bone_ccd_sweep()` mirroring `spring_bone_collider_sweep`, but: collider `attach: ColliderAttach::WorldCoordinates`, placed in the chain's swept column; axes = collider radius `{0.005, 0.02, 0.05}` × shape `{sphere, capsule}` × a speed tag carried for the sidecar (the actual fast/slow root motion is set in Task 7's sidecar, keyed off the id or a field). Bound to ~12 cells. Record the world collider geometry so Task 7 can emit it into `ccd_colliders`.

- [ ] **Step 4: Run** `cargo test -p vrm-asset-generator ccd_sweep` → PASS.

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vrm-asset-generator && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
git add crates/vrm-asset-generator/src/sweep.rs
git commit -m "feat(asset-gen): spring_bone_ccd_sweep (world collider, threshold-straddling)"
```

## Task 7: `build_spring_bone_ccd_test_plan` sidecar

**Files:**
- Modify: `crates/vrm-asset-generator/src/sidecar.rs` (mirror `build_spring_bone_swing_sequence_test_plan` ~line 219)
- Test: same file

- [ ] **Step 1: Write the failing test** — the emitted plan must: have a `render_sequence` block with `capture_positions: true`; a **fast** root animation (`translation_end ≈ [1.0,0,0]`, `duration ≈ 0.1 s` → per-substep ≫ collider radius); `tone_mapping: none`; populate `ccd_colliders` with the world collider geometry; and NOT have a single-frame `animation:` block.

- [ ] **Step 2: Run** → FAIL.

- [ ] **Step 3: Implement** `build_spring_bone_ccd_test_plan(params, asset_relpath, collider: ColliderWorldSpec, speed)`: start from the sequence plan builder; set the fast root motion + `capture_positions = true` on the `RenderSequenceBlock` (the field added in Task 3); set `plan.ccd_colliders = Some(vec![collider])`; `tone_mapping: None`. Confirm `TestPlan::validate` accepts `render_sequence` + `ccd_colliders` together (extend validate if it rejects the new field).

- [ ] **Step 4: Run** `cargo test -p vrm-asset-generator ccd` → PASS.

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vrm-asset-generator && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
git add crates/vrm-asset-generator/src/sidecar.rs
git commit -m "feat(asset-gen): CCD sidecar (fast root sweep + capture_positions + ccd_colliders)"
```

## Task 8: `emit-springbone-ccd-sweep` subcommand + E2E through the mock

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (mirror `EmitSpringboneColliderSweep`)
- Verify: end-to-end via the mock renderer + `penetration-diff`

- [ ] **Step 1: Add the subcommand + handler** (mirror the collider-sweep handler): iterate `spring_bone_ccd_sweep()`, emit each with `build_spring_bone_ccd_test_plan`, via the V1 collider emit path. Update the hand-maintained `describe` catalog (check `rg "EmitSpringboneColliderSweep|describe" crates/vrm-asset-generator/src/cli.rs`).

- [ ] **Step 2: Build + emit + render + score (the real proof):**
```bash
cargo build --release -p vrm-mock-renderer
cargo run -q -p vrm-asset-generator -- emit-springbone-ccd-sweep --output-dir /tmp/ccd
# render one fast cell through the mock with sequence + capture
cargo run -q -p vrm-runner -- execute-test-plan \
  --plan /tmp/ccd/ccd_sphere_r0p005_fast.test.yaml \
  --adapter-bin target/release/vrm-mock-renderer \
  --asset-dir /tmp/ccd --output-dir /tmp/ccd-out --renderer-name mock --json
# score penetration
cargo run -q -p vrm-runner -- penetration-diff \
  --positions /tmp/ccd-out/ccd_sphere_r0p005_fast_mock_positions.json \
  --plan /tmp/ccd/ccd_sphere_r0p005_fast.test.yaml --json
```
Expected: a positions JSON is produced, and `penetration-diff` emits a `PenetrationReport`. (The mock's spring model may or may not "tunnel" — the proof here is that the pipeline produces a real penetration number end-to-end, not the verdict.)

- [ ] **Step 3: Commit**
```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(cli): emit-springbone-ccd-sweep + mock E2E penetration pipeline"
```

---

# PHASE 4 — Adapters + cross-renderer run + findings

## Task 9: Per-adapter per-frame position capture

Each adapter already runs a per-frame physics loop inside `render_sequence` and already knows joint world positions (it implements `dump_bone_positions`). For each, when `params.capture_positions` is true, attach the per-frame joint positions to each frame's result. **Read the adapter's existing `render_sequence` implementation and its `dump_bone_positions` implementation first**, then wire the former to emit the latter's data per frame. The runner re-reads from `<id>_<renderer>_positions.json` (Task 3), so the only adapter requirement is: per-frame `spring_positions` populated in the `render_sequence` result.

- [ ] **9a — vrm-metal-kit (the target; Swift):** `adapters/vrm-metal-kit/`. Find the `render_sequence` handler and the spring-bone joint world-position accessor used by `dump_bone_positions`; emit positions per captured frame when requested. Verify: `swift build` then render a CCD cell, confirm `*_positions.json` non-empty with per-frame entries. This adapter carries the actual #306-class CCD signal.
- [ ] **9b — godot-vrm (GDScript + Rust shim):** same, in `adapters/godot-vrm/` + `crates/vrm-godot-shim`.
- [ ] **9c — three-vrm (TypeScript):** same, in `adapters/three-vrm/`.
- [ ] **9d — univrm (C#, batched):** best-effort. If the batch one-shot can emit per-frame positions, do it; otherwise leave `spring_positions: None` and record UniVRM as "no per-frame capture" — the absolute metric does not require it. Do NOT block on UniVRM.

Each 9x: small additive change; commit per adapter (`feat(<adapter>): per-frame spring positions in render_sequence`). Per CLAUDE.md, bump the VMK pinned revision deliberately if 9a requires an upstream change.

## Task 10: Methodology pin + cross-renderer run + findings

- [ ] **Step 1: Methodology** — append a section to `docs/methodology.md`: the fast-motion regime (per-substep-displacement vs collider-thickness rationale), the geometric non-penetration metric class (absolute, oracle-free, distinct from SSIM/consensus and from `diff_positions` drift), the `ε` tolerance + justification, determinism via 60 Hz fixed step, and "no golden — UniVRM is a peer subject." Commit.

- [ ] **Step 2: Run** the CCD sweep through every adapter that supports capture; `penetration-diff` per cell; **first confirm the ε gap empirically** (rest cells well under ε; fast/thin cells that tunnel well over) before trusting the threshold.

- [ ] **Step 3: Findings** — write a `docs/findings.md` entry: per-renderer `max_penetration_depth` across radius × speed (the tunneling-onset table), pass/fail per cell, and the verdict per renderer (where each solver begins tunneling). No oracle framing. Commit.

---

## Self-Review Notes

- **Spec coverage:** Phase 1 = capture extension (Tasks 1–3) ✓; Phase 2 = metric (Tasks 4–5) ✓; Phase 3 = sweep + sidecar + subcommand (Tasks 6–8) ✓; Phase 4 = adapters + run + findings + methodology (Tasks 9–10) ✓. Absolute-metric / no-oracle and world-fixed-collider decisions reflected throughout.
- **Type consistency:** `capture_positions: bool`, `SequenceFrame.spring_positions: Option<Vec<SpringPositions>>`, `ColliderSpec` (engine) vs `ColliderWorldSpec` (plan) with a runner-side mapper, `worst_penetration(frames, colliders, epsilon) -> PenetrationReport`, `<id>_<renderer>_positions.json` used consistently by Tasks 3/5/8.
- **Flagged investigation points (each with an `rg`):** mock render_sequence/positions handlers (Task 2), runner dispatch + `rehash_frames` (Task 3), `SpringBoneSceneParams` accessors (Task 6), `TestPlan::validate` acceptance of `render_sequence`+`ccd_colliders` (Task 7), `describe` catalog (Task 8), each adapter's render_sequence + dump_bone_positions (Task 9). These are real lookups, not placeholders.
- **Honest scope:** Tasks 1–8 are pure-Rust and CI-gated (full E2E via the mock). Task 9 is the adapter-contract lift across 4 languages — the integration gate. Task 10 is the VMK-facing deliverable. Phases 2–3 can be fully built and verified before any GPU adapter is touched.
