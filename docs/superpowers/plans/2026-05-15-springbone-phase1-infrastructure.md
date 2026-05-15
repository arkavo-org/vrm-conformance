# VRMC_springBone Phase 1 — Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the `dump_bone_positions` op end-to-end (vrm-ops + four adapters + diff engine + manifest + runner) so phases 2-7 can layer asset coverage on a stable infrastructure surface.

**Architecture:** New JSON-RPC op `dump_bone_positions` returns world-space joint positions for each spring in the session. Position-comparison math lives in `vrm-diff-engine`. Manifest gains optional `positions_url` / `positions_blake3` fields. Runner gains `--reference-positions <name>=<path>` flag parallel to `--reference`. `overall_passed` = SSIM passed AND (position diff passed OR no reference).

**Tech Stack:** Rust 1.88 (vrm-ops, vrm-diff-engine, vrm-runner, vrm-s3, vrm-mock-renderer, vrm-godot-shim), Swift 6.3 (vrm-metal-kit adapter), TypeScript + Playwright (three-vrm adapter), GDScript (godot-vrm adapter).

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §3 (Phase 1).

---

## File map

**Create:**
- `crates/vrm-diff-engine/src/positions.rs` — new module
- `tests/e2e/dump_positions_smoke.rs` — top-level integration test (workspace test crate may already exist; if not, add as a new workspace crate `crates/vrm-runner-e2e`)

**Modify:**
- `crates/vrm-ops/src/tools.rs` — add `DumpBonePositionsParams`, `DumpBonePositionsResult`, `SpringPositions`
- `crates/vrm-diff-engine/src/lib.rs` — `pub mod positions`
- `crates/vrm-s3/src/manifest.rs` — `positions_url` / `positions_blake3` optional fields
- `crates/vrm-s3/src/bin/validate-manifest.rs` — validate new fields when present
- `crates/vrm-runner/src/cli.rs` — `--reference-positions <name>=<path>`
- `crates/vrm-runner/src/execute.rs` — call op + thread position-diff result
- `crates/vrm-runner/src/diff.rs` — orchestrate position diff alongside SSIM
- `crates/vrm-mock-renderer/src/handlers.rs` — `dump_bone_positions` handler (zeros)
- `crates/vrm-mock-renderer/src/main.rs` — wire into dispatch
- `adapters/three-vrm/src/operations.ts` — handler
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` — handler
- `crates/vrm-godot-shim/src/bridge.rs` — pass-through of op
- `adapters/godot-vrm/src/operations.gd` — handler
- `adapters/godot-vrm/src/session.gd` — bone-path retention if not already present
- `docs/operation-contract.md` — op spec entry
- `docs/methodology.md` — position-diff thresholds section

---

## Task 1: Add op types to vrm-ops

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs:106-140`
- Test: `crates/vrm-ops/src/tools.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-ops/src/tools.rs` (create `mod tests` if absent — keep alongside existing tools tests if there are any):

```rust
#[cfg(test)]
mod dump_bone_positions_tests {
    use super::*;

    #[test]
    fn dump_bone_positions_params_roundtrip_with_spring_index() {
        let p = DumpBonePositionsParams {
            session_id: "sess-1".into(),
            spring_index: Some(2),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: DumpBonePositionsParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.spring_index, Some(2));
    }

    #[test]
    fn dump_bone_positions_params_omits_spring_index_when_none() {
        let p = DumpBonePositionsParams {
            session_id: "sess-1".into(),
            spring_index: None,
        };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert!(v.get("spring_index").is_none(),
            "spring_index None should be omitted, got {v}");
    }

    #[test]
    fn dump_bone_positions_result_roundtrip() {
        let r = DumpBonePositionsResult {
            springs: vec![SpringPositions {
                name: "hair_chain".into(),
                joint_positions: vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]],
            }],
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: DumpBonePositionsResult = serde_json::from_str(&s).unwrap();
        assert_eq!(back.springs.len(), 1);
        assert_eq!(back.springs[0].name, "hair_chain");
        assert_eq!(back.springs[0].joint_positions.len(), 2);
        assert_eq!(back.springs[0].joint_positions[1], [0.0, 0.95, 0.0]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p vrm-ops dump_bone_positions
```

Expected: compile error — `DumpBonePositionsParams`, `DumpBonePositionsResult`, `SpringPositions` are undefined.

- [ ] **Step 3: Add the types**

Append after `AnimateRootTransformParams` in `crates/vrm-ops/src/tools.rs`:

```rust
/// Dump world-space joint positions for spring-bone chains as of the most
/// recent state-advancing op (`render`, `step_physics`, `reset_physics`,
/// `animate_root_transform`). The op itself does NOT advance physics.
///
/// If `spring_index` is omitted, all springs in the loaded model are
/// returned. If provided, only that spring's positions are returned;
/// out-of-range indices return `-32602 InvalidParams`.
///
/// Adapters that have no spring-bone system or return rest-pose only (e.g.
/// univrm L3) MAY return `-32000 Unimplemented` with the standard phase
/// envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpBonePositionsParams {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpringPositions {
    pub name: String,
    pub joint_positions: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpBonePositionsResult {
    pub springs: Vec<SpringPositions>,
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cargo test -p vrm-ops dump_bone_positions
```

Expected: 3 tests pass.

- [ ] **Step 5: Lint**

```
cargo clippy -p vrm-ops --all-targets -- -D warnings
cargo fmt -p vrm-ops -- --check
```

Both must exit 0. If fmt fails, run `cargo fmt -p vrm-ops`.

- [ ] **Step 6: Commit**

```
git add crates/vrm-ops/src/tools.rs
git commit -m "feat(vrm-ops): add dump_bone_positions op types"
```

---

## Task 2: Mock renderer handler (deterministic zeros)

**Files:**
- Modify: `crates/vrm-mock-renderer/src/handlers.rs`
- Modify: `crates/vrm-mock-renderer/src/main.rs` (dispatch site)
- Test: `crates/vrm-mock-renderer/src/handlers.rs` (inline)

- [ ] **Step 1: Read existing handlers to mirror its style**

Read `crates/vrm-mock-renderer/src/handlers.rs` and `crates/vrm-mock-renderer/src/main.rs`. Identify how `step_physics` or `reset_physics` are dispatched and write your handler the same way.

- [ ] **Step 2: Write failing test**

Append to `crates/vrm-mock-renderer/src/handlers.rs`:

```rust
#[cfg(test)]
mod dump_positions_tests {
    use super::*;
    use vrm_ops::tools::{DumpBonePositionsParams, DumpBonePositionsResult};

    #[test]
    fn dump_positions_on_unknown_session_returns_invalid_params() {
        let mut store = SessionStore::default();
        let params = DumpBonePositionsParams {
            session_id: "nope".into(),
            spring_index: None,
        };
        let err = handle_dump_bone_positions(&mut store, params).unwrap_err();
        assert_eq!(err.code, -32602, "expected InvalidParams, got {err:?}");
    }

    #[test]
    fn dump_positions_returns_empty_springs_for_loaded_session() {
        let mut store = SessionStore::default();
        let session_id = store.create_session_for_test();
        let params = DumpBonePositionsParams {
            session_id,
            spring_index: None,
        };
        let result: DumpBonePositionsResult =
            handle_dump_bone_positions(&mut store, params).unwrap();
        assert_eq!(
            result.springs.len(),
            0,
            "mock has no springs; expected empty result"
        );
    }
}
```

