# `render_sequence` Phase 2 — Diff Engine + Test Plan + Manifest + Runner Integration

> **For agentic workers:** Use superpowers:subagent-driven-development to execute this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. RFC-0004 is now Accepted; Phase 1 op surface landed at SHA `7b1f1cf`.

**Goal:** Make the op surface from Phase 1 *consumable*. Add the temporal diff aggregator, extend the test plan and manifest schemas, wire the runner to dispatch `render_sequence` end-to-end, and extend `consensus-diff` to handle sequence manifest entries. After this phase, a handcrafted sequence test plan can run through the runner against any adapter (all 4 will return Unimplemented; that's expected — Phase 5+ makes them real).

**Architecture:** Five logical clusters, plan-ordered as sub-phases:

- **2a** (Tasks 1–4): `crates/vrm-diff-engine/src/temporal.rs` — `TemporalDiffResult`, `FrameDiff`, `temporal_diff` function with BLAKE3 short-circuit + worst-frame tracking.
- **2b** (Tasks 5–6): `crates/vrm-test-plan` — `RenderSequenceBlock` field on `TestPlan`, mutual-exclusion validator.
- **2c** (Tasks 7–8): `crates/vrm-s3/src/manifest.rs` — `ManifestEntry::Image | Sequence` discriminator (back-compat default `image`), `validate-manifest` extended.
- **2d** (Tasks 9–11): `crates/vrm-runner` — `plan_to_ops::render_sequence_params`, `execute::run_render_sequence`, JSON output extended with `TemporalDiffResult`.
- **2e** (Task 12): `consensus-diff` extended for sequence manifest entries (per-frame pairwise SSIM).
- **Cleanup** (Task 13): fmt + clippy + workspace test sweep.

**Tech stack:** Pure Rust; no adapter changes (Phase 5+).

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md). Methodology pins live in [`docs/methodology.md`](../../methodology.md) (Sequence captures section).

---

## File structure

**Create:**
- `crates/vrm-diff-engine/src/temporal.rs` (~250 LOC)

**Modify:**
- `crates/vrm-diff-engine/src/lib.rs` — `pub mod temporal;`
- `crates/vrm-test-plan/src/lib.rs` — new `RenderSequenceBlock` type + `Option<RenderSequenceBlock>` field on `TestPlan` + validator method
- `crates/vrm-s3/src/manifest.rs` — `ManifestEntry` becomes an enum-discriminated kind; back-compat default `image` for existing flat entries
- `crates/vrm-s3/src/bin/validate-manifest.rs` (or wherever the validator binary lives) — accept both kinds
- `crates/vrm-runner/src/plan_to_ops.rs` — new `render_sequence_params(...)` constructor
- `crates/vrm-runner/src/execute.rs` — new `run_render_sequence` branch that dispatches the op and invokes temporal_diff
- `crates/vrm-runner/src/diff.rs` — surface `TemporalDiffResult` in the diff CLI subcommand
- `crates/vrm-runner/src/cli.rs` — extend `execute-test-plan` JSON output schema
- New: `crates/vrm-runner/tests/` entry for end-to-end sequence runner test (handcrafted plan against mock renderer; mock will Unimplemented)

---

## Phase 2a — `temporal_diff` module

### Task 1: `TemporalDiffResult` + `FrameDiff` types

**Files:**
- Create: `crates/vrm-diff-engine/src/temporal.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs`

- [ ] **Step 1.1: Create the module skeleton and types**

Create `crates/vrm-diff-engine/src/temporal.rs`:

