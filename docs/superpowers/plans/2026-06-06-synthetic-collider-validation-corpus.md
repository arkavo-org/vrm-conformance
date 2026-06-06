# Synthetic-Collider Validation Corpus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the conformance suite a quantitative, falsifiable measurement that VMK 0.17.0-rc.2's synthetic spring-bone colliders (#309) and swept collision (#313) measurably deflect a chain — via VMK augment-ON vs augment-OFF, with per-frame penetration depth into the dumped synthetic colliders.

**Architecture:** Five components. (1) A new wire type `SequenceCollider` + two new optional fields in `vrm-ops`. (2) A per-frame (moving-collider) penetration function in `vrm-diff-engine`. (3) Runner plumbing: thread an `augment_colliders` load flag and a `capture_synthetic_colliders` sequence flag, persist a per-frame `_colliders.json`, and extend `penetration-diff` with a `--colliders` source. (4) Swift adapter: honor the augment flag at load; dump synthetic colliders to world space per frame. (5) Generator: emit a humanoid + hair-chain asset whose chain enters the synthetic-collider region under a fast root translation. Signal = OFF penetrates, ON ≈ 0.

**Tech Stack:** Rust (workspace crates `vrm-ops`, `vrm-diff-engine`, `vrm-test-plan`, `vrm-runner`, `vrm-asset-generator`), Swift 6.2 (VRMMetalKit adapter, macOS 26 / Xcode 26).

**Spec:** `docs/superpowers/specs/2026-06-06-synthetic-collider-validation-corpus-design.md`

**Design correction vs spec:** `animate_root_transform` supports **translation only** (no rotation). The swept excitation is therefore a **fast root translation** — inertial lag makes the head-attached synthetic collider overtake the lagging chain, the same relative-motion mechanism the existing CCD sweep uses against a world-fixed collider. Everywhere the spec says "fast root rotation", read "fast root translation".

---

## Task 1: Step-0 spike — confirm augmentation fires for the parametric humanoid (throwaway, gating)

**Goal:** Prove VMK generates synthetic colliders for a *generated* humanoid asset before building anything. If it doesn't, switch the corpus to the `AvatarSample_A_1.0` fixture.

**Files:** none committed (throwaway).

- [ ] **Step 1: Emit a humanoid spring-bone asset**

```bash
cargo run -p vrm-asset-generator -- emit-springbone-sweep --output-dir /tmp/aug-spike
ls /tmp/aug-spike/*.vrm | head -1
```

- [ ] **Step 2: Build the adapter with physics logging**

```bash
cd adapters/vrm-metal-kit && swift build -Xswiftc -DVRM_METALKIT_ENABLE_DEBUG_PHYSICS 2>&1 | tail -2
```

- [ ] **Step 3: Drive a render and read the collider dump**

The physics-debug build prints `[SpringBone DEBUG] === Colliders ===` with each collider's `group=` index after spring-bone setup. Synthetic colliders carry the reserved group index `min(authoredGroupCount, 31)`. Drive one render and capture stdout:

```bash
cd /Users/arkavo/Projects/vrm-conformance
ADAPTER=adapters/vrm-metal-kit/.build/debug/vrm-metal-kit-adapter
ID=$(basename $(ls /tmp/aug-spike/*.vrm | head -1) .vrm)
# The runner errors on debug stdout framing; we only need the adapter's printed dump.
cargo run -q -p vrm-runner -- execute-test-plan \
  --plan /tmp/aug-spike/$ID.test.yaml --adapter-bin "$ADAPTER" \
  --asset-dir /tmp/aug-spike --output-dir /tmp/aug-spike-out --renderer-name vmk 2>&1 \
  | grep -iE "=== Colliders ===|Spheres:|Capsules:|group=" | head -30
```

Expected: ≥1 sphere or capsule with a `group=` equal to the reserved synthetic index (i.e. more colliders than the asset authored — the spring-bone sweep authors none, so any sphere/capsule present is synthetic).

- [ ] **Step 4: Decide the asset source**

- If synthetic colliders appear → **parametric humanoid path** (Task 11 emits the asset).
- If none appear → **fixture path**: Task 11 uses `assets/humanoid/avatarA_1_0.vrm` instead of a generated asset; the plan references it and the corpus is documented as fixture-backed. (No other task changes.)

- [ ] **Step 5: Restore the clean adapter build**

```bash
cd adapters/vrm-metal-kit && swift build 2>&1 | tail -1
```

No commit (spike is throwaway).

---

## Task 2: vrm-ops wire types (augment flag + synthetic-collider capture)

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs` (`LoadVrmParams` ~13, `SequenceFrame` ~392, `RenderSequenceParams` ~427; add `SequenceCollider` near `SpringPositions` ~163)

- [ ] **Step 1: Write failing tests**

Add to the existing `mod ccd_capture_positions_tests` at the bottom of `crates/vrm-ops/src/tools.rs`:

```rust
    #[test]
    fn load_vrm_params_augment_colliders_optional_defaults_none() {
        let legacy = r#"{"path":"/tmp/a.vrm"}"#;
        let p: LoadVrmParams = serde_json::from_str(legacy).unwrap();
        assert_eq!(p.augment_colliders, None);
    }

    #[test]
    fn sequence_collider_sphere_and_capsule_roundtrip() {
        let s = SequenceCollider::Sphere { center: [0.1, 1.2, 0.0], radius: 0.05 };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"type\":\"sphere\""));
        assert_eq!(serde_json::from_str::<SequenceCollider>(&json).unwrap(), s);
        let c = SequenceCollider::Capsule { a: [0.0, 0.0, 0.0], b: [0.0, 0.1, 0.0], radius: 0.03 };
        assert_eq!(
            serde_json::from_str::<SequenceCollider>(&serde_json::to_string(&c).unwrap()).unwrap(),
            c
        );
    }

    #[test]
    fn render_sequence_params_capture_synthetic_colliders_defaults_false() {
        let legacy = r#"{"session_id":"s","width":4,"height":4,"output_dir":"/tmp",
            "frame_count":2,"frame_hz":30.0,"physics_dt_seconds":0.016666668,
            "color_space":"Linear","msaa":1,"output_type":"Color","output_format":"png_sequence"}"#;
        let p: RenderSequenceParams = serde_json::from_str(legacy).unwrap();
        assert!(!p.capture_synthetic_colliders);
    }

    #[test]
    fn sequence_frame_synthetic_colliders_optional_skips_when_none() {
        let f = SequenceFrame {
            index: 0,
            timestamp_seconds: 0.0,
            path: "0000.png".into(),
            blake3: "blake3:00".into(),
            spring_positions: None,
            synthetic_colliders: None,
        };
        assert!(!serde_json::to_string(&f).unwrap().contains("synthetic_colliders"));
    }
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-ops --lib`
Expected: FAIL — `augment_colliders`, `SequenceCollider`, `capture_synthetic_colliders`, `synthetic_colliders` do not exist.

- [ ] **Step 3: Add the type + fields**

Add `augment_colliders` to `LoadVrmParams` (replace the existing struct at ~13):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmParams {
    pub path: String,
    /// Renderer-specific: when `Some(false)`, ask the adapter to load the
    /// model WITHOUT synthesizing bone-derived spring-bone colliders (VMK
    /// #309). `None` = adapter default (VMK augments by default). Adapters
    /// that don't synthesize colliders ignore this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub augment_colliders: Option<bool>,
}
```