If `SessionStore::create_session_for_test` doesn't exist, add a test helper in the same module that constructs a session and returns its ID (look at how other tests do this).

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p vrm-mock-renderer dump_positions
```

Expected: compile error — `handle_dump_bone_positions` undefined.

- [ ] **Step 4: Implement handler**

Add to `crates/vrm-mock-renderer/src/handlers.rs`:

```rust
use vrm_ops::tools::{
    DumpBonePositionsParams, DumpBonePositionsResult, SpringPositions,
};

pub fn handle_dump_bone_positions(
    store: &mut SessionStore,
    params: DumpBonePositionsParams,
) -> Result<DumpBonePositionsResult, RpcError> {
    let _session = store
        .get(&params.session_id)
        .ok_or_else(|| RpcError {
            code: -32602,
            message: format!("unknown session_id: {}", params.session_id),
            data: None,
        })?;
    // Mock has no spring-bone system. Return empty springs deterministically.
    // This satisfies the op contract: success response, empty payload, no error.
    Ok(DumpBonePositionsResult { springs: Vec::new() })
}
```

If the existing handler module uses a different error constructor, mirror it.

- [ ] **Step 5: Wire into dispatch**

In `crates/vrm-mock-renderer/src/main.rs` (or wherever method-name → handler dispatch lives), find the match arm for `"step_physics" => ...` and add:

```rust
"dump_bone_positions" => {
    let p: DumpBonePositionsParams = serde_json::from_value(req.params)?;
    let r = handle_dump_bone_positions(&mut store, p)?;
    serde_json::to_value(r)?
}
```

- [ ] **Step 6: Run tests + clippy**

```
cargo test -p vrm-mock-renderer
cargo clippy -p vrm-mock-renderer --all-targets -- -D warnings
```

Both pass.

- [ ] **Step 7: Commit**

```
git add crates/vrm-mock-renderer/
git commit -m "feat(vrm-mock-renderer): dump_bone_positions handler (deterministic empty)"
```

---

## Task 3: vrm-diff-engine positions module

**Files:**
- Create: `crates/vrm-diff-engine/src/positions.rs`
- Modify: `crates/vrm-diff-engine/src/lib.rs` (export the module)
- Test: `crates/vrm-diff-engine/src/positions.rs` (inline)

- [ ] **Step 1: Write the failing test**

Create `crates/vrm-diff-engine/src/positions.rs` with the test block first (TDD red phase):

```rust
//! Position-space diff for spring-bone joint world coordinates.
//!
//! Two thresholds: per-joint max drift and chain-summed drift. Both must
//! hold for `passed = true`. A chain of N joints each 1 mm off baseline
//! is a different bug from a chain with one joint 10 mm off.

#[cfg(test)]
mod tests {
    use super::*;
    use vrm_ops::tools::SpringPositions;

    fn pos(name: &str, joints: Vec<[f32; 3]>) -> SpringPositions {
        SpringPositions {
            name: name.into(),
            joint_positions: joints,
        }
    }