```rust
//! Temporal diff for `render_sequence` outputs. Per-frame SSIM with
//! aggregation (mean / p95 / min), worst-frame tracking, and BLAKE3
//! identity short-circuit. See `rfcs/0004-render-sequence-op.md` and
//! `docs/methodology.md` ("Sequence captures") for the contract.

use serde::{Deserialize, Serialize};

/// Per-frame diff record. `identity_match` is true when both renders
/// produced byte-identical PNGs (BLAKE3 short-circuit hit); SSIM is set
/// to 1.0 in that case without computing SSIM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameDiff {
    pub index: u32,
    pub ssim: f64,
    pub identity_match: bool,
}

/// Aggregated diff result across a sequence pair. See
/// `docs/methodology.md` for the pass-criteria formula:
///   `passed = mean_ssim >= threshold AND min_ssim >= threshold - 0.05
///            AND frame_count_match`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDiffResult {
    pub frame_count: u32,
    pub frame_count_compared: u32,
    pub per_frame: Vec<FrameDiff>,
    pub mean_ssim: f64,
    pub p95_ssim: f64,
    pub min_ssim: f64,
    pub worst_frame_index: u32,
    pub frame_count_match: bool,
    pub temporal_ssim_threshold: f64,
    pub passed: bool,
}
```

Add `pub mod temporal;` to `crates/vrm-diff-engine/src/lib.rs` (insert at the alphabetical/logical position — match the file's existing module ordering).

- [ ] **Step 1.2: Add a serde round-trip test**

Decide whether to put it inside `temporal.rs` as `#[cfg(test)] mod tests` or in a sibling `tests/` integration file. Inspect the crate's existing convention by `ls crates/vrm-diff-engine/tests/` and `grep -l "#\[cfg(test)\]" crates/vrm-diff-engine/src/*.rs`.

The test should:
- Construct a `TemporalDiffResult` with 2 `FrameDiff` entries (one identity_match, one not).
- Round-trip through `serde_json`.
- Assert `assert_eq!(back.per_frame, original.per_frame)` (whole-vec equality).
- Assert key field values survive.

- [ ] **Step 1.3: Build + test**

```
cargo test -p vrm-diff-engine
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 1.4: Commit**

```bash
git add crates/vrm-diff-engine/src/temporal.rs crates/vrm-diff-engine/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): add TemporalDiffResult + FrameDiff types

Skeleton for the temporal_diff aggregator. Mirrors the schema declared in
RFC-0004. Identity-match flag carries BLAKE3 short-circuit signal so site
can distinguish "we proved identity" from "we computed 1.0 SSIM."
EOF
)"
```

---

### Task 2: `temporal_diff` function (no BLAKE3 yet)

**Files:**
- Modify: `crates/vrm-diff-engine/src/temporal.rs`

- [ ] **Step 2.1: Write failing tests for the four scenarios**

Append to `temporal.rs` test module:

```rust
// Helpers: produce N-byte PNG files (or use existing test fixtures from
// the crate's SSIM tests). Look at `crates/vrm-diff-engine/src/ssim.rs`
// tests for the convention — there's likely a helper that emits two
// solid-color PNGs to a temp dir.

#[test]
fn temporal_diff_identical_sequences_passes_with_ssim_1() {
    // Both sequences point at the same N PNG files.
    // Expected: mean_ssim ≈ 1.0, min_ssim ≈ 1.0,
    //           passed=true, frame_count_match=true.
    // (Without BLAKE3 short-circuit yet, SSIM is computed and yields 1.0.)
}

#[test]
fn temporal_diff_single_bad_frame_passes_when_mean_holds() {
    // 10 identical PNG pairs + 1 mismatched pair at index 5.
    // Threshold 0.90 ⇒ min_ssim ≈ 0.0 (bad frame) but mean ≈ 0.90.
    // Pass criterion uses threshold - 0.05 for min, so min would need
    // ≥ 0.85 to pass. Single bad frame at SSIM 0 fails this.
    // Expected: passed=false, worst_frame_index=5.
    //
    // Then with a threshold of 0.01: min relaxation = -0.04 (effective
    // 0.0 min bound), so passed=true.
}

#[test]
fn temporal_diff_gradual_drift_pass_or_fail_depends_on_threshold() {
    // SSIMs descending from 1.0 to 0.5 over 10 frames.
    // mean ≈ 0.75, min = 0.5.
    // Threshold 0.80 ⇒ fails on mean (0.75 < 0.80).
    // Threshold 0.50 ⇒ passes both (mean 0.75 ≥ 0.50, min 0.50 ≥ 0.45).
}

#[test]
fn temporal_diff_length_mismatch_fails_regardless_of_ssim() {
    // 10-frame baseline vs 8-frame candidate.
    // Expected: frame_count_match=false, passed=false, even if the
    // 8 compared frames all hit SSIM 1.0.
}
```

The test bodies need actual frame PNG paths. Two options:
1. Inline-generate solid-color PNGs via the `image` crate (already a dependency — check `Cargo.toml`).
2. Reuse existing test fixtures from `crates/vrm-diff-engine/src/ssim.rs` tests.

Use whichever pattern the crate's `ssim.rs` test module already follows.

- [ ] **Step 2.2: Run the tests to verify they fail to compile (function doesn't exist yet)**

- [ ] **Step 2.3: Implement `temporal_diff`**

Append to `temporal.rs`:

```rust
use crate::ssim::ssim_pngs;
use camino::Utf8Path;