Add `SequenceCollider` immediately after the `SpringPositions` struct (~163):

```rust
/// A world-space spring-bone collider captured at one frame. Mirrors
/// `vrm_test_plan::ColliderWorldSpec`; lives here so adapters can report
/// per-frame (moving) synthetic colliders on `SequenceFrame`. The runner
/// maps this to `vrm_diff_engine::penetration::ColliderSpec`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SequenceCollider {
    Sphere { center: [f32; 3], radius: f32 },
    Capsule { a: [f32; 3], b: [f32; 3], radius: f32 },
}
```

Add `synthetic_colliders` to `SequenceFrame` (after the `spring_positions` field, ~403):

```rust
    /// Per-frame world-space synthetic colliders (VMK #309), present only when
    /// `RenderSequenceParams.capture_synthetic_colliders` was set and the
    /// adapter generated any. Bone-attached colliders move per frame, so this
    /// is captured alongside `spring_positions`. Empty when augmentation is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthetic_colliders: Option<Vec<SequenceCollider>>,
```

Add `capture_synthetic_colliders` to `RenderSequenceParams` (after `capture_positions`, ~448):

```rust
    /// When true, the adapter additionally reports per-frame world-space
    /// synthetic colliders on each `SequenceFrame.synthetic_colliders`.
    /// Default false. Adapters without synthetic colliders leave it null.
    #[serde(default)]
    pub capture_synthetic_colliders: bool,
```

- [ ] **Step 4: Fix the two existing struct literals that now miss `synthetic_colliders`**

The existing test `sequence_frame_positions_optional_roundtrip` constructs `SequenceFrame { ... }` twice (~482, ~495). Add `synthetic_colliders: None,` to the first literal; the second uses `..f.clone()` and needs no change.

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vrm-ops --lib`
Expected: PASS (all, including the new four).

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-ops/src/tools.rs
git commit -m "feat(ops): augment_colliders load flag + per-frame SequenceCollider capture wire"
```

---

## Task 3: diff-engine per-frame (moving-collider) penetration

**Files:**
- Modify: `crates/vrm-diff-engine/src/penetration.rs`

- [ ] **Step 1: Write failing test**

Add inside the existing `mod tests` in `penetration.rs`:

```rust
    #[test]
    fn per_frame_collider_moves_with_frame() {
        // Joint fixed at x=0.10. Collider sphere (r=0.05) sits at x=0.20 in
        // frame 0 (joint outside) and sweeps to x=0.12 in frame 1 (joint 0.02
        // inside the surface → 0.03 penetration).
        let frames = vec![
            vec![sp(vec![[0.10, 0.0, 0.0]])],
            vec![sp(vec![[0.10, 0.0, 0.0]])],
        ];
        let colliders_per_frame = vec![
            vec![ColliderSpec::Sphere { center: [0.20, 0.0, 0.0], radius: 0.05 }],
            vec![ColliderSpec::Sphere { center: [0.12, 0.0, 0.0], radius: 0.05 }],
        ];
        let r = worst_penetration_per_frame(&frames, &colliders_per_frame, 0.002);
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.03).abs() < 1e-5);
        assert_eq!(r.worst_frame, 1);
    }

    #[test]
    fn per_frame_empty_colliders_for_a_frame_is_skipped() {
        let frames = vec![vec![sp(vec![[0.0, 0.0, 0.0]])], vec![sp(vec![[0.0, 0.0, 0.0]])]];
        let colliders_per_frame = vec![
            vec![], // no colliders this frame
            vec![ColliderSpec::Sphere { center: [0.0, 0.0, 0.0], radius: 0.05 }],
        ];
        let r = worst_penetration_per_frame(&frames, &colliders_per_frame, 0.002);
        assert!(!r.passed); // frame 1 penetrates by 0.05
        assert_eq!(r.worst_frame, 1);
    }
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-diff-engine penetration`
Expected: FAIL — `worst_penetration_per_frame` not found.

- [ ] **Step 3: Implement**

Add after `worst_penetration` (after line ~92) in `penetration.rs`:

```rust
/// Like [`worst_penetration`] but the colliders move per frame: `frames[i]`
/// joints are tested only against `colliders_per_frame[i]`. Iterates the
/// shorter of the two lengths. Used for bone-attached (synthetic) colliders
/// captured alongside positions. Empty input passes with depth 0.
pub fn worst_penetration_per_frame(
    frames: &[Vec<SpringPositions>],
    colliders_per_frame: &[Vec<ColliderSpec>],
    epsilon_m: f32,
) -> PenetrationReport {
    let mut deepest = 0.0_f32;
    let (mut wf, mut ws, mut wj) = (0usize, 0usize, 0usize);
    let n = frames.len().min(colliders_per_frame.len());
    for fi in 0..n {
        for (si, spring) in frames[fi].iter().enumerate() {
            for (ji, &p) in spring.joint_positions.iter().enumerate() {
                for c in &colliders_per_frame[fi] {
                    let depth = -signed_distance(p, c);
                    if depth > deepest {
                        deepest = depth;
                        wf = fi;
                        ws = si;
                        wj = ji;
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

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vrm-diff-engine penetration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-diff-engine/src/penetration.rs
git commit -m "feat(diff): worst_penetration_per_frame for moving (bone-attached) colliders"
```

---

## Task 4: test-plan `capture_synthetic_colliders` flag

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs` (`RenderSequenceBlock` ~168)

- [ ] **Step 1: Write failing test**

Add inside `mod render_sequence_tests` in `crates/vrm-test-plan/src/lib.rs`:

```rust
    #[test]
    fn render_sequence_block_capture_synthetic_colliders_defaults_false() {
        let yaml = "frame_count: 2\nframe_hz: 30.0\nphysics_dt_seconds: 0.016666668\noutput_format: png_sequence\n";
        let block: RenderSequenceBlock = serde_yml::from_str(yaml).unwrap();
        assert!(!block.capture_synthetic_colliders);
    }
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-test-plan render_sequence`
Expected: FAIL — `capture_synthetic_colliders` not found.

- [ ] **Step 3: Implement**

Add to `RenderSequenceBlock` after the `capture_positions` field (~186):

```rust
    /// When true, the runner sets `capture_synthetic_colliders = true` in
    /// `RenderSequenceParams` and persists the per-frame world-space synthetic
    /// colliders to `<output_dir>/<plan_id>_<renderer>_colliders.json`.
    /// Default false so existing sequence plans parse unchanged.
    #[serde(default)]
    pub capture_synthetic_colliders: bool,
```

- [ ] **Step 4: Fix existing `RenderSequenceBlock { ... }` literals**

`build_spring_bone_swing_sequence_test_plan` (`crates/vrm-asset-generator/src/sidecar.rs` ~225) and `build_spring_bone_ccd_test_plan` (~716) construct this struct. Add `capture_synthetic_colliders: false,` to both literals (Task 11 will set it true for the new builder). Also any in-crate test literals flagged by the compiler — add the field set to `false`.

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vrm-test-plan && cargo build -p vrm-asset-generator`
Expected: PASS / builds.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs crates/vrm-asset-generator/src/sidecar.rs
git commit -m "feat(test-plan): capture_synthetic_colliders flag on RenderSequenceBlock"
```

---

## Task 5: plan→ops threading of `capture_synthetic_colliders`

**Files:**
- Modify: `crates/vrm-runner/src/plan_to_ops.rs` (`render_sequence_params` ~80)

- [ ] **Step 1: Write failing test**

Add at the bottom of `crates/vrm-runner/src/plan_to_ops.rs` (create a `#[cfg(test)] mod` if none exists):

```rust
#[cfg(test)]
mod synthetic_collider_threading_tests {
    use super::*;

    #[test]
    fn capture_synthetic_colliders_projects_into_params() {
        let output = plan::Output {
            width: 8, height: 8,
            color_space: plan::ColorSpace::Srgb, msaa: 1,
        };
        let block = plan::RenderSequenceBlock {
            frame_count: 2, frame_hz: 30.0, physics_dt_seconds: 1.0 / 60.0,
            output_format: plan::SequenceFormat::PngSequence,
            animate_root_transform: None, apply_vrma: None,
            temporal_ssim_threshold: None,
            capture_positions: true,
            capture_synthetic_colliders: true,
        };
        let p = render_sequence_params("s", &output, &block, "/tmp".into());
        assert!(p.capture_synthetic_colliders);
    }
}
```

(If `plan::Output` has more fields, copy them from an existing `plan_to_ops` test or `vrm-test-plan`; match the current struct exactly.)

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-runner --lib capture_synthetic_colliders_projects`
Expected: FAIL — field not set / not in params.

- [ ] **Step 3: Implement**

In `render_sequence_params`, add to the returned `ops::RenderSequenceParams { ... }` (after `capture_positions: block.capture_positions,`):

```rust
        capture_synthetic_colliders: block.capture_synthetic_colliders,
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vrm-runner --lib capture_synthetic_colliders_projects`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/plan_to_ops.rs
git commit -m "feat(runner): project capture_synthetic_colliders into RenderSequenceParams"
```

---

## Task 6: runner persists `_colliders.json` + threads the augment flag

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs` (`ExecuteOptions` ~94, load_vrm call ~208, add `FrameCollidersEntry` + `persist_colliders_json`, call it ~421)

- [ ] **Step 1: Write failing test**

Add a fast (no-toolchain) test to `crates/vrm-runner/tests/` — create `crates/vrm-runner/tests/persist_colliders_json.rs`:

```rust
//! Fast test: the runner writes <id>_<renderer>_colliders.json when a sequence
//! result carries per-frame synthetic colliders. Uses a stub result, no adapter.

use vrm_runner::execute::{persist_colliders_json_for_test, FrameCollidersEntry};
use vrm_ops::tools::{RenderSequenceResult, SequenceCollider, SequenceFrame};
use vrm_ops::ColorSpace;