    #[test]
    fn identical_positions_pass() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let report = diff_positions(&a, &a, 0.005, 0.020);
        assert!(report.passed);
        assert_eq!(report.per_joint_max_drift_m, 0.0);
        assert_eq!(report.chain_summed_drift_m, 0.0);
    }

    #[test]
    fn single_joint_drift_within_tolerance_passes() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let b = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.953, 0.0]]); // 3 mm drift
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(report.passed);
        assert!((report.per_joint_max_drift_m - 0.003).abs() < 1e-5);
        assert_eq!(report.worst_joint_index, 1);
    }

    #[test]
    fn single_joint_exceeds_per_joint_tolerance_fails() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let b = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.94, 0.0]]); // 10 mm drift
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(!report.passed);
        assert_eq!(report.worst_joint_index, 1);
    }

    #[test]
    fn chain_summed_drift_exceeds_threshold_fails_even_if_each_joint_within() {
        // Three joints each 4 mm off (under 5 mm per-joint tolerance)
        // but chain-summed = 12 mm, still under 20 mm tolerance. PASSES.
        // Bump to each-joint 8 mm: per-joint exceeds; also chain = 24 mm > 20 mm.
        // Use 4 joints × 6 mm each: per-joint 6 mm > 5 mm tolerance, so this
        // case also fails per-joint. We need a case where chain-summed fails
        // but per-joint passes:
        // 5 joints × 4.5 mm each = per-joint 4.5 mm (under 5 mm),
        // chain-summed = 22.5 mm (over 20 mm).
        let a = pos("c", vec![
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]);
        let b = pos("c", vec![
            [0.0045, 1.0, 0.0],
            [0.0045, 1.0, 0.0],
            [0.0045, 1.0, 0.0],
            [0.0045, 1.0, 0.0],
            [0.0045, 1.0, 0.0],
        ]);
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(!report.passed,
            "per-joint 4.5mm under 5mm tol but chain-sum 22.5mm exceeds 20mm tol; expected fail");
        assert!(report.chain_summed_drift_m > 0.020);
    }

    #[test]
    fn mismatched_joint_count_fails_with_zero_passed() {
        let a = pos("c", vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let b = pos("c", vec![[0.0, 1.0, 0.0]]);
        let report = diff_positions(&a, &b, 0.005, 0.020);
        assert!(!report.passed,
            "different joint counts MUST fail; this is a structural error not a threshold one");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p vrm-diff-engine positions
```

Expected: module-not-found error from lib.rs, or compile error inside positions.rs.

- [ ] **Step 3: Implement `diff_positions` + types**

Replace the test-only file with the full module. The tests stay at the bottom.

```rust
//! Position-space diff for spring-bone joint world coordinates.
//!
//! Two thresholds: per-joint max drift and chain-summed drift. Both must
//! hold for `passed = true`. A chain of N joints each 1 mm off baseline
//! is a different bug from a chain with one joint 10 mm off.

use serde::{Deserialize, Serialize};
use vrm_ops::tools::SpringPositions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionDiffReport {
    pub per_joint_max_drift_m: f32,
    pub chain_summed_drift_m: f32,
    pub per_joint_tolerance_m: f32,
    pub chain_max_drift_m: f32,
    pub worst_joint_index: usize,
    pub passed: bool,
}

/// Returns a structural-failure report when joint counts differ. Callers
/// MUST treat `passed = false` as a hard failure regardless of magnitudes —
/// counts must match for thresholds to be meaningful.
pub fn diff_positions(
    actual: &SpringPositions,
    reference: &SpringPositions,
    per_joint_tolerance_m: f32,
    chain_max_drift_m: f32,
) -> PositionDiffReport {
    if actual.joint_positions.len() != reference.joint_positions.len() {
        return PositionDiffReport {
            per_joint_max_drift_m: f32::INFINITY,
            chain_summed_drift_m: f32::INFINITY,
            per_joint_tolerance_m,
            chain_max_drift_m,
            worst_joint_index: 0,
            passed: false,
        };
    }

    let mut per_joint_max = 0.0_f32;
    let mut chain_summed = 0.0_f32;
    let mut worst_joint = 0usize;

    for (i, (a, b)) in actual
        .joint_positions
        .iter()
        .zip(reference.joint_positions.iter())
        .enumerate()
    {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        chain_summed += d;
        if d > per_joint_max {
            per_joint_max = d;
            worst_joint = i;
        }
    }

    let passed = per_joint_max <= per_joint_tolerance_m
        && chain_summed <= chain_max_drift_m;

    PositionDiffReport {
        per_joint_max_drift_m: per_joint_max,
        chain_summed_drift_m: chain_summed,
        per_joint_tolerance_m,
        chain_max_drift_m,
        worst_joint_index: worst_joint,
        passed,
    }
}

// ... tests block from Step 1 stays here ...
```

- [ ] **Step 4: Export the module**

Edit `crates/vrm-diff-engine/src/lib.rs` and add:

```rust
pub mod positions;
```

(Place near the other `pub mod` lines, alphabetical or grouped however the file already organizes them.)

- [ ] **Step 5: Run tests + lint**

```
cargo test -p vrm-diff-engine positions
cargo clippy -p vrm-diff-engine --all-targets -- -D warnings
cargo fmt -p vrm-diff-engine -- --check
```

All 5 tests pass; no clippy/fmt issues.

- [ ] **Step 6: Commit**

```
git add crates/vrm-diff-engine/src/
git commit -m "feat(vrm-diff-engine): positions module with two-threshold diff"
```

---

## Task 4: Manifest schema extension

**Files:**
- Modify: `crates/vrm-s3/src/manifest.rs`
- Modify: `crates/vrm-s3/src/bin/validate-manifest.rs`
- Test: `crates/vrm-s3/src/manifest.rs` (inline)

- [ ] **Step 1: Write the failing test**

In `crates/vrm-s3/src/manifest.rs`, extend the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn entry_with_positions_roundtrips() {
    let e = ManifestEntry {
        test_id: "springbone_default".into(),
        renderer_name: "three-vrm".into(),
        renderer_version: "0.1.0".into(),
        git_hash: "deadbeef".into(),
        metadata: sample_metadata(),
        image_url: "s3://b/x.png".into(),
        image_blake3: "blake3:aaa".into(),
        byte_size: 100,
        submitted_at: "2026-05-15T12:00:00Z".into(),
        positions_url: Some("s3://b/x.positions.json".into()),
        positions_blake3: Some("blake3:bbb".into()),
    };
    let s = serde_json::to_string(&e).unwrap();
    let back: ManifestEntry = serde_json::from_str(&s).unwrap();
    assert_eq!(back.positions_url.as_deref(), Some("s3://b/x.positions.json"));
    assert_eq!(back.positions_blake3.as_deref(), Some("blake3:bbb"));
}

#[test]
fn entry_without_positions_omits_fields_from_json() {
    let e = ManifestEntry {
        test_id: "t".into(),
        renderer_name: "r".into(),
        renderer_version: "v".into(),
        git_hash: "g".into(),
        metadata: sample_metadata(),
        image_url: "s3://b/x.png".into(),
        image_blake3: "blake3:aaa".into(),
        byte_size: 1,
        submitted_at: "2026-05-15T12:00:00Z".into(),
        positions_url: None,
        positions_blake3: None,
    };
    let v: serde_json::Value = serde_json::to_value(&e).unwrap();
    assert!(v.get("positions_url").is_none(),
        "None positions_url must be omitted, got {v}");
    assert!(v.get("positions_blake3").is_none(),
        "None positions_blake3 must be omitted, got {v}");
}

#[test]
fn entry_existing_json_without_positions_parses() {
    // Backward compat: entries from before this change have no positions
    // fields. Must deserialize cleanly.
    let raw = r#"{
        "test_id": "old",
        "renderer_name": "three-vrm",
        "renderer_version": "0.1.0",
        "git_hash": "deadbeef",
        "os": "macos", "os_version": "14",
        "gpu_vendor": "Apple", "gpu_model": "M2",
        "driver_version": "M3", "build_flags": "rel",
        "image_url": "s3://b/x.png",
        "image_blake3": "blake3:aaa",
        "byte_size": 1,
        "submitted_at": "2026-05-10T12:00:00Z"
    }"#;
    let e: ManifestEntry = serde_json::from_str(raw).unwrap();
    assert!(e.positions_url.is_none());
    assert!(e.positions_blake3.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p vrm-s3 manifest
```

Expected: compile errors — `positions_url` / `positions_blake3` fields don't exist.

- [ ] **Step 3: Add fields**

Edit `crates/vrm-s3/src/manifest.rs` `ManifestEntry`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub test_id: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,

    #[serde(flatten)]
    pub metadata: SubmissionMetadata,

    pub image_url: String,
    pub image_blake3: String,
    pub byte_size: u64,
    pub submitted_at: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_blake3: Option<String>,
}
```

Update the existing test helper `fn entry(...)` to fill `positions_url: None, positions_blake3: None` so existing tests still compile.

- [ ] **Step 4: Run tests**

```
cargo test -p vrm-s3 manifest
```

All tests (existing + 3 new) pass.

- [ ] **Step 5: Extend validate-manifest assertions**

In `crates/vrm-s3/src/bin/validate-manifest.rs`, find where `image_blake3` is checked (length, prefix). Add parallel checks for `positions_blake3` when `positions_url` is present:

```rust
// after the image_blake3 validation block:
if let Some(url) = &entry.positions_url {
    let hash = entry.positions_blake3.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "{}: positions_url set without positions_blake3",
            entry.test_id
        )
    })?;
    if !hash.starts_with("blake3:") || hash.len() != "blake3:".len() + 64 {
        anyhow::bail!(
            "{}: positions_blake3 malformed (expected blake3:<64-hex>): {}",
            entry.test_id,
            hash
        );
    }
    // No URL scheme guard beyond what image_url already enforces — same
    // s3:// / file:// rules apply.
    let _ = url; // url presence already required above
}

if entry.positions_url.is_none() && entry.positions_blake3.is_some() {
    anyhow::bail!(
        "{}: positions_blake3 set without positions_url",
        entry.test_id
    );
}
```

Mirror exactly the style of the image_url/image_blake3 validation block.

- [ ] **Step 6: Run validate-manifest unit/integration tests**

```
cargo test -p vrm-s3
cargo build -p vrm-s3 --bin validate-manifest
```

If there are bin-specific tests, run them. Build must succeed.

- [ ] **Step 7: Lint**

```
cargo clippy -p vrm-s3 --all-targets -- -D warnings
cargo fmt -p vrm-s3 -- --check
```

- [ ] **Step 8: Commit**

```
git add crates/vrm-s3/
git commit -m "feat(vrm-s3): optional positions_url/positions_blake3 on manifest entries"
```

---

## Task 5: Runner `--reference-positions` flag + execute integration

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs`
- Modify: `crates/vrm-runner/src/execute.rs`
- Modify: `crates/vrm-runner/src/diff.rs`
- Modify: `crates/vrm-runner/src/main.rs` (if it does CLI handling separately)
- Test: `crates/vrm-runner/src/execute.rs` (inline integration test)

- [ ] **Step 1: Familiarize with current execute flow**

Read `crates/vrm-runner/src/execute.rs` end-to-end. Note where `reference: Option<Utf8PathBuf>` lives in `ExecuteOptions`, where `diff::diff_one` is called, and how the result is packed into `ExecuteResult::diff`.