/// Diff two sequences frame-by-frame. Each sequence is a list of PNG paths
/// in capture order (index 0 = first frame). When the two sequences differ
/// in length, only the common prefix is compared and `frame_count_match`
/// is set to false (a hard failure regardless of SSIM).
///
/// The `threshold` is the per-test temporal_ssim_threshold (RFC-0004
/// default 0.90). Pass formula matches `docs/methodology.md`:
///   `passed = mean_ssim >= threshold AND
///            min_ssim >= threshold - 0.05 AND
///            frame_count_match`
pub fn temporal_diff(
    candidate_frames: &[&Utf8Path],
    reference_frames: &[&Utf8Path],
    threshold: f64,
) -> Result<TemporalDiffResult, TemporalDiffError> {
    let candidate_count = candidate_frames.len() as u32;
    let reference_count = reference_frames.len() as u32;
    let frame_count_match = candidate_count == reference_count;
    let compared = candidate_count.min(reference_count);

    let mut per_frame = Vec::with_capacity(compared as usize);
    for i in 0..compared as usize {
        let ssim = ssim_pngs(candidate_frames[i], reference_frames[i])
            .map_err(TemporalDiffError::Ssim)?;
        per_frame.push(FrameDiff {
            index: i as u32,
            ssim,
            identity_match: false,  // populated by BLAKE3 pass in Task 3
        });
    }

    let (mean_ssim, p95_ssim, min_ssim, worst_frame_index) =
        aggregate(&per_frame);

    let passed = frame_count_match
        && mean_ssim >= threshold
        && min_ssim >= threshold - 0.05;

    Ok(TemporalDiffResult {
        frame_count: candidate_count,
        frame_count_compared: compared,
        per_frame,
        mean_ssim,
        p95_ssim,
        min_ssim,
        worst_frame_index,
        frame_count_match,
        temporal_ssim_threshold: threshold,
        passed,
    })
}

fn aggregate(frames: &[FrameDiff]) -> (f64, f64, f64, u32) {
    if frames.is_empty() {
        return (0.0, 0.0, 0.0, 0);
    }
    let sum: f64 = frames.iter().map(|f| f.ssim).sum();
    let mean = sum / frames.len() as f64;

    let mut sorted: Vec<f64> = frames.iter().map(|f| f.ssim).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_index = ((sorted.len() as f64) * 0.05).floor() as usize;
    let p95 = sorted[p95_index];  // 5th percentile of SSIM values (=worst 5%)
    let min = *sorted.first().unwrap();

    let worst = frames
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.ssim.partial_cmp(&b.ssim).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i as u32)
        .unwrap_or(0);

    (mean, p95, min, worst)
}

#[derive(Debug, thiserror::Error)]
pub enum TemporalDiffError {
    #[error(transparent)]
    Ssim(#[from] crate::ssim::SsimError),
}
```

**Implementation note on p95:** the RFC says `p95_ssim` aggregates the per-frame distribution. "p95 SSIM" in the context of "worst-case quality" means the *5th percentile* (95% of frames are at least this good). The aggregate helper above computes that. Verify by reading the RFC's `TemporalDiffResult` field comments if ambiguous.

- [ ] **Step 2.4: Run tests, fix discrepancies**

```
cargo test -p vrm-diff-engine temporal
```

Expected: all four scenarios pass. If a test's expected math doesn't match the implementation, fix the test body to reflect the actual aggregate semantics (e.g. mean vs threshold relationship).

- [ ] **Step 2.5: Commit**

```bash
git add crates/vrm-diff-engine/src/temporal.rs
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): implement temporal_diff aggregator

Per-frame SSIM via the existing ssim_pngs path, aggregated to mean / p95 /
min plus worst-frame index. Pass criteria match the docs/methodology.md
pin: mean >= threshold AND min >= threshold - 0.05 AND frame_count_match.