#[test]
fn writes_colliders_json_when_frames_carry_synthetic_colliders() {
    let dir = tempfile::tempdir().unwrap();
    let out = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let result = RenderSequenceResult {
        frames: vec![SequenceFrame {
            index: 0, timestamp_seconds: 0.0, path: "0.png".into(), blake3: "blake3:0".into(),
            spring_positions: None,
            synthetic_colliders: Some(vec![SequenceCollider::Sphere {
                center: [0.0, 1.0, 0.0], radius: 0.05,
            }]),
        }],
        duration_seconds: 0.0, actual_color_space: ColorSpace::Linear, frame_hz_achieved: 30.0,
        muxed_path: None,
    };
    persist_colliders_json_for_test(&result, &out, "synthcoll", "vmk").unwrap();
    let path = out.join("synthcoll_vmk_colliders.json");
    assert!(path.exists());
    let entries: Vec<FrameCollidersEntry> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].frame_index, 0);
    assert_eq!(entries[0].colliders.len(), 1);
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-runner --test persist_colliders_json`
Expected: FAIL — `FrameCollidersEntry` / `persist_colliders_json_for_test` not found.

- [ ] **Step 3: Implement the type, persistence, and a test shim**

In `crates/vrm-runner/src/execute.rs`, add after `FramePositionsEntry` (~146):

```rust
/// One entry per frame that carried `synthetic_colliders` data. Written to
/// `<output_dir>/<plan_id>_<renderer>_colliders.json` when
/// `render_sequence.capture_synthetic_colliders` is true. Consumed by
/// `penetration-diff --colliders`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrameCollidersEntry {
    pub frame_index: u32,
    pub colliders: Vec<ops::SequenceCollider>,
}
```

Add a private persistence fn next to `persist_positions_json` (~774), plus a public test shim:

```rust
/// Collect per-frame synthetic colliders and write
/// `<output_dir>/<plan_id>_<renderer>_colliders.json`. Only writes when at
/// least one frame carried colliders.
fn persist_colliders_json(
    seq_result: &SequenceExecuteResult,
    output_dir: &Utf8Path,
    plan_id: &str,
    renderer_name: &str,
) -> Result<()> {
    let frames = match seq_result.result.as_ref() {
        Some(r) => &r.frames,
        None => return Ok(()),
    };
    let entries: Vec<FrameCollidersEntry> = frames
        .iter()
        .filter_map(|f| {
            f.synthetic_colliders.as_ref().map(|c| FrameCollidersEntry {
                frame_index: f.index,
                colliders: c.clone(),
            })
        })
        .collect();
    if entries.is_empty() {
        return Ok(());
    }
    let path = output_dir.join(format!("{plan_id}_{renderer_name}_colliders.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&entries)?)?;
    Ok(())
}

/// Test-only shim so an integration test can exercise `persist_colliders_json`
/// against a hand-built `RenderSequenceResult` without spawning an adapter.
pub fn persist_colliders_json_for_test(
    result: &ops::RenderSequenceResult,
    output_dir: &Utf8Path,
    plan_id: &str,
    renderer_name: &str,
) -> Result<()> {
    let seq = SequenceExecuteResult {
        status: SequenceStatus::Ok,
        result: Some(result.clone()),
        unimplemented_phase: None,
        error_message: None,
    };
    persist_colliders_json(&seq, output_dir, plan_id, renderer_name)
}
```

Call `persist_colliders_json` right after the existing `persist_positions_json` call (~421):

```rust
        persist_positions_json(&seq_result, &opts.output_dir, &plan.id, &opts.renderer_name)?;
        persist_colliders_json(&seq_result, &opts.output_dir, &plan.id, &opts.renderer_name)?;
```

Add the augment field to `ExecuteOptions` (after `reference_pose_json`, ~116):

```rust
    /// Renderer-specific: forwarded to `load_vrm` as `augment_colliders`.
    /// `None` = adapter default. Used by the synthetic-collider corpus to
    /// render the same asset with VMK augmentation on vs off.
    pub augment_colliders: Option<bool>,
```

Thread it into the `load_vrm` call. The current call (~208) passes `LoadVrmParams`-equivalent; update both `execute_plan` and `execute_plan_capturing_positions` load sites to pass:

```rust
    let load: ops::LoadVrmResult = adapter
        .call(
            "load_vrm",
            ops::LoadVrmParams {
                path: asset_path.to_string(),
                augment_colliders: opts.augment_colliders,
            },
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
```

(If the existing call uses a `json!({...})` literal rather than `LoadVrmParams`, replace it with the typed struct above. Grep `"load_vrm"` in `execute.rs` to find every call site.)

- [ ] **Step 4: Fix all `ExecuteOptions { ... }` literals**

Grep for `ExecuteOptions {` across `crates/vrm-runner/` and `crates/vrm-runner/tests/` and add `augment_colliders: None,` to each (the existing capture-positions tests and `cli.rs` construct it).

```bash
grep -rln "ExecuteOptions {" crates/vrm-runner/
```

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vrm-runner --test persist_colliders_json && cargo build -p vrm-runner`
Expected: PASS / builds.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-runner/src/execute.rs crates/vrm-runner/tests/persist_colliders_json.rs crates/vrm-runner/src/cli.rs
git commit -m "feat(runner): persist per-frame _colliders.json + thread augment_colliders load flag"
```

---

## Task 7: runner CLI — `--augment-colliders` and `--colliders`

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` (`ExecuteTestPlan` ~18 + handler ~274; `PenetrationDiff` ~210 + handler ~815)

- [ ] **Step 1: Add the `--augment-colliders` arg to `ExecuteTestPlan`**

After the `reference_pose_json` arg (~56), add:

```rust
        /// Renderer-specific: forward to `load_vrm` as `augment_colliders`.
        /// `true`/`false` toggles VMK synthetic-collider augmentation (#309);
        /// omit for the adapter default. Used by the synthetic-collider corpus.
        #[arg(long = "augment-colliders")]
        augment_colliders: Option<bool>,
```

- [ ] **Step 2: Pass it through in the handler**

In the `Cmd::ExecuteTestPlan { .. }` match arm (~274), add `augment_colliders,` to the destructured fields and set it on the `ExecuteOptions { .. }` built there:

```rust
        augment_colliders,
```

(in the destructure), and in the `ExecuteOptions` literal:

```rust
            augment_colliders,
```

- [ ] **Step 3: Add the `--colliders` arg to `PenetrationDiff`**

After the `plan` arg (~216), add:

```rust
        /// Optional per-frame colliders JSON (`<id>_<renderer>_colliders.json`).
        /// When set, penetration is measured against these moving colliders
        /// instead of the plan's static `ccd_colliders`. Used for bone-attached
        /// (synthetic) colliders.
        #[arg(long, value_name = "PATH")]
        colliders: Option<Utf8PathBuf>,
```

- [ ] **Step 4: Pass it through in the handler**

In `Cmd::PenetrationDiff { .. }` (~815), add `colliders,` to the destructure and pass it to `run_penetration_diff` (Task 8 widens the signature):

```rust
            let result = crate::penetration_diff::run_penetration_diff(
                &positions, &plan, colliders.as_deref(), epsilon,
            )?;
```

- [ ] **Step 5: Verify it compiles (Task 8 finishes the signature)**

Run: `cargo build -p vrm-runner 2>&1 | tail -5`
Expected: a single error about `run_penetration_diff` arity — resolved by Task 8. (Do Task 8 before committing; commit together at the end of Task 8.)

---

## Task 8: `penetration-diff` consumes a per-frame collider source

**Files:**
- Modify: `crates/vrm-runner/src/penetration_diff.rs`

- [ ] **Step 1: Write failing test**

Add at the bottom of `crates/vrm-runner/src/penetration_diff.rs`:

```rust
#[cfg(test)]
mod per_frame_colliders_tests {
    use super::*;
    use crate::execute::{FrameCollidersEntry, FramePositionsEntry};
    use vrm_ops::tools::{SequenceCollider, SpringPositions};

    #[test]
    fn colliders_path_measures_against_moving_colliders() {
        let dir = tempfile::tempdir().unwrap();
        let d = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        // Joint fixed at x=0.10 across both frames.
        let positions = vec![
            FramePositionsEntry { frame_index: 0, timestamp_seconds: 0.0,
                springs: vec![SpringPositions { name: "hair".into(), joint_positions: vec![[0.10, 0.0, 0.0]] }] },
            FramePositionsEntry { frame_index: 1, timestamp_seconds: 0.033,
                springs: vec![SpringPositions { name: "hair".into(), joint_positions: vec![[0.10, 0.0, 0.0]] }] },
        ];
        // Collider sweeps from x=0.20 (joint outside) to x=0.12 (0.03 inside).
        let colliders = vec![
            FrameCollidersEntry { frame_index: 0,
                colliders: vec![SequenceCollider::Sphere { center: [0.20, 0.0, 0.0], radius: 0.05 }] },
            FrameCollidersEntry { frame_index: 1,
                colliders: vec![SequenceCollider::Sphere { center: [0.12, 0.0, 0.0], radius: 0.05 }] },
        ];
        let pos_path = d.join("x_vmk_positions.json");
        let col_path = d.join("x_vmk_colliders.json");
        std::fs::write(&pos_path, serde_json::to_string(&positions).unwrap()).unwrap();
        std::fs::write(&col_path, serde_json::to_string(&colliders).unwrap()).unwrap();

        // A plan path is still required by the signature but ccd_colliders is
        // ignored when --colliders is given; write a minimal valid plan.
        let plan_path = d.join("x.test.yaml");
        std::fs::write(&plan_path, MINIMAL_PLAN_YAML).unwrap();

        let r = run_penetration_diff(&pos_path, &plan_path, Some(&col_path), 0.002).unwrap();
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.03).abs() < 1e-5);
        assert_eq!(r.worst_frame_index, 1);
    }

    // Minimal plan that parses; ccd_colliders intentionally absent.
    const MINIMAL_PLAN_YAML: &str = r#"
id: x
spec_version: v1
spec_section: synthetic
asset: x.vrm
camera: { position: [0,0,1], target: [0,0,0], up: [0,1,0], fov_degrees: 30 }
lighting:
  directional: { direction: [0,-1,0], color: [1,1,1], intensity: 1.0 }
  ambient: { color: [1,1,1], intensity: 0.2 }
output: { width: 8, height: 8, color_space: srgb, msaa: 1 }
diff: { threshold: 0.9 }
"#;
}
```

(Adjust `MINIMAL_PLAN_YAML` to the exact required fields of `TestPlan` — copy from any `*.test.yaml` under `goldens-cache/_assets/` and strip to the required keys if the schema differs.)

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-runner --lib per_frame_colliders`
Expected: FAIL — arity / behavior mismatch.

- [ ] **Step 3: Widen `run_penetration_diff`**

Replace the signature and the collider-extraction block in `penetration_diff.rs`. New signature (~80):

```rust
pub fn run_penetration_diff(
    positions_json_path: &Utf8Path,
    plan_path: &Utf8Path,
    colliders_json_path: Option<&Utf8Path>,
    epsilon_m: f32,
) -> Result<PenetrationDiffResult> {
```

Keep the positions-loading block (lines ~85–101) unchanged. Then branch: when `colliders_json_path` is `Some`, use the per-frame path; else the existing static path. Replace the "Load plan / Extract and map colliders / Run penetration check" section (~103–135) with:

```rust
    // ── Load plan (still parsed; ccd_colliders used only in static mode) ──────
    let plan_raw = std::fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file {plan_path}"))?;
    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(&plan_raw)
        .with_context(|| format!("failed to parse test plan {plan_path}"))?;

    let report = if let Some(col_path) = colliders_json_path {
        // ── Per-frame (moving) colliders ─────────────────────────────────────
        use crate::execute::FrameCollidersEntry;
        let col_raw = std::fs::read_to_string(col_path)
            .with_context(|| format!("failed to read colliders file {col_path}"))?;
        let mut col_entries: Vec<FrameCollidersEntry> = serde_json::from_str(&col_raw)
            .with_context(|| format!("failed to parse colliders JSON {col_path}"))?;
        col_entries.sort_by_key(|e| e.frame_index);
        // Align colliders to positions by frame_index (positions already sorted).
        let by_index: std::collections::HashMap<u32, Vec<ColliderSpec>> = col_entries
            .into_iter()
            .map(|e| (e.frame_index, e.colliders.iter().map(to_collider_spec_seq).collect()))
            .collect();
        let colliders_per_frame: Vec<Vec<ColliderSpec>> = original_frame_indices
            .iter()
            .map(|fi| by_index.get(fi).cloned().unwrap_or_default())
            .collect();
        vrm_diff_engine::penetration::worst_penetration_per_frame(
            &frames, &colliders_per_frame, epsilon_m,
        )
    } else {
        // ── Static colliders from the plan ───────────────────────────────────
        let world_specs = plan.ccd_colliders.as_deref().unwrap_or(&[]);
        if world_specs.is_empty() {
            bail!("plan has no ccd_colliders and no --colliders given — cannot run penetration-diff");
        }
        let colliders: Vec<ColliderSpec> = world_specs.iter().map(to_collider_spec).collect();
        worst_penetration(&frames, &colliders, epsilon_m)
    };
```

Add a mapper for the new wire type (next to `to_collider_spec`, ~35):

```rust
/// Map a `vrm_ops::tools::SequenceCollider` (per-frame capture) to the engine
/// `ColliderSpec`. Structurally identical; lives in the runner per the
/// dependency-direction rule.
pub fn to_collider_spec_seq(c: &vrm_ops::tools::SequenceCollider) -> ColliderSpec {
    use vrm_ops::tools::SequenceCollider as S;
    match c {
        S::Sphere { center, radius } => ColliderSpec::Sphere { center: *center, radius: *radius },
        S::Capsule { a, b, radius } => ColliderSpec::Capsule { a: *a, b: *b, radius: *radius },
    }
}
```

Add the import at the top: `use vrm_diff_engine::penetration::worst_penetration_per_frame;` is reached via the fully-qualified call above, so no new `use` is strictly required; keep the existing `use vrm_diff_engine::penetration::{worst_penetration, ColliderSpec};`.

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vrm-runner --lib per_frame_colliders && cargo build -p vrm-runner`
Expected: PASS / builds (Task 7's CLI call now type-checks).

- [ ] **Step 5: Commit (Tasks 7 + 8 together)**

```bash
git add crates/vrm-runner/src/cli.rs crates/vrm-runner/src/penetration_diff.rs
git commit -m "feat(runner): penetration-diff --colliders (per-frame) + --augment-colliders CLI"
```

---

## Task 9: adapter honors `augment_colliders` at load (Swift)

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` (`handleLoadVrm` ~208, `blockingLoad` ~1495)

- [ ] **Step 1: Read `augment_colliders` in `handleLoadVrm`**

In `handleLoadVrm` (~208), after parsing `path`, read the optional flag (default true). Replace the `blockingLoad(url:device:)` call to pass it:

```swift
        let augment: Bool = {
            if case .object(let o) = params, case .bool(let b) = o["augment_colliders"] { return b }
            return true  // VMK default
        }()
        let url = URL(fileURLWithPath: path)
        switch blockingLoad(url: url, device: device, augmentColliders: augment) {
```

- [ ] **Step 2: Thread it through `blockingLoad`**

Replace `blockingLoad` (~1495):

```swift
    private func blockingLoad(url: URL, device: MTLDevice, augmentColliders: Bool = true) -> Result<VRMModel, Error> {
        let box = ResultBox()
        let sem = DispatchSemaphore(value: 0)
        Task {
            do {
                let options = VRMLoadingOptions(augmentSpringBoneColliders: augmentColliders)
                let model = try await VRMModel.load(from: url, device: device, options: options)
                box.value = .success(model)
            } catch {
                box.value = .failure(error)
            }
            sem.signal()
        }
        sem.wait()
        return box.value!
    }
```

- [ ] **Step 3: Build**

Run: `cd adapters/vrm-metal-kit && swift build 2>&1 | tail -2`
Expected: Build complete.

- [ ] **Step 4: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift
git commit -m "feat(vmk-adapter): honor augment_colliders flag in load_vrm"
```

---

## Task 10: adapter dumps per-frame synthetic colliders (Swift)

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` (`handleRenderSequence` ~866; capture block ~1090–1116)

- [ ] **Step 1: Read the `capture_synthetic_colliders` param**

Next to the existing `capturePositions` parse (~931), add:

```swift
        let captureSyntheticColliders: Bool = {
            if case .bool(let b) = obj["capture_synthetic_colliders"] { return b }
            return false
        }()
```

- [ ] **Step 2: Capture synthetic colliders per frame**

In the per-frame loop, after the `spring_positions` capture block (~1105), add:

```swift
            // Capture world-space synthetic colliders (VMK #309) for this frame.
            // They are bone-attached, so they move every frame; transform each
            // local shape by its node's worldMatrix. Empty when augmentation off.
            var frameColliders: [JSONValue] = []
            if captureSyntheticColliders, let springBone = session.model.springBone {
                let nodes = session.model.nodes
                for collider in springBone.syntheticColliders {
                    guard collider.node >= 0 && collider.node < nodes.count else { continue }
                    let m = nodes[collider.node].worldMatrix
                    func toWorld(_ p: SIMD3<Float>) -> SIMD3<Float> {
                        let w = m * SIMD4<Float>(p.x, p.y, p.z, 1.0)
                        return SIMD3<Float>(w.x, w.y, w.z)
                    }
                    func vec(_ p: SIMD3<Float>) -> JSONValue {
                        .array([.number(Double(p.x)), .number(Double(p.y)), .number(Double(p.z))])
                    }
                    switch collider.shape {
                    case .sphere(let offset, let radius):
                        frameColliders.append(.object([
                            "type": .string("sphere"),
                            "center": vec(toWorld(offset)),
                            "radius": .number(Double(radius)),
                        ]))
                    case .capsule(let offset, let radius, let tail):
                        frameColliders.append(.object([
                            "type": .string("capsule"),
                            "a": vec(toWorld(offset)),
                            "b": vec(toWorld(tail)),
                            "radius": .number(Double(radius)),
                        ]))
                    default:
                        continue  // planes/other shapes not part of synthetic set
                    }
                }
            }
```

Then add the field to `frameObj` (alongside the `spring_positions` insertion, ~1113):

```swift
            if captureSyntheticColliders {
                frameObj["synthetic_colliders"] = .array(frameColliders)
            }
```

> **Note (radius & scale):** `radius` is passed unscaled, assuming unit node scale (VRM 1.0 avatars are meter-scale with scale≈1). If Task 1's spike shows non-unit head/leg scale, multiply `radius` by the node's world scale magnitude. Document if applied.

> **Note (collider shape API):** confirm the enum case names against `VRMColliderShape` in the checkout (`.build/checkouts/VRMMetalKit/Sources/VRMMetalKit/Core/VRMTypes.swift`): `.sphere(offset:radius:)`, `.capsule(offset:radius:tail:)`. Match the exact associated-value labels the compiler expects.

- [ ] **Step 3: Build**

Run: `cd adapters/vrm-metal-kit && swift build 2>&1 | tail -2`
Expected: Build complete.

- [ ] **Step 4: Toolchain-gated integration test**

Create `crates/vrm-runner/tests/capture_synthetic_colliders_vmk.rs` (mirror `capture_positions_vmk.rs`): emit/point at a humanoid asset, set `capture_positions = true` and `capture_synthetic_colliders = true` on the plan's `render_sequence`, run with `augment_colliders: Some(true)`, assert `<id>_<r>_colliders.json` exists, is non-empty, and that collider geometry moves across frames under root translation. Add a second case with `augment_colliders: Some(false)` asserting **no** colliders file (or empty). Gate with `#[ignore = "requires Xcode 26 + macOS 26 + swift build + Metal GPU"]`.

Run (locally): `cargo test -p vrm-runner --test capture_synthetic_colliders_vmk -- --ignored --nocapture`
Expected: PASS — ON has moving colliders, OFF has none.

- [ ] **Step 5: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift crates/vrm-runner/tests/capture_synthetic_colliders_vmk.rs
git commit -m "feat(vmk-adapter): dump per-frame world-space synthetic colliders in render_sequence"
```

---

## Task 11: generator — humanoid + hair-chain corpus asset and plan

**Files:**
- Modify: `crates/vrm-asset-generator/src/sidecar.rs` (add `build_synthetic_collider_test_plan`)
- Modify: `crates/vrm-asset-generator/src/main.rs` (or wherever subcommands are dispatched — add `emit-synthetic-collider-asset`)

> If Task 1 selected the **fixture path**, skip the generated asset: the plan's `asset` points at `assets/humanoid/avatarA_1_0.vrm` and you run the runner with `--asset-dir assets/humanoid`. The plan builder below is unchanged.

- [ ] **Step 1: Write failing test for the plan builder**

Add to `crates/vrm-asset-generator/src/sidecar.rs` tests:

```rust
    #[test]
    fn synthetic_collider_plan_captures_positions_and_colliders_no_ccd() {
        let mtoon = MToonParams::defaults("synthcoll_swept");
        let plan = build_synthetic_collider_test_plan(&mtoon, "synthcoll_swept.vrm", /*fast=*/ true);
        let rs = plan.render_sequence.as_ref().expect("render_sequence");
        assert!(rs.capture_positions);
        assert!(rs.capture_synthetic_colliders);
        assert!(rs.animate_root_transform.is_some(), "swept uses root translation");
        assert!(plan.ccd_colliders.is_none(), "synthetic colliders are runtime-dumped, not authored");
        assert!(plan.animation.is_none());
        plan.validate().expect("plan must validate");
    }
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vrm-asset-generator synthetic_collider_plan`
Expected: FAIL — `build_synthetic_collider_test_plan` not found.

- [ ] **Step 3: Implement the plan builder**

Add to `crates/vrm-asset-generator/src/sidecar.rs` (model on `build_spring_bone_ccd_test_plan`):

```rust
/// Plan for the synthetic-collider validation corpus. Drives `render_sequence`
/// with both `capture_positions` and `capture_synthetic_colliders` true, a
/// fast (swept) or slow (static-ish) root translation to excite the hair
/// chain, and NO authored `ccd_colliders` — the synthetic colliders are dumped
/// at runtime by the adapter. Rendered twice (augment on/off) by the runner.
pub fn build_synthetic_collider_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
    fast: bool,
) -> TestPlan {
    let (frame_count, frame_hz) = if fast { (12u32, 60.0_f32) } else { (120u32, 60.0_f32) };
    // Sweep the head (and its bone-attached synthetic colliders) laterally so
    // the lagging hair chain is overtaken by the moving collider.
    let translation_start = [-0.30_f32, 0.0, 0.0];
    let translation_end = [0.30_f32, 0.0, 0.0];

    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.spec_section = "VMK synthetic collider augmentation (#309/#313) — augment on/off".into();
    plan.physics = Some(PhysicsConfig { settle_steps: 60 });
    plan.post_processing = PostProcessing { tone_mapping: ToneMapping::None, exposure: 1.0 };
    plan.animation = None;
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count,
        frame_hz,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start,
            translation_end,
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,
        capture_positions: true,
        capture_synthetic_colliders: true,
    });
    plan.ccd_colliders = None;
    plan
}
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vrm-asset-generator synthetic_collider_plan`
Expected: PASS.

- [ ] **Step 5: Add an emit subcommand**

Add `emit-synthetic-collider-asset` that emits ONE humanoid + hair-chain triplet (`.vrm` + `.meta.json` + `.test.yaml`). Reuse `emit_vrm_with_spring_bone` with a hair chain hung off the head. Starting chain config (tune in Task 12):

```rust
let mut spring = crate::spring_bone::SpringBoneParams::defaults(&id);
spring.joint_count = 6;          // longer chain → more inertial lag
spring.segment_length_m = 0.06;
spring.stiffness = 0.2;          // soft enough to lag and to be pushed
spring.drag_force = 0.2;
// Name the chain "hair" so it's recognizable; VMK collides every spring with
// the synthetic group regardless, but this matches the fixture convention.
```

Emit the asset, then write the plan from `build_synthetic_collider_test_plan(&mtoon, &format!("{id}.vrm"), fast)` (emit both a `_fast` and `_slow` plan id). Follow the exact emit/write pattern used by `emit-springbone-ccd-sweep` in the same dispatch file.

- [ ] **Step 6: Smoke the emit**

Run: `cargo run -p vrm-asset-generator -- emit-synthetic-collider-asset --output-dir /tmp/synthcoll`
Expected: `<id>.vrm`, `<id>.meta.json`, `<id>.test.yaml` written; `cargo run -p vrm-validator-wrap` (or the suite's validator) parses the `.vrm`.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/sidecar.rs crates/vrm-asset-generator/src/main.rs
git commit -m "feat(asset-gen): synthetic-collider corpus (humanoid + hair chain, augment on/off plan)"
```

---

## Task 12: end-to-end run, tuning, and findings entry

**Files:**
- Modify: `docs/findings.md`

- [ ] **Step 1: Render the asset twice (augment on/off) through rc.2**

```bash
cd /Users/arkavo/Projects/vrm-conformance
ADAPTER=adapters/vrm-metal-kit/.build/debug/vrm-metal-kit-adapter
A=/tmp/synthcoll; OUT=/tmp/synthcoll-out; mkdir -p $OUT
ID=<the fast asset id from Task 11>
for mode in on off; do
  flag=$([ $mode = on ] && echo true || echo false)
  cargo run -q -p vrm-runner -- execute-test-plan \
    --plan $A/$ID.test.yaml --adapter-bin "$ADAPTER" \
    --asset-dir $A --output-dir $OUT --renderer-name vmk-$mode \
    --augment-colliders $flag --json >/dev/null
done
ls $OUT/${ID}_vmk-on_colliders.json $OUT/${ID}_vmk-on_positions.json $OUT/${ID}_vmk-off_positions.json
```

Expected: ON produces both `_positions.json` and `_colliders.json`; OFF produces `_positions.json` and **no** `_colliders.json`.

- [ ] **Step 2: Measure ON vs OFF penetration against the ON-run colliders**

```bash
echo "ON:"; cargo run -q -p vrm-runner -- penetration-diff \
  --positions $OUT/${ID}_vmk-on_positions.json \
  --plan $A/$ID.test.yaml \
  --colliders $OUT/${ID}_vmk-on_colliders.json --json
echo "OFF:"; cargo run -q -p vrm-runner -- penetration-diff \
  --positions $OUT/${ID}_vmk-off_positions.json \
  --plan $A/$ID.test.yaml \
  --colliders $OUT/${ID}_vmk-on_colliders.json --json
```

Expected signal: **OFF `max_penetration_depth_m` > 0** (chain enters the synthetic volume), **ON `max_penetration_depth_m` ≈ 0** (caught at the surface).

- [ ] **Step 3: Tune if no signal (spec risk #2)**

If OFF ≈ 0 (chain never reaches the synthetic volume) or ON ≈ OFF (augmentation not engaging): adjust in Task 11's emit and re-run — increase `joint_count`/`segment_length_m` (longer chain, more lag), lower `stiffness`, move the chain attach point toward the skull-sphere / head-capsule region, or widen the translation sweep. Iterate Steps 1–2 until OFF clearly penetrates and ON clearly doesn't. **Log the final tuned config** in the findings entry so it's reproducible.

- [ ] **Step 4: Also run the slow variant** (Step 1–2 with the `_slow` plan id) to cover the static-ish (#309) excitation alongside the swept (#313) one.

- [ ] **Step 5: Write the findings entry**

Add a dated entry to the top of `docs/findings.md` (newest first): the ON/OFF `max_penetration_depth_m` table for both excitations, the tuned chain config, and the verdict — whether rc.2's synthetic augmentation measurably deflects the chain (ON ≪ OFF) — the suite's independent confirmation of #309/#313. If the result is null (ON≈OFF), state that plainly and that the corpus could not produce a deflection signal with this asset.

- [ ] **Step 6: Workspace gates + commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add docs/findings.md
git commit -m "docs(findings): synthetic-collider augment on/off validation at VMK 0.17.0-rc.2"
```

---

## Self-Review

**Spec coverage:**
- Step-0 spike → Task 1. ✓
- Asset (humanoid + hair chain) → Task 11. ✓
- Adapter augment on/off flag → Tasks 2 (wire), 6 (thread), 7 (CLI), 9 (Swift). ✓
- Adapter per-frame synthetic-collider dump → Tasks 2 (wire), 10 (Swift). ✓
- Runner persists `_colliders.json` → Task 6. ✓
- diff-engine per-frame penetration → Task 3; consumed via Task 8. ✓
- Metric ON vs OFF, both excitations, null-is-valid → Task 12. ✓
- Coordinate frames (worldMatrix transform) → Task 10. ✓
- Findings deliverable → Task 12. ✓

**Type consistency:** `SequenceCollider` (ops) ↔ `FrameCollidersEntry.colliders` (runner) ↔ `to_collider_spec_seq` → `ColliderSpec` (diff). `augment_colliders` is `Option<bool>` end-to-end (CLI → `ExecuteOptions` → `LoadVrmParams` → Swift `augment_colliders`). `capture_synthetic_colliders` is `bool` (plan block → params → Swift). `synthetic_colliders` is `Option<Vec<SequenceCollider>>` on `SequenceFrame`. File name `<plan_id>_<renderer>_colliders.json` consistent across Task 6 (write) and Tasks 8/12 (read). Consistent.

**Placeholders:** none — every code step shows code. Two flagged confirmations in Task 10 (radius scale; exact `VRMColliderShape` case labels) are verification notes against the live checkout, not deferred work.

**Open dependency:** Task 7 intentionally doesn't compile until Task 8 widens `run_penetration_diff`; they commit together (noted in Task 7 Step 5 / Task 8 Step 5).