- [ ] **Step 2: Write the failing test**

Append to `crates/vrm-runner/src/execute.rs`:

```rust
#[cfg(test)]
mod reference_positions_tests {
    use super::*;

    #[test]
    fn execute_result_carries_position_diff_when_reference_positions_set() {
        // This is a structural test: it asserts the type signature has
        // a `position_diff: Option<PositionDiffReport>` field on
        // ExecuteResult. We don't spawn an adapter here — that lives in
        // the e2e smoke test in task 11. Compile-pass is the assertion.
        fn _structural(opts: &ExecuteOptions, r: &ExecuteResult) {
            let _: &Option<Utf8PathBuf> = &opts.reference_positions;
            let _: &Option<vrm_diff_engine::positions::PositionDiffReport> =
                &r.position_diff;
        }
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p vrm-runner reference_positions
```

Expected: compile error — `reference_positions` and `position_diff` fields don't exist.

- [ ] **Step 4: Add fields to ExecuteOptions and ExecuteResult**

In `crates/vrm-runner/src/execute.rs`:

```rust
pub struct ExecuteOptions {
    pub adapter_bin: Utf8PathBuf,
    pub adapter_args: Vec<String>,
    pub asset_dir: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub emit_progress_ndjson: bool,
    pub reference: Option<Utf8PathBuf>,
    /// If provided, dump bone positions after render and diff against
    /// this JSON reference file (same shape as `DumpBonePositionsResult`).
    pub reference_positions: Option<Utf8PathBuf>,
}

pub struct ExecuteResult {
    pub test_id: String,
    pub renderer: String,
    pub output_png: Utf8PathBuf,
    pub actual_color_space: ops::ColorSpace,
    pub diff: Option<vrm_diff_engine::result::DiffResult>,
    /// Populated only when `ExecuteOptions::reference_positions` was set.
    pub position_diff: Option<vrm_diff_engine::positions::PositionDiffReport>,
}
```

- [ ] **Step 5: Wire op call into execute_plan**

In `execute_plan`, after the `render` call and before `dispose`, add:

```rust
let position_dump: Option<ops::DumpBonePositionsResult> =
    if opts.reference_positions.is_some() {
        progress(opts, "dump_bone_positions", &plan.id, json!({}));
        let r: ops::DumpBonePositionsResult = adapter
            .call(
                "dump_bone_positions",
                ops::DumpBonePositionsParams {
                    session_id: session_id.clone(),
                    spring_index: None,
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
        Some(r)
    } else {
        None
    };
```

Then after the existing image-diff block, add a parallel position-diff block:

```rust
let position_diff = if let (Some(ref_path), Some(dump)) =
    (&opts.reference_positions, position_dump.as_ref())
{
    progress(
        opts,
        "position_diff",
        &plan.id,
        json!({ "reference_positions": ref_path }),
    );
    Some(crate::diff::diff_positions_one(
        plan,
        dump,
        ref_path,
    )?)
} else {
    None
};
```

Populate `ExecuteResult::position_diff` with this value.

- [ ] **Step 6: Add `diff_positions_one` in diff.rs**

In `crates/vrm-runner/src/diff.rs`, add:

```rust
use vrm_diff_engine::positions::{diff_positions, PositionDiffReport};
use vrm_ops::tools::{DumpBonePositionsResult, SpringPositions};

/// Default tolerances for v1.0. Overridable via test plan in a later phase;
/// for phase 1 these are fixed. Settle: 5 mm per-joint, 20 mm chain.
/// Swing: 10 mm per-joint, 40 mm chain. Detection: a plan with an
/// `animation.root_transform` block uses swing thresholds.
const PER_JOINT_TOL_SETTLE_M: f32 = 0.005;
const CHAIN_TOL_SETTLE_M: f32 = 0.020;
const PER_JOINT_TOL_SWING_M: f32 = 0.010;
const CHAIN_TOL_SWING_M: f32 = 0.040;

pub fn diff_positions_one(
    plan: &TestPlan,
    actual: &DumpBonePositionsResult,
    reference_path: &Utf8Path,
) -> Result<PositionDiffReport> {
    let raw = std::fs::read_to_string(reference_path.as_std_path())?;
    let reference: DumpBonePositionsResult = serde_json::from_str(&raw)?;

    let (per_joint_tol, chain_tol) = if plan
        .animation
        .as_ref()
        .and_then(|a| a.root_transform.as_ref())
        .is_some()
    {
        (PER_JOINT_TOL_SWING_M, CHAIN_TOL_SWING_M)
    } else {
        (PER_JOINT_TOL_SETTLE_M, CHAIN_TOL_SETTLE_M)
    };

    if actual.springs.len() != reference.springs.len() {
        anyhow::bail!(
            "spring count mismatch: actual={} reference={}",
            actual.springs.len(),
            reference.springs.len()
        );
    }

    // For v1.0 phase 1, we diff the first spring only. Multi-spring N-way
    // reduction lands in phase 6 (multi-chain). The op already returns
    // all springs; this is just the consumer choosing how to summarize.
    let a = actual.springs.first().ok_or_else(|| {
        anyhow::anyhow!("actual dump contained zero springs")
    })?;
    let b = reference.springs.first().ok_or_else(|| {
        anyhow::anyhow!("reference dump contained zero springs")
    })?;

    Ok(diff_positions(a, b, per_joint_tol, chain_tol))
}
```

Add `use camino::Utf8Path;` if not already imported, and `use anyhow::Result;`, `use vrm_test_plan::TestPlan;`. Note `crate::diff::diff_positions_one` referenced from `execute.rs` Step 5.

- [ ] **Step 7: Wire CLI flag**

In `crates/vrm-runner/src/cli.rs`, find the `--reference` arg and add a sibling:

```rust
#[arg(long, value_name = "PATH")]
pub reference_positions: Option<Utf8PathBuf>,
```