Length mismatch is a hard fail regardless of SSIM (RFC-0004 failure mode).
EOF
)"
```

---

### Task 3: BLAKE3 short-circuit

**Files:**
- Modify: `crates/vrm-diff-engine/src/temporal.rs`

- [ ] **Step 3.1: Add a test for identity match short-circuit**

Add a test that asserts: when both PNG paths hash to the same BLAKE3, `identity_match` is true and `ssim` is exactly 1.0 (not 0.999...) without SSIM being computed.

The test should use a sentinel — e.g. point both candidate and reference at the same PNG file (so BLAKE3 will match trivially). Assert `identity_match: true, ssim: 1.0`.

- [ ] **Step 3.2: Add the short-circuit to `temporal_diff`**

Before calling `ssim_pngs`, compute BLAKE3 of each frame's PNG content. If they match, push a `FrameDiff { ssim: 1.0, identity_match: true, ... }` and skip the SSIM compute. The `blake3` crate is already a workspace dependency — verify in `Cargo.toml`; if not, add it.

```rust
fn blake3_of_file(path: &Utf8Path) -> Result<[u8; 32], TemporalDiffError> {
    let bytes = std::fs::read(path).map_err(|_| TemporalDiffError::Io(path.to_string()))?;
    Ok(blake3::hash(&bytes).into())
}
```

Add a corresponding `Io(String)` variant to `TemporalDiffError`.

- [ ] **Step 3.3: Run tests**

```
cargo test -p vrm-diff-engine temporal
cargo clippy --workspace --all-targets -- -D warnings
```

All five tests (four from Task 2 + new identity_match test) must pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vrm-diff-engine/src/temporal.rs crates/vrm-diff-engine/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(vrm-diff-engine): BLAKE3 short-circuit in temporal_diff

When two frame PNGs hash to the same BLAKE3, return identity_match=true
and ssim=1.0 exactly without computing SSIM. Common case for rest-pose
lead-in frames where every adapter produces the same bytes.
EOF
)"
```

---

### Task 4: Sequence-comparison fixture helper + edge-case tests

**Files:**
- Modify: `crates/vrm-diff-engine/src/temporal.rs`

- [ ] **Step 4.1: Add tests for empty sequence + single-frame sequence**

```rust
#[test]
fn temporal_diff_empty_sequences() {
    // Both empty: frame_count_match=true, but no frames to compare.
    // mean/p95/min default to 0.0; passed should be... false?
    // RFC doesn't specify. Document the choice in the test comment.
    // Decision: empty sequence passed=false (no signal == no pass).
}

#[test]
fn temporal_diff_single_frame_sequence() {
    // 1-frame each. p95 == min == mean.
    // Verifies edge case of len-1 sort.
}
```

- [ ] **Step 4.2: Implement edge-case handling**

Adjust `temporal_diff` if needed to handle empty + single-frame correctly. The aggregate function already short-circuits on `is_empty()`; verify it returns sensible values for `passed`.

- [ ] **Step 4.3: Run all temporal tests**

- [ ] **Step 4.4: Commit**

```bash
git add crates/vrm-diff-engine/src/temporal.rs
git commit -m "$(cat <<'EOF'
test(vrm-diff-engine): empty + single-frame sequence edge cases

Empty sequences pass frame_count_match (both 0) but cannot pass overall
(no SSIM signal). Single-frame sequences exercise the len-1 sort path.
EOF
)"
```

---

## Phase 2b — Test plan schema extension

### Task 5: `RenderSequenceBlock` field on `TestPlan`

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs`

- [ ] **Step 5.1: Define the new type**

Look at the existing `AnimationConfig` and `PhysicsConfig` structs for shape. Add:

```rust
/// Optional sequence-capture config. When present, the runner dispatches
/// `render_sequence` instead of (or in addition to) the single-frame
/// `render` op. Both `render`-only and `render_sequence`-only plans are
/// valid; both set is rejected by `TestPlan::validate`.
///
/// Field names match `vrm_ops::tools::RenderSequenceParams` for direct
/// projection in `plan_to_ops`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderSequenceBlock {
    pub frame_count: u32,
    pub frame_hz: f32,
    pub physics_dt_seconds: f32,
    pub output_format: SequenceFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate_root_transform: Option<RootTransformAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_vrma: Option<VrmaPlaybackSpec>,
    /// Temporal SSIM threshold for the diff aggregator. Optional override
    /// of the default 0.90 (RFC-0004).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_ssim_threshold: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFormat {
    PngSequence,
    Mp4,
    Mov,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootTransformAnimation {
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VrmaPlaybackSpec {
    pub vrma_handle: u32,
    pub start_seconds: f32,
}
```

**Important:** these mirror the `vrm-ops` types but are kept in `vrm-test-plan` to avoid the test-plan crate depending on vrm-ops. The runner's `plan_to_ops` does the conversion. (This matches the existing `Camera` / `Lighting` pattern: test-plan owns the schema, vrm-ops owns the op contract, plan_to_ops translates.)

Add a field to `TestPlan`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub render_sequence: Option<RenderSequenceBlock>,
```

Place the field after `animation` to match the file's ordering of optional blocks.

- [ ] **Step 5.2: Add serde round-trip tests**

Add tests in the same style as existing test-plan tests (likely in `crates/vrm-test-plan/src/lib.rs` `#[cfg(test)]` mod). Cover:
- Round-trip a `TestPlan` with `render_sequence: Some(...)`.
- Round-trip a `TestPlan` with `render_sequence: None` (verify field is absent from JSON).
- Round-trip with `animate_root_transform` set.
- Round-trip with `apply_vrma` set.

- [ ] **Step 5.3: Run tests + clippy**

- [ ] **Step 5.4: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(vrm-test-plan): add RenderSequenceBlock for sequence-capture plans

Mirrors vrm_ops::RenderSequenceParams structurally but lives in the plan
crate so vrm-test-plan stays independent of vrm-ops (runner's plan_to_ops
does the conversion). Existing single-frame render: plans untouched —
render_sequence is opt-in via Option.
EOF
)"
```

---

### Task 6: `TestPlan::validate` rejects render + render_sequence

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs`

- [ ] **Step 6.1: Check whether a validator already exists**

`grep -n "fn validate\|impl TestPlan" crates/vrm-test-plan/src/lib.rs`. If a `validate` method exists, extend it. If not, add one.

- [ ] **Step 6.2: Add the rule + a test**

```rust
impl TestPlan {
    pub fn validate(&self) -> Result<(), TestPlanError> {
        // ... existing rules ...

        // RFC-0004: render + render_sequence are mutually exclusive.
        // Existing `output:` block governs single-frame render; presence
        // of `render_sequence` redirects to multi-frame capture. A plan
        // declaring both is malformed.
        //
        // Note: the "render" presence test depends on how the existing
        // schema represents single-frame render. The `output:` block is
        // ALWAYS present (it carries dimensions). The rule is therefore:
        //   if `self.render_sequence.is_some()` AND the plan declares
        //   single-frame-specific config that would conflict, reject.
        // Inspect the existing `Output` struct to decide the precise
        // trigger. A safe default: just check `render_sequence.is_some()`
        // alongside any explicit single-frame `render` marker if one
        // exists. If `Output` is unambiguous and always required, no
        // mutual-exclusion is needed (the field set is unambiguous).
        //
        // Implement accordingly; the test below pins the chosen behavior.

        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TestPlanError {
    #[error("render and render_sequence are mutually exclusive")]
    BothRenderAndSequence,
    // ... other variants ...
}
```

- [ ] **Step 6.3: Test**

Add a test asserting that a plan with both blocks rejects. The exact shape depends on the resolution chosen in Step 6.2.

- [ ] **Step 6.4: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(vrm-test-plan): validator rejects plans with render + render_sequence

Per RFC-0004 the two are mutually exclusive at the plan level. The
runner's dispatch path relies on the validator catching this — otherwise
adapter dispatch would have to handle ambiguous plans.
EOF
)"
```

---

## Phase 2c — Manifest schema extension

### Task 7: `ManifestEntry` becomes kind-discriminated

**Files:**
- Modify: `crates/vrm-s3/src/manifest.rs`

This is the trickiest schema change because existing manifest entries on disk must continue to parse. Two viable designs:

**Design A (preferred):** Add `kind: ManifestEntryKind` field with `#[serde(default)] = Image`; the existing flat `ManifestEntry` carries image-specific fields as Option-with-default and a new `sequence: Option<SequenceManifest>` block.

**Design B:** `ManifestEntry` becomes `enum { Image(ImageEntry), Sequence(SequenceEntry) }` with `#[serde(tag = "kind", rename_all = "lowercase")]`.