In wherever this flag is mapped into `ExecuteOptions` (probably `main.rs` or `cli.rs`'s subcommand dispatch), thread `args.reference_positions` into `ExecuteOptions::reference_positions`.

- [ ] **Step 8: JSON output**

Find where `ExecuteResult` is serialized to JSON for `--json` output. Add `position_diff` alongside `diff`. Update `overall_passed` computation:

```rust
let overall_passed = match (&result.diff, &result.position_diff) {
    (Some(d), Some(p)) => d.ssim_passed && p.passed,
    (Some(d), None) => d.ssim_passed,
    (None, Some(p)) => p.passed,
    (None, None) => true, // no references provided — pipeline ran
};
```

- [ ] **Step 9: Run tests + lint**

```
cargo test -p vrm-runner
cargo clippy -p vrm-runner --all-targets -- -D warnings
cargo fmt -p vrm-runner -- --check
```

All pass.

- [ ] **Step 10: Commit**

```
git add crates/vrm-runner/
git commit -m "feat(vrm-runner): --reference-positions flag + position-diff in execute-test-plan output"
```

---

## Task 6: consensus-diff N-way position support

**Files:**
- Modify: `crates/vrm-runner/src/cli.rs` (or wherever consensus-diff subcommand lives)
- Modify: `crates/vrm-runner/src/diff.rs` or a new sibling module
- Modify: `crates/vrm-diff-engine/src/consensus.rs`
- Test: `crates/vrm-diff-engine/src/consensus.rs` (inline)

- [ ] **Step 1: Read existing consensus.rs to mirror its shape**

Read `crates/vrm-diff-engine/src/consensus.rs`. Existing pattern is "pairwise SSIM, flag outliers". The position parallel: "pairwise per-joint drift, flag any renderer whose summed-drift-to-others exceeds the rest by a margin".

- [ ] **Step 2: Write the failing test**

In `crates/vrm-diff-engine/src/consensus.rs`, add at the end:

```rust
#[cfg(test)]
mod position_consensus_tests {
    use super::*;
    use vrm_ops::tools::SpringPositions;

    fn pos(joints: Vec<[f32; 3]>) -> SpringPositions {
        SpringPositions {
            name: "c".into(),
            joint_positions: joints,
        }
    }

    #[test]
    fn three_renderers_in_agreement_have_no_outlier() {
        let a = pos(vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let entries = vec![
            ("three-vrm".into(), a.clone()),
            ("vrm-metal-kit".into(), a.clone()),
            ("godot-vrm".into(), a.clone()),
        ];
        let report = position_consensus(&entries, 0.010);
        assert!(report.outliers.is_empty());
    }

    #[test]
    fn single_outlier_above_threshold_is_flagged() {
        let baseline = pos(vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]]);
        let drifted = pos(vec![[0.0, 1.0, 0.0], [0.05, 0.95, 0.0]]); // 5 cm off
        let entries = vec![
            ("three-vrm".into(), baseline.clone()),
            ("vrm-metal-kit".into(), drifted),
            ("godot-vrm".into(), baseline),
        ];
        let report = position_consensus(&entries, 0.010);
        assert_eq!(report.outliers, vec!["vrm-metal-kit".to_string()]);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p vrm-diff-engine position_consensus
```

Expected: undefined `position_consensus`.

- [ ] **Step 4: Implement**

In `crates/vrm-diff-engine/src/consensus.rs` add:

```rust
use crate::positions::diff_positions;
use vrm_ops::tools::SpringPositions;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PositionConsensusReport {
    pub mean_pairwise_drift_m: f32,
    pub outliers: Vec<String>,
    pub outlier_threshold_m: f32,
}

/// For each renderer R, compute the mean per-joint drift between R and
/// every other renderer. R is an outlier if that mean exceeds the median
/// renderer's mean by `outlier_threshold_m` or more.
pub fn position_consensus(
    entries: &[(String, SpringPositions)],
    outlier_threshold_m: f32,
) -> PositionConsensusReport {
    let n = entries.len();
    if n < 3 {
        return PositionConsensusReport {
            mean_pairwise_drift_m: 0.0,
            outliers: Vec::new(),
            outlier_threshold_m,
        };
    }

    // For each renderer compute mean drift vs others (using diff_positions
    // with sufficiently loose thresholds so we just read the numeric drift).
    let mut per_renderer_mean_drift: Vec<(String, f32)> = Vec::with_capacity(n);
    for (i, (name_i, pos_i)) in entries.iter().enumerate() {
        let mut sum = 0.0_f32;
        let mut cnt = 0u32;
        for (j, (_, pos_j)) in entries.iter().enumerate() {
            if i == j {
                continue;
            }
            let r = diff_positions(pos_i, pos_j, f32::INFINITY, f32::INFINITY);
            sum += r.per_joint_max_drift_m;
            cnt += 1;
        }
        let mean = if cnt > 0 { sum / cnt as f32 } else { 0.0 };
        per_renderer_mean_drift.push((name_i.clone(), mean));
    }

    // Median renderer = sorted middle.
    let mut sorted: Vec<f32> = per_renderer_mean_drift
        .iter()
        .map(|(_, m)| *m)
        .collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[n / 2];

    let outliers: Vec<String> = per_renderer_mean_drift
        .iter()
        .filter(|(_, m)| (m - median).abs() >= outlier_threshold_m)
        .map(|(name, _)| name.clone())
        .collect();

    let total: f32 = per_renderer_mean_drift.iter().map(|(_, m)| *m).sum();
    let mean_pairwise_drift_m = total / n as f32;

    PositionConsensusReport {
        mean_pairwise_drift_m,
        outliers,
        outlier_threshold_m,
    }
}
```

- [ ] **Step 5: Wire consensus-diff CLI**

Find the `consensus-diff` subcommand in `crates/vrm-runner/src/cli.rs` (or main.rs). It currently takes `--render <name>=<png>` flags. Add `--render-positions <name>=<positions.json>` flags. After SSIM consensus output, run `position_consensus` and include the report in the JSON output under a `position_consensus` key.

- [ ] **Step 6: Run tests + lint**

```
cargo test -p vrm-diff-engine
cargo test -p vrm-runner
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

All pass.

- [ ] **Step 7: Commit**

```
git add crates/vrm-diff-engine/ crates/vrm-runner/
git commit -m "feat(vrm-diff-engine,vrm-runner): N-way position consensus with outlier flagging"
```

---

## Task 7: three-vrm adapter `dump_bone_positions`

**Files:**
- Modify: `adapters/three-vrm/src/operations.ts`
- Modify: `adapters/three-vrm/src/browser-session.ts` (if bone access lives there)
- Test: `adapters/three-vrm/tests/operations.spec.ts` (or wherever Playwright/unit tests live; locate via `npm test`)

- [ ] **Step 1: Read existing op handlers in `operations.ts`**

Look at how `step_physics` and `reset_physics` are dispatched. The handler signature shape is what you'll mirror. Note where the browser-side `THREE.Bone` objects are accessible from the Node-side handler (typically via Playwright's `page.evaluate`).

- [ ] **Step 2: Write the failing test**

Locate the existing test pattern. If there's a unit test that mocks the browser session, add:

```typescript
import { handleDumpBonePositions } from "../src/operations";

describe("dump_bone_positions", () => {
  it("returns InvalidParams for unknown session", async () => {
    const result = await handleDumpBonePositions({
      session_id: "unknown",
      spring_index: undefined,
    });
    expect(result).toHaveProperty("error");
    expect(result.error.code).toBe(-32602);
  });

  it("returns empty springs for a loaded model with no spring chains", async () => {
    // Use the same test fixture other handlers use for "load_vrm then
    // call op X". Asset = empty.vrm or whichever fixture has no
    // VRMC_springBone extension.
    const session = await loadFixtureSession("empty");
    const result = await handleDumpBonePositions({
      session_id: session.id,
      spring_index: undefined,
    });
    expect(result.springs).toEqual([]);
  });

  it("returns expected joint count for a loaded spring model", async () => {
    const session = await loadFixtureSession("springbone_default");
    const result = await handleDumpBonePositions({
      session_id: session.id,
      spring_index: undefined,
    });
    expect(result.springs).toHaveLength(1);
    expect(result.springs[0].joint_positions).toHaveLength(4); // default 4 joints
  });
});
```

If the test infrastructure for unit-testing handlers doesn't exist, add the handler test as a Playwright integration test in the existing Playwright suite, using the runner harness.

- [ ] **Step 3: Run test to verify it fails**

```
cd adapters/three-vrm && npm test -- --grep dump_bone_positions
```

Expected: handler not found.

- [ ] **Step 4: Implement handler**

In `adapters/three-vrm/src/operations.ts`, add a handler that uses `page.evaluate` to read joint world positions:

```typescript
export async function handleDumpBonePositions(
  params: { session_id: string; spring_index?: number },
  sessions: SessionRegistry, // existing type used by other handlers
): Promise<DumpBonePositionsResult | JsonRpcError> {
  const session = sessions.get(params.session_id);
  if (!session) {
    return {
      error: {
        code: -32602,
        message: `unknown session_id: ${params.session_id}`,
      },
    };
  }

  // In the browser context, read every spring's joint world positions
  // via THREE.Bone.getWorldPosition. springBoneManager exposes joints
  // grouped by their parent spring.
  const result = await session.page.evaluate(
    ({ springIndex }) => {
      // @ts-ignore — set up by the browser-side bootstrap
      const vrm = (window as any).__vrm as any;
      if (!vrm?.springBoneManager) return { springs: [] };

      const springs = vrm.springBoneManager.springs ?? [];
      const filtered =
        typeof springIndex === "number"
          ? springs.slice(springIndex, springIndex + 1)
          : springs;

      // @ts-ignore
      const THREE = (window as any).THREE;
      return {
        springs: filtered.map((spring: any, idx: number) => ({
          name: spring.name ?? `spring_${idx}`,
          joint_positions: spring.joints.map((j: any) => {
            const v = new THREE.Vector3();
            j.bone.getWorldPosition(v);
            return [v.x, v.y, v.z];
          }),
        })),
      };
    },
    { springIndex: params.spring_index ?? null },
  );

  return result as DumpBonePositionsResult;
}
```

If `springBoneManager.springs[].joints[].bone` access path is different in the three-vrm version currently pinned, adjust based on `three-vrm`'s actual API. The Playwright browser-side debugger (`page.evaluate(() => console.log(Object.keys((window as any).__vrm.springBoneManager)))`) is the fastest check.

Register the handler in the dispatcher's method-name map (next to `step_physics`).

- [ ] **Step 5: Run tests**

```
cd adapters/three-vrm && npm test -- --grep dump_bone_positions
```

All three assertions pass.

- [ ] **Step 6: Build**

```
cd adapters/three-vrm && npm run build
```

Build succeeds; no TS errors.

- [ ] **Step 7: Commit**

```
git add adapters/three-vrm/
git commit -m "feat(adapters/three-vrm): dump_bone_positions handler reading THREE.Bone world positions"
```

---

## Task 8: vrm-metal-kit adapter `dump_bone_positions`

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/JsonRpcServer.swift` (dispatch wire-up)
- Test: `adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/JsonRpcServerTests.swift`

- [ ] **Step 1: Read VMK's BoneTrajectoryDumper**

Look at `VRMMetalKit/Sources/VRMMetalKit/BoneTrajectoryDumper.swift` (in the dependency, not our repo — read via `swift package describe` or by browsing the package's source under `.build/checkouts/`). Identify how it accesses live joint world positions. The new op wraps that internal access.

- [ ] **Step 2: Write the failing test**

In `adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/JsonRpcServerTests.swift`, mirror `testStepPhysicsOnUnknownSessionReturnsInvalidParams`:

```swift
func testDumpBonePositionsOnUnknownSessionReturnsInvalidParams() throws {
    let server = JsonRpcServer()
    let req = """
    {"jsonrpc":"2.0","id":1,"method":"dump_bone_positions","params":{"session_id":"unknown"}}
    """.data(using: .utf8)!
    let resp = try server.handle(jsonData: req)
    let json = try JSONSerialization.jsonObject(with: resp) as! [String: Any]
    let error = json["error"] as! [String: Any]
    XCTAssertEqual(error["code"] as! Int, -32602)
}

func testDumpBonePositionsReturnsEmptySpringsForSessionWithoutSpringBones() throws {
    // Use whichever fixture the existing tests use for "load a vrm and
    // call an op on it". Verify result is { "springs": [] } for an
    // asset that has no VRMC_springBone extension.
    let server = JsonRpcServer()
    let loadResp = try server.handle(jsonData: Data(...))
    // ... mirror test setup ...
    let dumpReq = """
    {"jsonrpc":"2.0","id":2,"method":"dump_bone_positions","params":{"session_id":"\(sessionId)"}}
    """.data(using: .utf8)!
    let dumpResp = try server.handle(jsonData: dumpReq)
    let json = try JSONSerialization.jsonObject(with: dumpResp) as! [String: Any]
    let result = json["result"] as! [String: Any]
    let springs = result["springs"] as! [Any]
    XCTAssertEqual(springs.count, 0)
}
```

If the test setup for "load a vrm and call op X" doesn't exist as a helper, use the closest existing pattern (the test file uses a few setup primitives — match style).

- [ ] **Step 3: Run test to verify it fails**

```
cd adapters/vrm-metal-kit && swift test --filter testDumpBonePositions
```

Expected: handler not registered, returns -32601 MethodNotFound (or compile error if you wired the dispatch arm prematurely).

- [ ] **Step 4: Implement handler**

In `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`, add:

```swift
struct DumpBonePositionsParams: Decodable {
    let session_id: String
    let spring_index: Int?
}

struct SpringPositionsDump: Encodable {
    let name: String
    let joint_positions: [[Float]]
}

struct DumpBonePositionsResult: Encodable {
    let springs: [SpringPositionsDump]
}

func handleDumpBonePositions(
    sessions: inout SessionStore,
    params: DumpBonePositionsParams
) -> Result<DumpBonePositionsResult, RpcError> {
    guard let session = sessions.get(params.session_id) else {
        return .failure(RpcError(
            code: -32602,
            message: "unknown session_id: \(params.session_id)",
            data: nil
        ))
    }

    // VMK keeps spring-bone joint world positions live on the renderer
    // after each render/physics step. The access path here must match
    // VMK's published surface. As of VMK 0.14.0: VRMRenderer.model has
    // a springBoneSystem with springs[].joints[].worldPosition (or the
    // equivalent published API — verify against the linked headers at
    // implementation time).
    let renderer = session.renderer
    let springs = renderer.model?.springBoneSystem?.springs ?? []
    let filtered: [Any] = {
        if let idx = params.spring_index {
            guard idx < springs.count else { return [] }
            return [springs[idx]]
        }
        return springs.map { $0 as Any }
    }()

    // PLACEHOLDER TYPE — the cast below names VMK's published spring type.
    // Confirm against `swift package describe` for VRMMetalKit 0.14.0: the
    // type is likely `VRMSpringBoneSpring` but may be `SpringBoneSpring` or
    // similar. Replace with the exact name before this compiles.
    let dumped = filtered.enumerated().map { (i, raw) -> SpringPositionsDump in
        let spring = raw as! VRMSpringBoneSpring // <-- verify exact name
        let name = spring.name ?? "spring_\(i)"
        let joints = spring.joints.map { joint -> [Float] in
            let p = joint.worldPosition
            return [p.x, p.y, p.z]
        }
        return SpringPositionsDump(name: name, joint_positions: joints)
    }

    return .success(DumpBonePositionsResult(springs: dumped))
}
```

The exact VMK access path (`renderer.model?.springBoneSystem?.springs` etc) may differ from what's written above — confirm against the VMK 0.14.0 source. If `BoneTrajectoryDumper` exposes a cleaner read API, prefer that over reaching into the internal type.

- [ ] **Step 5: Wire into JsonRpcServer dispatch**

In `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/JsonRpcServer.swift`, find the `"step_physics"` arm and add a sibling:

```swift
case "dump_bone_positions":
    let params = try JSONDecoder().decode(
        DumpBonePositionsParams.self, from: paramsData)
    let result = handleDumpBonePositions(sessions: &sessions, params: params)
    return try encodeResponse(id: req.id, result: result)
```

- [ ] **Step 6: Build + test**

```
cd adapters/vrm-metal-kit && swift build && swift test
```

All adapter tests pass.

- [ ] **Step 7: Commit**

```
git add adapters/vrm-metal-kit/
git commit -m "feat(adapters/vrm-metal-kit): dump_bone_positions handler wrapping VMK spring system"
```

---

## Task 9: godot-vrm adapter `dump_bone_positions`

**Files:**
- Modify: `adapters/godot-vrm/src/operations.gd`
- Modify: `adapters/godot-vrm/src/session.gd` (only if bone-path lookup needs to be retained)
- Modify: `crates/vrm-godot-shim/src/bridge.rs` (pass-through op)
- Test: `adapters/godot-vrm/tests/test_operations.gd`

- [ ] **Step 1: Read existing op flow**

Read `adapters/godot-vrm/src/operations.gd` and `adapters/godot-vrm/src/session.gd`. Confirm the pattern for adding a new op: the methods list (line ~23 in operations.gd has the array), the dispatch match (line ~43), and the session-level method.

- [ ] **Step 2: Write failing GDScript test**

In `adapters/godot-vrm/tests/test_operations.gd`, mirror existing tests:

```gdscript
func test_dump_bone_positions_unknown_session_returns_invalid_params():
    var ops = preload("res://src/operations.gd").new()
    var resp = ops.dispatch("dump_bone_positions", {"session_id": "unknown"})
    assert_eq(resp.error.code, -32602)

func test_dump_bone_positions_after_load_returns_springs():
    var ops = preload("res://src/operations.gd").new()
    var load_resp = ops.dispatch("load_vrm", {"path": "res://test_assets/springbone_default.vrm"})
    var sid = load_resp.result.session_id
    var resp = ops.dispatch("dump_bone_positions", {"session_id": sid})
    assert_true(resp.has("result"))
    assert_eq(resp.result.springs.size(), 1)
    assert_eq(resp.result.springs[0].joint_positions.size(), 4)
```

If test fixture paths differ, use the matching ones.

- [ ] **Step 3: Run test to verify it fails**

```
cd adapters/godot-vrm && ./tests/run_gdscript_tests.gd
```

Or however the test runner is invoked (check `adapters/godot-vrm/README.md` for the exact command). Expected: method unknown.

- [ ] **Step 4: Implement in operations.gd**

In `adapters/godot-vrm/src/operations.gd`:

```gdscript
# In the methods list (around line 23), add "dump_bone_positions"
const SUPPORTED_METHODS := [
    "load_vrm", "dispose",
    "set_camera", "set_lighting", "set_post_processing",
    "render",
    "step_physics", "reset_physics", "animate_root_transform",
    "dump_bone_positions",  # added
]

# In dispatch match (around line 43), add the case:
"dump_bone_positions":
    outcome = session.dump_bone_positions(
        params if typeof(params) == TYPE_DICTIONARY else {}
    )
```

In `adapters/godot-vrm/src/session.gd`, add:

```gdscript
func dump_bone_positions(params: Dictionary) -> Dictionary:
    # params: { spring_index: int? }
    if not _loaded:
        return {"error": {"code": -32602, "message": "session not loaded"}}

    var springs := []
    var spring_index = params.get("spring_index", null)

    # godot-vrm stores spring chains under VRMSecondary as joint Node3Ds.
    # Walk the secondary tree and emit world positions per chain.
    var secondary := _vrm_root.get_node_or_null("VRMSecondary")
    if secondary == null:
        return {"result": {"springs": []}}

    var i := 0
    for spring_chain in secondary.spring_chains:
        if spring_index != null and i != spring_index:
            i += 1
            continue
        var joint_positions := []
        for joint_node in spring_chain.joint_nodes:
            var p: Vector3 = joint_node.global_position
            joint_positions.append([p.x, p.y, p.z])
        springs.append({
            "name": spring_chain.name if spring_chain.has_method("name") else "spring_%d" % i,
            "joint_positions": joint_positions,
        })
        i += 1

    return {"result": {"springs": springs}}
```

The exact path through `VRMSecondary` / `spring_chains` depends on godot-vrm's API — confirm against the addon source (`adapters/godot-vrm/addons/vrm/`). If the addon doesn't expose chains directly, walk the scene tree for `Node3D` children of the secondary tagged with the joint property.

- [ ] **Step 5: Wire shim bridge (Rust side)**

In `crates/vrm-godot-shim/src/bridge.rs`, the shim is mostly a stdio passthrough. Confirm that adding a new method name doesn't require a registry change. If there's a method allowlist, add `"dump_bone_positions"`.

- [ ] **Step 6: Run tests**

```
cd adapters/godot-vrm && ./tests/run_gdscript_tests.gd
cargo test -p vrm-godot-shim
```

Both pass.

- [ ] **Step 7: Commit**

```
git add adapters/godot-vrm/ crates/vrm-godot-shim/
git commit -m "feat(adapters/godot-vrm): dump_bone_positions reading Node3D.global_position per chain"
```

---

## Task 10: Documentation updates

**Files:**
- Modify: `docs/operation-contract.md`
- Modify: `docs/methodology.md`

- [ ] **Step 1: Read existing op-contract entries to mirror style**

`docs/operation-contract.md` documents the op set. Find the `step_physics` or `reset_physics` entry. Mirror its structure (parameters table, result schema, error envelope, adapter-level support matrix).

- [ ] **Step 2: Add `dump_bone_positions` entry**

Insert after `animate_root_transform`:

```markdown
### `dump_bone_positions`

Returns world-space joint positions for one or all spring-bone chains in
the session, captured as of the most recent state-advancing op (`render`,
`step_physics`, `reset_physics`, `animate_root_transform`). Does NOT
advance physics itself.

**Params:**

| field | type | required | notes |
|---|---|---|---|
| `session_id` | string | yes | from `load_vrm` |
| `spring_index` | int | no | omit to dump all springs; out-of-range returns `-32602` |

**Result:**

```json
{
  "springs": [
    {
      "name": "hair_chain",
      "joint_positions": [[x, y, z], [x, y, z], ...]
    }
  ]
}
```

`joint_positions` is in world space, in the VRM 1.0 coordinate system,
joint-order head-to-tail.

**Errors:**

- `-32602` InvalidParams — unknown `session_id` or out-of-range `spring_index`
- `-32000` Unimplemented — adapter does not have a spring-bone system (e.g.
  univrm L3 in rest-pose-only mode)

**Adapter support (as of phase 1):**

| adapter | status |
|---|---|
| mock | deterministic empty array (model has no springs) |
| three-vrm | implemented |
| vrm-metal-kit | implemented |
| godot-vrm | implemented |
| univrm | returns `-32000 Unimplemented` (phase: "L3") |
```

- [ ] **Step 3: Add methodology section**

In `docs/methodology.md`, add a new section after "Spring bone excitation":

```markdown
## Spring bone position-diff thresholds

Cross-renderer SSIM is necessary but not sufficient for spring-bone tests:
two valid renderers can produce visibly different chain poses because
collision response and time-integration are not pinned by the spec. The
`dump_bone_positions` op exposes per-joint world coordinates so position
divergence can be measured directly.

Two thresholds — single-joint outliers and chain-wide drift are different
bug shapes:

| context | per-joint tolerance | chain-summed tolerance |
|---|---|---|
| settle (no `animate_root_transform`) | 5 mm | 20 mm |
| swing (with `animate_root_transform`) | 10 mm | 40 mm |

Settle thresholds reflect that two correctly-converged renderers should
agree to within sub-cm at equilibrium. Swing thresholds widen because
sub-frame stepping divergence accumulates during animation.

`vrm-runner execute-test-plan --reference-positions <renderer>=<positions.json>`
runs the diff. `consensus-diff --render-positions ...` produces N-way
outlier flagging.

These thresholds are operational, not spec-defined. Future tightening
follows the same trajectory as the cross-renderer SSIM thresholds in
`docs/methodology.md`.
```

- [ ] **Step 4: Commit**

```
git add docs/operation-contract.md docs/methodology.md
git commit -m "docs: dump_bone_positions op spec + spring-bone position-diff thresholds"
```

---

## Task 11: E2E smoke test

**Files:**
- Create: `crates/vrm-runner/tests/dump_positions_smoke.rs` (or whichever workspace integration-test location matches the project's `tests/` convention)

- [ ] **Step 1: Check the existing test layout**

```
ls crates/vrm-runner/tests/ 2>&1
```

If there's an existing integration-test pattern, mirror it. If not, create the test file as a new top-level `tests/` directory under `vrm-runner` (Cargo picks them up automatically).

- [ ] **Step 2: Write the failing test**

Create `crates/vrm-runner/tests/dump_positions_smoke.rs`:

```rust
//! End-to-end smoke for phase 1 infrastructure:
//!   - spawn mock renderer
//!   - execute a minimal plan with --reference-positions
//!   - assert overall_passed = true and position_diff present
//!
//! Mock returns empty springs; reference is also empty; diff trivially
//! passes. This tests the wiring, not the math.

use camino::Utf8PathBuf;
use std::fs;
use vrm_runner::execute::{execute_plan, load_plan, ExecuteOptions};

#[test]
fn execute_plan_with_reference_positions_against_mock_passes() {
    // Build mock renderer binary path (assumes workspace target dir).
    let mock_bin: Utf8PathBuf = env!("CARGO_BIN_EXE_vrm-mock-renderer")
        .to_string()
        .into();

    // Use a generated smoke asset; fixture already exists under
    // assets/generated/. Get the path of smoke_default.test.yaml from
    // the repo root.
    let manifest_dir =
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let plan_path =
        repo_root.join("assets/generated/smoke_default.test.yaml");
    let asset_dir = repo_root.join("assets/generated");
    let plan = load_plan(&plan_path).expect("load plan");

    // Reference positions: empty (mock has no springs)
    let tmp = tempfile::tempdir().unwrap();
    let ref_path = Utf8PathBuf::from_path_buf(
        tmp.path().join("ref_positions.json"),
    ).unwrap();
    fs::write(&ref_path, r#"{"springs":[]}"#).unwrap();

    let output_dir = Utf8PathBuf::from_path_buf(
        tmp.path().join("out"),
    ).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let opts = ExecuteOptions {
        adapter_bin: mock_bin,
        adapter_args: vec![],
        asset_dir,
        output_dir,
        renderer_name: "mock".into(),
        emit_progress_ndjson: false,
        reference: None,
        reference_positions: Some(ref_path),
    };

    let result = execute_plan(&plan, &opts).expect("execute_plan");
    assert!(result.position_diff.is_some());
    let pd = result.position_diff.unwrap();
    // Mock returns 0 springs; reference is 0 springs → diff_positions_one
    // bails with "actual dump contained zero springs". So this test
    // currently expects an error case, not a pass. Adjust the assertion
    // and the early-bail behavior in diff.rs so that "both sides empty"
    // is treated as a structural pass (no chains to diff, vacuously
    // consistent). Update `diff_positions_one` to return a synthetic
    // "all zeros, passed" report when actual.springs.is_empty()
    // && reference.springs.is_empty().
    assert!(pd.passed);
}
```

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p vrm-runner --test dump_positions_smoke
```

Expected: test panics on `.expect("execute_plan")` because `diff_positions_one` returns `Err("actual dump contained zero springs")` when both sides have zero chains. The test surfaces a gap: empty-on-both-sides should be a structural pass, not an error.

- [ ] **Step 4: Fix `diff_positions_one` for both-empty case**

Edit `crates/vrm-runner/src/diff.rs` `diff_positions_one`:

```rust
if actual.springs.is_empty() && reference.springs.is_empty() {
    return Ok(PositionDiffReport {
        per_joint_max_drift_m: 0.0,
        chain_summed_drift_m: 0.0,
        per_joint_tolerance_m: 0.0,
        chain_max_drift_m: 0.0,
        worst_joint_index: 0,
        passed: true,
    });
}
```

Place this check before the spring-count-mismatch bail.

- [ ] **Step 5: Re-run test**

```
cargo test -p vrm-runner --test dump_positions_smoke
```

Passes.

- [ ] **Step 6: Run the full workspace**

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Everything green. Fix any issues before committing.

- [ ] **Step 7: Commit**

```
git add crates/vrm-runner/
git commit -m "test(vrm-runner): E2E smoke wiring dump_bone_positions through mock renderer"
```

---

## Final acceptance

Phase 1 is complete when:

- [ ] All 11 task commits are on the branch
- [ ] `cargo test --workspace` is green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is green
- [ ] `cargo fmt --all -- --check` is green
- [ ] `cd adapters/three-vrm && npm test` is green
- [ ] `cd adapters/vrm-metal-kit && swift test` is green
- [ ] `cd adapters/godot-vrm && ./tests/run_gdscript_tests.gd` (or equivalent runner) is green
- [ ] `docs/operation-contract.md` has the `dump_bone_positions` entry
- [ ] `docs/methodology.md` has the position-diff thresholds section
- [ ] `crates/vrm-s3/src/bin/validate-manifest.rs` accepts manifests with and without `positions_url` fields

Once accepted, phase 2 (Colliders) starts from a stable infrastructure base: collider emission in the generator, the chain-skinned cylinder pointed at the collider, plans, and goldens — no further changes to the op, manifest, or diff engine should be needed for that phase.

## Out of scope for phase 1 (deferred to later phases per spec)

- Plan-level threshold overrides (phase 2+: plans may specify their own per-joint and chain tolerances)
- Multi-spring N-way reduction in `diff_positions_one` (phase 6 multi-chain)
- `execute-test-plan-matrix` runner mode for parameter-coupling self-comparison (phase 7)
- Any asset generator changes — phases 2-6 own that surface
- Methodology updates beyond position-diff thresholds — phases 2/3 will add collider parsing notes etc.