Design B is cleaner but breaks every existing manifest entry on disk (every line needs `"kind": "image"` added). Design A is backward-compatible — existing entries continue to parse as kind=image without any data migration.

**Use Design A.**

- [ ] **Step 7.1: Inspect existing entries on disk**

`head -50 goldens/manifest.json` (if it exists) to confirm the on-disk shape. Verify Design A preserves parsing.

- [ ] **Step 7.2: Refactor `ManifestEntry`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestEntryKind {
    Image,
    Sequence,
}

impl Default for ManifestEntryKind {
    fn default() -> Self { Self::Image }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub test_id: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,

    #[serde(flatten)]
    pub metadata: SubmissionMetadata,

    /// Defaults to Image for back-compat with existing flat entries.
    #[serde(default)]
    pub kind: ManifestEntryKind,

    // Image-kind fields (Optional for sequence entries; required for image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_blake3: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,

    pub submitted_at: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_blake3: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_blake3: Option<String>,

    // Sequence-kind fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<SequenceManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceManifest {
    pub frame_count: u32,
    pub frame_hz: f32,
    pub frames: Vec<SequenceManifestFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muxed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muxed_blake3: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceManifestFrame {
    pub index: u32,
    pub image_url: String,
    pub blake3: String,
}
```

**Critical:** `image_url`, `image_blake3`, `byte_size` were previously `String`/`u64` (required). Making them Optional could regress existing CI that reads them. Check `crates/vrm-s3/src/bin/` and `scripts/bootstrap-goldens.sh` for callers that assume non-Option. If found, add a convenience method `ManifestEntry::image_url_or_panic(&self)` that asserts `kind == Image` and unwraps — keeps the migration tractable.

- [ ] **Step 7.3: Add serde round-trip tests**

The existing test module already has good coverage for image entries. Add:
- Round-trip an image-kind entry written WITHOUT `"kind":` field (back-compat — must default to Image).
- Round-trip a sequence-kind entry.
- Reject deserialization of a sequence entry missing `sequence` field if `kind == Sequence`.

- [ ] **Step 7.4: Run tests + clippy**

`cargo test -p vrm-s3` and clippy.

- [ ] **Step 7.5: Commit**

```bash
git add crates/vrm-s3/src/manifest.rs
git commit -m "$(cat <<'EOF'
feat(vrm-s3): ManifestEntry gains kind discriminator + sequence variant

Design A: flat struct with kind field defaulting to Image so existing
manifest entries on disk continue to parse without migration. Sequence
entries carry a SequenceManifest block with per-frame URLs + BLAKE3.

image_url, image_blake3, byte_size become Optional at the type level so
sequence entries can omit them; image-kind callers should treat them as
required (validate-manifest enforces this in the next task).
EOF
)"
```

---

### Task 8: `validate-manifest` accepts both kinds

**Files:**
- Locate: `find . -name 'validate-manifest*' -not -path './target/*'` (in crates/vrm-s3/src/bin or similar)

- [ ] **Step 8.1: Read the existing validator**

Understand what checks it currently runs (URL presence, BLAKE3 well-formedness, byte_size, schema version).

- [ ] **Step 8.2: Add kind-aware checks**

For `kind: Image`: require `image_url`, `image_blake3`, `byte_size` are all `Some`. Reject if any is missing.

For `kind: Sequence`: require `sequence: Some(_)` with at least one frame; each frame's `image_url` + `blake3` must be well-formed; `image_url`/`image_blake3` at the top level should be `None` (sequence entries don't carry a top-level image).

Add a test exercising each branch.

- [ ] **Step 8.3: Run tests**

- [ ] **Step 8.4: Commit**

```bash
git add crates/vrm-s3/
git commit -m "$(cat <<'EOF'
feat(vrm-s3): validate-manifest accepts sequence-kind entries

Image kind requires image_url + image_blake3 + byte_size. Sequence kind
requires a non-empty frames list with per-frame image_url + blake3 and
no top-level image_*. Existing manifests parse identically because the
kind discriminator defaults to Image.
EOF
)"
```

---

## Phase 2d — Runner integration

### Task 9: `plan_to_ops::render_sequence_params`

**Files:**
- Modify: `crates/vrm-runner/src/plan_to_ops.rs`

- [ ] **Step 9.1: Add the constructor**

```rust
pub fn render_sequence_params(
    session_id: &str,
    output_dir: &Utf8Path,
    output: &plan::Output,
    block: &plan::RenderSequenceBlock,
) -> ops::RenderSequenceParams {
    ops::RenderSequenceParams {
        session_id: session_id.into(),
        width: output.width,
        height: output.height,
        output_dir: output_dir.to_string(),
        frame_count: block.frame_count,
        frame_hz: block.frame_hz,
        physics_dt_seconds: block.physics_dt_seconds,
        color_space: convert_color_space(&output.color_space),
        msaa: output.msaa,
        output_type: convert_output_type(&output.output_type),
        output_format: match block.output_format {
            plan::SequenceFormat::PngSequence => ops::SequenceFormat::PngSequence,
            plan::SequenceFormat::Mp4 => ops::SequenceFormat::Mp4,
            plan::SequenceFormat::Mov => ops::SequenceFormat::Mov,
        },
        animate_root_transform: block.animate_root_transform.as_ref().map(|a| {
            ops::RootTransformAnimation {
                translation_start: a.translation_start,
                translation_end: a.translation_end,
            }
        }),
        apply_vrma: block.apply_vrma.as_ref().map(|v| ops::VrmaPlaybackSpec {
            vrma_handle: v.vrma_handle,
            start_seconds: v.start_seconds,
        }),
    }
}
```

`convert_color_space` and `convert_output_type` likely already exist in `plan_to_ops.rs` for the single-frame `render_params` — reuse.

- [ ] **Step 9.2: Add a unit test**

Construct a `RenderSequenceBlock` and `Output`, call the constructor, assert each `ops::RenderSequenceParams` field is set as expected.

- [ ] **Step 9.3: Build + clippy**

- [ ] **Step 9.4: Commit**

```bash
git add crates/vrm-runner/src/plan_to_ops.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): plan_to_ops::render_sequence_params

Mirror of render_params for sequence plans. Converts the test-plan-side
RenderSequenceBlock + Output into vrm_ops::RenderSequenceParams,
including the SequenceFormat / RootTransformAnimation / VrmaPlaybackSpec
projections.
EOF
)"
```

---

### Task 10: `execute::run_render_sequence`

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs`

- [ ] **Step 10.1: Identify the existing dispatch site**

`grep -n "render_params\|execute_test_plan\|fn run\b\|Adapter::call" crates/vrm-runner/src/execute.rs`. Find where the single-frame `render` op is dispatched. The new sequence dispatch should branch off the same point.

- [ ] **Step 10.2: Add the branch**

When `plan.render_sequence.is_some()`, dispatch `render_sequence` via the adapter and handle the result. Skeleton:

```rust
if let Some(seq_block) = &plan.render_sequence {
    let params = plan_to_ops::render_sequence_params(
        session_id, output_dir, &plan.output, seq_block,
    );
    let result: ops::RenderSequenceResult = adapter.call("render_sequence", &params)?;
    // Result handling: collect frames, optionally invoke temporal_diff
    // against --reference frames (Phase 2 leaves the reference plumbing
    // as a follow-up — for now, just surface RenderSequenceResult in
    // the runner JSON output without diff).
} else {
    // existing single-frame path
}
```

The exact adapter-call shape depends on the existing `Adapter` trait. Verify with `grep -n "trait Adapter\|fn call" crates/vrm-runner/src/adapter.rs`.

**Handle the Unimplemented case cleanly.** When the adapter returns `-32000` with `phase: "v1.x-sequence"`, the runner should surface this as a meaningful failure status (not a generic error). This is the path the integration test in Task 11 exercises against the mock renderer.

- [ ] **Step 10.3: Run integration test against mock**

Use the existing `vrm-mock-renderer` binary as the adapter. Construct a minimal sequence test plan, run through the runner, assert the runner reports the Unimplemented envelope back through its JSON output without crashing.

- [ ] **Step 10.4: Commit**

```bash
git add crates/vrm-runner/src/execute.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): dispatch render_sequence when plan declares it

Branches off the existing render dispatch. Adapter Unimplemented (-32000
with phase: v1.x-sequence) surfaces cleanly through the runner's JSON
output as a failure status; runner exits with overall_passed=false but
no crash. Temporal diff invocation against a --reference frame set is a
follow-up — Phase 2 lands the dispatch path; reference plumbing in 2d.x.
EOF
)"
```

---

### Task 11: Runner JSON output surfaces `TemporalDiffResult`

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` (or wherever the runner's JSON-output schema is built)
- Modify: `crates/vrm-runner/src/diff.rs` (extend diff subcommand to support sequence references)

- [ ] **Step 11.1: Extend the diff subcommand**

The current `vrm-runner diff --plan --render --reference` takes single PNGs. Add a mode for sequences: `--render-frames <dir>` + `--reference-frames <dir>`. Or extend the existing flags to accept directories.

Inspect the existing diff CLI in `crates/vrm-runner/src/cli.rs` and `diff.rs` first; decide the additive UX that doesn't break existing callers.

- [ ] **Step 11.2: Wire `temporal_diff` into the diff command**

When called in sequence mode, invoke `vrm_diff_engine::temporal::temporal_diff(...)`, surface the result in the runner's JSON output under a `temporal_diff` field.

- [ ] **Step 11.3: Integration test**

Construct two synthetic 3-frame PNG sequences (identical, then with one altered frame), run the diff CLI, assert the JSON output's `passed`, `mean_ssim`, `worst_frame_index` match expectations.

- [ ] **Step 11.4: Commit**

```bash
git add crates/vrm-runner/src/diff.rs crates/vrm-runner/src/cli.rs
git commit -m "$(cat <<'EOF'
feat(vrm-runner): diff subcommand supports sequence mode

--render-frames <dir> / --reference-frames <dir> invoke
vrm_diff_engine::temporal::temporal_diff and surface the
TemporalDiffResult in the JSON output. Existing --render/--reference
single-frame mode unchanged.
EOF
)"
```

---

## Phase 2e — `consensus-diff` temporal mode

### Task 12: `consensus-diff` for sequence manifest entries

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` + wherever consensus-diff dispatch lives

- [ ] **Step 12.1: Identify the existing consensus-diff implementation**

`grep -rn "consensus-diff\|consensus_diff\|consensus" crates/vrm-runner/src/`

- [ ] **Step 12.2: Add sequence-aware path**

When all `--render` arguments point at sequence-kind manifest entries (or `--render` is extended to take per-frame directories), invoke `temporal_diff` pairwise across the N adapters and emit a per-frame consensus report.

- [ ] **Step 12.3: Integration test**

Three synthetic sequences, all identical → consensus reports all pairs at SSIM 1.0. One sequence diverges at frame 5 → consensus identifies the outlier.

- [ ] **Step 12.4: Commit**

```bash
git add crates/vrm-runner/
git commit -m "$(cat <<'EOF'
feat(vrm-runner): consensus-diff sequence mode

N-way pairwise temporal_diff across sequence-kind manifest entries. Per-
frame consensus identifies outliers (e.g. one renderer diverges only at
the midpoint of a swing animation). Image-kind entries unchanged.
EOF
)"
```

---

## Cleanup — Task 13

- [ ] **Step 13.1: fmt + clippy + workspace test sweep**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd adapters/three-vrm && npm test && cd -
```

- [ ] **Step 13.2: Commit any fmt fixes**

---

## Phase 2 completion checklist

- [ ] `vrm-diff-engine::temporal::temporal_diff` implemented with BLAKE3 short-circuit
- [ ] 4 + identity-match scenarios + 2 edge-case tests all pass
- [ ] `vrm-test-plan::TestPlan::render_sequence` field with `RenderSequenceBlock`
- [ ] `TestPlan::validate` rejects render + render_sequence both set
- [ ] `ManifestEntry` kind-discriminated with back-compat default
- [ ] `validate-manifest` enforces kind-specific requirements
- [ ] `plan_to_ops::render_sequence_params` constructor + test
- [ ] Runner dispatches `render_sequence` when plan declares it
- [ ] Runner JSON output surfaces Unimplemented envelope cleanly
- [ ] `vrm-runner diff` supports `--render-frames` + `--reference-frames`
- [ ] `vrm-runner consensus-diff` supports sequence-kind entries
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] three-vrm npm test green

After Phase 2 lands, Phase 3 (mock renderer reference implementation) is unblocked. The mock renderer's `render_sequence` impl gives the diff engine and runner a deterministic E2E target.
