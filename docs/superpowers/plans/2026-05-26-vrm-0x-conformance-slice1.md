# VRM 0.x Conformance Slice 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land cross-cutting VRM 0.x conformance infrastructure (SpecVersion enum, manifest schema field, vrm-normalize crate, SweepApplicability registry) plus a thin asset surface (`mtoon_basic_v0` 3 variants including one `NotApplicable`, `expressions_preset_basic` v0+v1 pair, plus the existing `avatarA_0_0.vrm` fixture) rendered across all four adapters, with VMK's 180° orientation flip surfaced as a first-class conformance failure in the published site.

**Architecture:** Single `vrm-asset-generator` crate gains `--spec-version 0.x | 1.0` flag. `SpecVersion::{V0, V1}` enum threads through generator CLI, manifest, test plan, ops contract. Read-side dumps (`dump_expression_weights`, `dump_humanoid_pose`, `dump_look_at_state`) gain optional `as_spec_version` param and required `source_spec_version` response field. Normalization (v0→v1 only) lives in new `crates/vrm-normalize/`, called by runner not adapters. Manifest gains required `spec_version` field; runner cross-checks `test_plan.spec_version` ↔ `manifest.spec_version` ↔ adapter-reported `source_spec_version` as three hard-error gates.

**Tech Stack:** Rust 1.88 (vrm-ops, vrm-asset-generator, vrm-test-plan, vrm-s3, vrm-runner, vrm-diff-engine, vrm-normalize), Swift 6.3 + Xcode 26 (vrm-metal-kit adapter), TypeScript + Playwright (three-vrm adapter), C# / Unity 6 (univrm adapter), GDScript + Rust shim (godot-vrm adapter).

**Spec:** `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md` (slice 1).

**Slice 1 success criteria (gate end-of-slice):**

1. Four-adapter diff produced on `mtoon_basic_v0_lit_001` and `expressions_preset_basic_v0`.
2. VMK 180° flip flagged as conformance failure with clear visual signal in published site.
3. `vrm-normalize` round-trip property test passes in CI.
4. Methodology doc section live with spec citations, camera-Z table, and at least one failure-mode example image.
5. `spec_version` field present on every manifest entry; CI validator enforces.
6. Sweep registry symmetry assertion passes — every `*_v0` sweep entry has a 1.0 counterpart or `NotApplicable` reason.

---

## File map

**Create:**
- `crates/vrm-normalize/Cargo.toml`
- `crates/vrm-normalize/src/lib.rs`
- `crates/vrm-normalize/src/expressions.rs`
- `crates/vrm-normalize/src/humanoid.rs`
- `crates/vrm-normalize/src/look_at.rs`
- `crates/vrm-asset-generator/src/vrm_ext_v0.rs`
- `crates/vrm-asset-generator/src/mtoon_common.rs`
- `crates/vrm-asset-generator/src/mtoon_v0.rs`
- `crates/vrm-asset-generator/src/expressions_v0.rs`
- `crates/vrm-asset-generator/src/sweep_v0.rs`
- `test-plans/manual/humanoid/avatarA_0_0.test.yaml`
- `scripts/check-vroid-studio-0x-export.sh` (empirical check, recorded result)
- `scripts/migrate-manifest-spec-version.sh` (backfill `spec_version: "1.0"` on existing entries)
- `docs/findings.md` entries (empirical-check results)

**Modify:**
- `Cargo.toml` (workspace) — add `crates/vrm-normalize` member
- `crates/vrm-ops/src/lib.rs` — `pub mod spec_version` re-export
- `crates/vrm-ops/src/spec_version.rs` (NEW small module) — `SpecVersion` enum
- `crates/vrm-ops/src/tools.rs` — add `as_spec_version: Option<SpecVersion>` to dump params; `source_spec_version: SpecVersion` to dump results; new error code `-32001 NormalizationDirectionUnsupported`
- `crates/vrm-test-plan/src/lib.rs` — add `spec_version: SpecVersion` field on `TestPlan` (required; default `V1` for back-compat parse)
- `crates/vrm-s3/src/manifest.rs` — add `spec_version: SpecVersion` to `ManifestEntry` (required; default `V1` for parse-back-compat)
- `crates/vrm-s3/src/bin/validate-manifest.rs` — enforce `spec_version` presence on new entries (post-migration)
- `crates/vrm-asset-generator/src/lib.rs` — re-export `SpecVersion`; declare `SweepApplicability` + `NotApplicableReason` enums
- `crates/vrm-asset-generator/src/cli.rs` — add `--spec-version 0.x | 1.0` flag
- `crates/vrm-asset-generator/src/mtoon.rs` — slim; delegate shared math to `mtoon_common.rs`
- `crates/vrm-asset-generator/src/sweep.rs` — add `mtoon_basic_v0_sweep()`; add `expressions_preset_basic_sweep()` (v0+v1 pair); add registry symmetry assertion test
- `crates/vrm-runner/src/execute.rs` — runner cross-checks plan ↔ manifest ↔ adapter `source_spec_version`; enforces per-spec-version camera convention; calls `vrm-normalize` when `as_spec_version` requested
- `adapters/three-vrm/src/operations.ts` — report `source_spec_version` on dumps
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` — report `source_spec_version` on dumps
- `adapters/univrm/UniVrmAdapter/Operations.cs` — pass `canLoadVrm0X: true` to `Vrm10.LoadPathAsync`; report `source_spec_version` on dumps
- `adapters/godot-vrm/src/operations.gd` — report `source_spec_version` on dumps
- `crates/vrm-godot-shim/src/bridge.rs` — pass-through of `source_spec_version` on dump responses
- `docs/methodology.md` — new "VRM 0.x conformance" section
- `docs/operation-contract.md` — document `as_spec_version` param + `source_spec_version` response field + error code -32001
- `site/src/manifest.ts` (or wherever the manifest type is) — add `spec_version`; add filter UI + per-card badge
- `assets/humanoid/.gitignore` or fixture install script — Tier 2 canonical `vroid_default_F_0_0.vrm` install path
- `scripts/install-humanoid-fixtures.sh` — install 0.x canonical fixture if available; else document fallback
- `goldens/manifest.json` — migration commit backfills existing entries with `spec_version: "1.0"`

**Phases:**
- **Phase A** (days 1–3) — Foundation: enums, schema, crate skeleton, empirical checks
- **Phase B** (days 4–9) — Generator emit: 0.x extension, MToon shared math, expressions, sweep registry symmetry
- **Phase C** (days 10–17) — Adapter wiring + mid-slice checkpoint
- **Phase D** (days 18–21) — Normalization, round-trip property test, methodology doc, site filter, end-of-slice

---

# Phase A — Foundation (days 1–3)

Gate at end of Phase A: manifest schema committed; existing 1.0 entries backfilled; `vrm-normalize` crate skeleton compiles; empirical-check findings recorded.

## Task 1: `SpecVersion` enum in vrm-ops

**Files:**
- Create: `crates/vrm-ops/src/spec_version.rs`
- Modify: `crates/vrm-ops/src/lib.rs:1` (add `pub mod spec_version;` and re-export)
- Test: inline in `crates/vrm-ops/src/spec_version.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vrm-ops/src/spec_version.rs`:

```rust
//! Spec version enum — wire form is `"0.x"` / `"1.0"`. Threaded through
//! generator CLI, manifest schema, test plan, ops contract.
//!
//! See `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`
//! for the design rationale.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum SpecVersion {
    #[serde(rename = "0.x")]
    V0,
    #[serde(rename = "1.0")]
    V1,
}

impl SpecVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            SpecVersion::V0 => "0.x",
            SpecVersion::V1 => "1.0",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_wire_form() {
        assert_eq!(serde_json::to_string(&SpecVersion::V0).unwrap(), "\"0.x\"");
        assert_eq!(serde_json::to_string(&SpecVersion::V1).unwrap(), "\"1.0\"");
    }

    #[test]
    fn deserializes_from_wire_form() {
        let v0: SpecVersion = serde_json::from_str("\"0.x\"").unwrap();
        let v1: SpecVersion = serde_json::from_str("\"1.0\"").unwrap();
        assert_eq!(v0, SpecVersion::V0);
        assert_eq!(v1, SpecVersion::V1);
    }

    #[test]
    fn rejects_unknown_wire_form() {
        assert!(serde_json::from_str::<SpecVersion>("\"2.0\"").is_err());
        assert!(serde_json::from_str::<SpecVersion>("\"v0\"").is_err());
    }

    #[test]
    fn as_str_round_trips() {
        assert_eq!(SpecVersion::V0.as_str(), "0.x");
        assert_eq!(SpecVersion::V1.as_str(), "1.0");
    }
}
```

- [ ] **Step 2: Run test to verify it fails (module not yet declared in lib.rs)**

Run: `cargo test -p vrm-ops spec_version --lib`
Expected: FAIL with "module spec_version is not declared" or similar.

- [ ] **Step 3: Declare module in lib.rs**

Edit `crates/vrm-ops/src/lib.rs`. Add near the top, alongside existing module declarations:

```rust
pub mod spec_version;
pub use spec_version::SpecVersion;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vrm-ops spec_version --lib`
Expected: 4 tests pass.

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p vrm-ops --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-ops/src/spec_version.rs crates/vrm-ops/src/lib.rs
git commit -m "feat(vrm-ops): SpecVersion enum (V0 / V1) — foundation for 0.x conformance"
```

---

## Task 2: `spec_version` field on `TestPlan`

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs:7-30` (TestPlan struct)
- Modify: `crates/vrm-test-plan/Cargo.toml` (add `vrm-ops` dep if absent)
- Test: append `#[cfg(test)] mod spec_version_tests` to `crates/vrm-test-plan/src/lib.rs`

- [ ] **Step 1: Verify Cargo dep**

Check `crates/vrm-test-plan/Cargo.toml`. If `vrm-ops` is not under `[dependencies]`, add:

```toml
vrm-ops = { path = "../vrm-ops" }
```

- [ ] **Step 2: Write the failing test**

Append to `crates/vrm-test-plan/src/lib.rs`:

```rust
#[cfg(test)]
mod spec_version_tests {
    use super::*;
    use vrm_ops::SpecVersion;

    fn minimal_yaml_with_spec_version(v: &str) -> String {
        format!(r#"
id: t
spec_version: "{v}"
spec_section: test
asset: a.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: {{ dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }}
  ambient: {{ color: [1, 1, 1], intensity: 0.2 }}
output:
  width: 256
  height: 256
diff:
  ssim_threshold: 0.95
"#)
    }

    #[test]
    fn parses_spec_version_v0() {
        let yaml = minimal_yaml_with_spec_version("0.x");
        let plan: TestPlan = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(plan.spec_version, SpecVersion::V0);
    }

    #[test]
    fn parses_spec_version_v1() {
        let yaml = minimal_yaml_with_spec_version("1.0");
        let plan: TestPlan = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(plan.spec_version, SpecVersion::V1);
    }

    #[test]
    fn defaults_to_v1_when_absent() {
        // Back-compat: legacy plans without spec_version parse as VRM 1.0.
        let yaml = r#"
id: t
spec_section: test
asset: a.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: { dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }
  ambient: { color: [1, 1, 1], intensity: 0.2 }
output:
  width: 256
  height: 256
diff:
  ssim_threshold: 0.95
"#;
        let plan: TestPlan = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(plan.spec_version, SpecVersion::V1);
    }
}
```

- [ ] **Step 3: Run test to verify it fails (field doesn't exist)**

Run: `cargo test -p vrm-test-plan spec_version --lib`
Expected: FAIL with "no field `spec_version` on type `TestPlan`".

- [ ] **Step 4: Add field to `TestPlan` struct**

Edit `crates/vrm-test-plan/src/lib.rs`. Add `use vrm_ops::SpecVersion;` near the top. Add field to `TestPlan` (after `pub id: String,`):

```rust
#[serde(default = "default_spec_version_v1")]
pub spec_version: SpecVersion,
```

Add the default helper near the bottom of the file (or beside other small helpers):

```rust
fn default_spec_version_v1() -> SpecVersion {
    SpecVersion::V1
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vrm-test-plan spec_version --lib`
Expected: 3 tests pass.

- [ ] **Step 6: Verify existing tests still pass (back-compat)**

Run: `cargo test -p vrm-test-plan --lib`
Expected: all existing tests pass (back-compat default kicks in).

- [ ] **Step 7: Clippy**

Run: `cargo clippy -p vrm-test-plan --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs crates/vrm-test-plan/Cargo.toml
git commit -m "feat(vrm-test-plan): spec_version field on TestPlan (defaults V1 for back-compat)"
```

---

## Task 3: `SweepApplicability` + `NotApplicableReason` enums

**Files:**
- Modify: `crates/vrm-asset-generator/src/lib.rs` (re-export SpecVersion; add SweepApplicability + NotApplicableReason)
- Test: inline `#[cfg(test)]` in `crates/vrm-asset-generator/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-asset-generator/src/lib.rs`:

```rust
pub use vrm_ops::SpecVersion;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum SweepApplicability {
    Applicable,
    NotApplicable { reason: NotApplicableReason },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NotApplicableReason {
    PerJointStiffnessV1Only,
    CapsuleColliderV1Only,
    ExtendedCollidersV1Only,
    OutlineLightingMixV1Only,
    VrmaIsVrm1Era,
    // Extend with new variants as discovered; never use free text.
}

#[cfg(test)]
mod applicability_tests {
    use super::*;

    #[test]
    fn applicable_round_trips() {
        let a = SweepApplicability::Applicable;
        let s = serde_json::to_string(&a).unwrap();
        assert_eq!(s, r#"{"kind":"Applicable"}"#);
        let back: SweepApplicability = serde_json::from_str(&s).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn not_applicable_carries_reason() {
        let n = SweepApplicability::NotApplicable {
            reason: NotApplicableReason::OutlineLightingMixV1Only,
        };
        let s = serde_json::to_string(&n).unwrap();
        // Confirm the reason variant name surfaces in the wire format.
        assert!(s.contains("OutlineLightingMixV1Only"), "got {s}");
        let back: SweepApplicability = serde_json::from_str(&s).unwrap();
        assert_eq!(back, n);
    }
}
```

- [ ] **Step 2: Verify Cargo dep**

Check `crates/vrm-asset-generator/Cargo.toml`. Ensure `vrm-ops` is under `[dependencies]`. If absent, add `vrm-ops = { path = "../vrm-ops" }`.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator applicability --lib`
Expected: 2 tests pass.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/lib.rs crates/vrm-asset-generator/Cargo.toml
git commit -m "feat(vrm-asset-generator): SweepApplicability + NotApplicableReason enums"
```

---

## Task 4: Manifest gains `spec_version` field

**Files:**
- Modify: `crates/vrm-s3/src/manifest.rs:34-80` (ManifestEntry struct)
- Modify: `crates/vrm-s3/Cargo.toml` (add `vrm-ops` dep if absent)
- Test: append `#[cfg(test)] mod spec_version_tests` to `crates/vrm-s3/src/manifest.rs`

- [ ] **Step 1: Verify Cargo dep**

Check `crates/vrm-s3/Cargo.toml`. Add if absent: `vrm-ops = { path = "../vrm-ops" }`.

- [ ] **Step 2: Write the failing test**

Append to `crates/vrm-s3/src/manifest.rs`:

```rust
#[cfg(test)]
mod spec_version_tests {
    use super::*;
    use vrm_ops::SpecVersion;

    fn minimal_entry_json(spec_version_block: &str) -> String {
        // 64-char zero blake3 placeholder; valid shape for serde parse,
        // not for actual hash verification.
        let blake3 = "0".repeat(64);
        format!(r#"{{
  "test_id": "t",
  "renderer_name": "r",
  "renderer_version": "0",
  "git_hash": "abc",
  "renderer_host": "host",
  {spec_version_block}
  "image_url": "s3://b/r/t.png",
  "image_blake3": "{blake3}",
  "submitted_at": "2026-05-26T00:00:00Z"
}}"#)
    }

    #[test]
    fn parses_spec_version_v0() {
        let json = minimal_entry_json("\"spec_version\": \"0.x\",");
        let e: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.spec_version, SpecVersion::V0);
    }

    #[test]
    fn parses_spec_version_v1() {
        let json = minimal_entry_json("\"spec_version\": \"1.0\",");
        let e: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.spec_version, SpecVersion::V1);
    }

    #[test]
    fn defaults_to_v1_when_absent() {
        let json = minimal_entry_json("");
        let e: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.spec_version, SpecVersion::V1);
    }
}
```

(If the existing manifest tests use a different fixture builder, adapt the JSON to match the actual `SubmissionMetadata` shape — the salient bit is adding `"spec_version": "..."` alongside the existing fields.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vrm-s3 spec_version --lib`
Expected: FAIL with "no field `spec_version`".

- [ ] **Step 4: Add field to `ManifestEntry`**

Edit `crates/vrm-s3/src/manifest.rs`. Add `use vrm_ops::SpecVersion;` near the top. Add to `ManifestEntry` (after `pub test_id: String,`):

```rust
#[serde(default = "default_spec_version_v1")]
pub spec_version: SpecVersion,
```

Add the helper at the end of the file:

```rust
fn default_spec_version_v1() -> SpecVersion {
    SpecVersion::V1
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vrm-s3 spec_version --lib`
Expected: 3 tests pass.

- [ ] **Step 6: Verify existing tests still pass**

Run: `cargo test -p vrm-s3 --lib`
Expected: all existing tests pass (back-compat default).

- [ ] **Step 7: Clippy**

Run: `cargo clippy -p vrm-s3 --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/vrm-s3/src/manifest.rs crates/vrm-s3/Cargo.toml
git commit -m "feat(vrm-s3): spec_version field on ManifestEntry (defaults V1)"
```

---

## Task 5: Manifest validator enforces `spec_version` post-migration

**Files:**
- Modify: `crates/vrm-s3/src/bin/validate-manifest.rs` (add validation for spec_version)
- Modify: `crates/vrm-s3/src/validation.rs` (if validation logic lives here)
- Test: integration test in `crates/vrm-s3/tests/validate_manifest_spec_version.rs` (new)

- [ ] **Step 1: Inspect current validator structure**

Read `crates/vrm-s3/src/bin/validate-manifest.rs` and `crates/vrm-s3/src/validation.rs` to identify where per-entry validation runs. Typical pattern: a function `validate_entry(&ManifestEntry) -> Result<(), Vec<ValidationError>>`.

- [ ] **Step 2: Write failing integration test**

Create `crates/vrm-s3/tests/validate_manifest_spec_version.rs`:

```rust
//! Tests that the manifest validator surfaces `spec_version` mismatches
//! between the declared field and `spec_section` text inference.

use vrm_s3::manifest::{Manifest, ManifestEntry};
use vrm_s3::validation::validate_entry;
use vrm_ops::SpecVersion;

fn entry_with(spec_version: SpecVersion, spec_section: &str) -> ManifestEntry {
    // Build a minimal valid entry; details depend on ManifestEntry shape.
    // Use the existing test-fixture helpers if present; otherwise hand-build.
    let mut e: ManifestEntry = serde_json::from_str(&format!(r#"{{
  "test_id": "t",
  "renderer_name": "r",
  "renderer_version": "0",
  "git_hash": "abc",
  "renderer_host": "host",
  "spec_version": "{}",
  "image_url": "s3://b/r/t.png",
  "image_blake3": "{}",
  "submitted_at": "2026-05-26T00:00:00Z"
}}"#, spec_version.as_str(), "0".repeat(64))).expect("entry parse");
    e.test_id = format!("{spec_section}_{}", spec_version.as_str());
    e
}

#[test]
fn rejects_spec_section_says_0x_but_field_says_1_0() {
    // If the validator can disambiguate, this should fail validation.
    let mut e = entry_with(SpecVersion::V1, "vrm-0.x-mtoon");
    e.test_id = "mtoon_basic_v0_lit".into();
    let result = validate_entry(&e);
    assert!(result.is_err(), "expected validation error for v0 test_id with V1 spec_version");
}

#[test]
fn accepts_consistent_v0() {
    let mut e = entry_with(SpecVersion::V0, "VRM 0.x MToon");
    e.test_id = "mtoon_basic_v0_lit_001".into();
    let result = validate_entry(&e);
    assert!(result.is_ok(), "expected validation pass; got {result:?}");
}

#[test]
fn accepts_consistent_v1() {
    let mut e = entry_with(SpecVersion::V1, "VRM 1.0 MToon");
    e.test_id = "mtoon_basic_lit_001".into();
    let result = validate_entry(&e);
    assert!(result.is_ok(), "expected validation pass; got {result:?}");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vrm-s3 --test validate_manifest_spec_version`
Expected: FAIL — likely either compile errors (validate_entry signature) or test failures (no inference logic yet).

- [ ] **Step 4: Implement spec_version cross-check**

In `crates/vrm-s3/src/validation.rs` (or wherever per-entry validation lives), add a check: when `test_id` contains `_v0` or `_v0_` AND `spec_version == V1`, return an error. Conversely, when `test_id` contains `_v0` AND `spec_version == V0`, pass; when neither contains `_v0` AND `spec_version == V1`, pass.

Sketch (adapt to existing structure):

```rust
pub fn validate_entry(entry: &ManifestEntry) -> Result<(), ValidationError> {
    // ... existing checks ...

    let id_implies_v0 = entry.test_id.contains("_v0_") || entry.test_id.ends_with("_v0");
    let id_implies_v1 = !id_implies_v0; // Default 1.0 unless explicitly _v0
    match (id_implies_v0, entry.spec_version) {
        (true, vrm_ops::SpecVersion::V1) => {
            return Err(ValidationError::SpecVersionMismatch {
                test_id: entry.test_id.clone(),
                declared: "1.0".into(),
                inferred_from_test_id: "0.x".into(),
            });
        }
        (false, vrm_ops::SpecVersion::V0) if !entry.test_id.is_empty() => {
            return Err(ValidationError::SpecVersionMismatch {
                test_id: entry.test_id.clone(),
                declared: "0.x".into(),
                inferred_from_test_id: "1.0".into(),
            });
        }
        _ => {}
    }

    Ok(())
}
```

Add to the `ValidationError` enum:

```rust
SpecVersionMismatch { test_id: String, declared: String, inferred_from_test_id: String },
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vrm-s3 --test validate_manifest_spec_version`
Expected: 3 tests pass.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p vrm-s3 --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-s3/src/validation.rs crates/vrm-s3/tests/validate_manifest_spec_version.rs crates/vrm-s3/src/bin/validate-manifest.rs
git commit -m "feat(vrm-s3): validate-manifest cross-checks spec_version against test_id naming"
```

---

## Task 6: Migration — backfill `spec_version: "1.0"` on existing manifest entries

**Files:**
- Create: `scripts/migrate-manifest-spec-version.sh`
- Modify: `goldens/manifest.json` (data, via script)

- [ ] **Step 1: Write the migration script**

Create `scripts/migrate-manifest-spec-version.sh`:

```bash
#!/usr/bin/env bash
# One-shot migration: backfill spec_version: "1.0" on every existing
# manifest entry that lacks it. Idempotent — re-running is a no-op.
#
# Usage: scripts/migrate-manifest-spec-version.sh [manifest.json]
set -euo pipefail
MANIFEST="${1:-goldens/manifest.json}"
TMP="$(mktemp)"

if ! command -v jq >/dev/null; then
    echo "error: jq is required" >&2
    exit 1
fi

# For every entry that lacks spec_version, add it set to "1.0".
jq '(.entries[] | select(.spec_version == null)) |= (. + {spec_version: "1.0"})' \
    "$MANIFEST" > "$TMP"

mv "$TMP" "$MANIFEST"
echo "Backfilled spec_version on $(jq '.entries | length' "$MANIFEST") entries in $MANIFEST"
```

```bash
chmod +x scripts/migrate-manifest-spec-version.sh
```

- [ ] **Step 2: Snapshot pre-migration entry count**

```bash
jq '.entries | length' goldens/manifest.json
```

Record the count (e.g., `N`) — used to verify post-migration sanity.

- [ ] **Step 3: Run the migration**

```bash
scripts/migrate-manifest-spec-version.sh
```

Expected: prints `Backfilled spec_version on N entries in goldens/manifest.json`.

- [ ] **Step 4: Verify every entry now has `spec_version: "1.0"`**

```bash
jq '[.entries[] | select(.spec_version != "1.0")] | length' goldens/manifest.json
```

Expected: `0`.

- [ ] **Step 5: Verify validator passes on the migrated manifest**

```bash
cargo run -p vrm-s3 --bin validate-manifest -- goldens/manifest.json
```

Expected: validator reports zero errors.

- [ ] **Step 6: Verify migration is idempotent**

```bash
scripts/migrate-manifest-spec-version.sh
jq '[.entries[] | select(.spec_version != "1.0")] | length' goldens/manifest.json
```

Expected: still `0`; script reports same count.

- [ ] **Step 7: Commit**

```bash
git add scripts/migrate-manifest-spec-version.sh goldens/manifest.json
git commit -m "chore(goldens): backfill spec_version: \"1.0\" on existing manifest entries"
```

---

## Task 7: `vrm-normalize` crate skeleton

**Files:**
- Create: `crates/vrm-normalize/Cargo.toml`
- Create: `crates/vrm-normalize/src/lib.rs`
- Create: `crates/vrm-normalize/src/expressions.rs` (empty module stub)
- Create: `crates/vrm-normalize/src/humanoid.rs` (empty module stub)
- Create: `crates/vrm-normalize/src/look_at.rs` (empty module stub)
- Modify: `Cargo.toml` (workspace) — add `crates/vrm-normalize` to `members`

- [ ] **Step 1: Create the crate manifest**

Create `crates/vrm-normalize/Cargo.toml`:

```toml
[package]
name = "vrm-normalize"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
description = "v0 → v1 normalization for VRM dump responses; called by runner, not adapters"

[dependencies]
vrm-ops = { path = "../vrm-ops" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"

[dev-dependencies]
```

- [ ] **Step 2: Create the lib root**

Create `crates/vrm-normalize/src/lib.rs`:

```rust
//! v0 → v1 normalization for VRM dump responses.
//!
//! This crate is called by the **runner** (never by adapters) so that
//! four adapter implementations of normalization don't produce four bug
//! surfaces. Normalization is one-directional and lossy:
//!
//! - v0 → v1 has a documented preset mapping table (joy→happy, etc.).
//! - v1 → v0 has no lossless mapping and is rejected.
//! - v0 custom blendshapes are passed through with `custom:<name>` markers.
//!
//! See `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`.

pub mod expressions;
pub mod humanoid;
pub mod look_at;

use thiserror::Error;
use vrm_ops::SpecVersion;

#[derive(Debug, Error)]
pub enum NormalizeError {
    #[error("normalization direction unsupported: cannot project {from:?} dump as {to:?}")]
    DirectionUnsupported { from: SpecVersion, to: SpecVersion },
}
```

- [ ] **Step 3: Create the module stubs**

Create `crates/vrm-normalize/src/expressions.rs`:

```rust
//! Expression-preset normalization. v0 `blendShapeMaster` preset names
//! → v1 `VRMC_vrm.expressions.preset` preset names.

// (Populated in Task 31.)
```

Create `crates/vrm-normalize/src/humanoid.rs`:

```rust
//! Humanoid bone-name normalization. Most names are identical between v0
//! and v1; the renames are minimal.

// (Populated in Task 32.)
```

Create `crates/vrm-normalize/src/look_at.rs`:

```rust
//! `look_at` state normalization between v0 and v1 shape.

// (Populated in Task 33.)
```

- [ ] **Step 4: Add to workspace members**

Edit the workspace `Cargo.toml` (top-level). Add `"crates/vrm-normalize"` to `members`:

```toml
[workspace]
members = [
    # ... existing entries ...
    "crates/vrm-normalize",
]
```

- [ ] **Step 5: Verify the crate compiles**

Run: `cargo build -p vrm-normalize`
Expected: builds cleanly.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p vrm-normalize --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-normalize/ Cargo.toml
git commit -m "feat(vrm-normalize): crate skeleton — v0→v1 normalization, runner-side"
```

---

## Task 8: Empirical check — VRoid Studio 2.12.0 0.x export availability

**Files:**
- Create: `scripts/check-vroid-studio-0x-export.sh`
- Modify: `docs/findings.md` (append entry)

This is an **investigation task**, not implementation. It records an empirical finding that gates downstream Tier 2 fixture sourcing.

- [ ] **Step 1: Write the check script**

Create `scripts/check-vroid-studio-0x-export.sh`:

```bash
#!/usr/bin/env bash
# Determine whether VRoid Studio 2.12.0 still ships the "Export → VRM 0.x" path.
#
# Methodology:
# 1. Open VRoid Studio (manual — Studio is a GUI app, no CLI).
# 2. Open any character.
# 3. File → Export → check menu for "Export as VRM 0.x" entry alongside "Export as VRM".
# 4. Record: present (continue with re-export), removed (fall back to alternate sourcing).
#
# This script just emits the methodology — the human runs the check.
set -euo pipefail
cat <<'EOF'
VRoid Studio 0.x export availability check — manual procedure:

1. Launch VRoid Studio (version 2.12.0 confirmed; check Settings → About).
2. Open any saved character (or create a new "Default Female" preset).
3. Navigate to File → Export.
4. Check the export-format dropdown:
   - "VRM" → 1.0 path (default).
   - "VRM 0.x" or "Legacy VRM" → 0.x path still present. RECORD: AVAILABLE.
   - No 0.x option visible → RECORD: REMOVED.

Append the finding to docs/findings.md under a new entry dated $(date +%Y-%m-%d):
"VRoid Studio 0.x export availability: <AVAILABLE | REMOVED>".

Fallbacks if REMOVED:
- Source 0.x VRoid Studio 1.x installer (older release).
- Use VRoid Hub-sourced 0.x content (license-vetted, attribution-required).
- Drop the vroid_default_F_0_0 fixture from slice 1; rely on avatarA_0_0 alone.
EOF
```

```bash
chmod +x scripts/check-vroid-studio-0x-export.sh
```

- [ ] **Step 2: Run the check**

```bash
scripts/check-vroid-studio-0x-export.sh
```

Then perform the manual steps. Decision-record the outcome.

- [ ] **Step 3: Record finding in docs/findings.md**

Append to `docs/findings.md` (insert near the top under a new dated entry):

```markdown
## 2026-MM-DD — VRoid Studio 2.12.0 0.x export availability

**Check.** VRoid Studio 2.12.0, File → Export, format dropdown inspected.

**Result.** <AVAILABLE | REMOVED>.

**Implication.**
- AVAILABLE → re-export VRoid default character through the 0.x path; land as `assets/humanoid/vroid_default_F_0_0.vrm` alongside the existing 1.0 fixture. Slice 1 Tier 2 canonical fixture proceeds as designed.
- REMOVED → fall back per slice-1 risk mitigation: <chosen fallback>. Slice schedule absorbs by <how>.
```

Fill in the placeholders with the actual result.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-vroid-studio-0x-export.sh docs/findings.md
git commit -m "docs(findings): VRoid Studio 0.x export availability check — <AVAILABLE | REMOVED>"
```

---

## Task 9: Empirical check — VMK 180° flip location

**Files:**
- Modify: `docs/findings.md` (append entry)

Investigation task. Determines whether the flip is in VMK's adapter shim (local fix path) or in upstream VRMMetalKit (file-against-self path).

- [ ] **Step 1: Inspect the pinned VRMMetalKit revision**

```bash
cat adapters/vrm-metal-kit/Package.swift | grep -A2 "VRMMetalKit\|url:"
```

Note the pinned revision (commit hash or version tag).

- [ ] **Step 2: Search the adapter shim for orientation handling**

```bash
grep -rn -i "180\|flip\|rotate.*Y\|orientation" adapters/vrm-metal-kit/Sources/ 2>&1
```

If any matches reference a 180° rotation applied at adapter level (e.g., in `load_vrm` or `set_camera` handling), the flip is **local**.

- [ ] **Step 3: If adapter shim is clean, search upstream**

If step 2 returned nothing relevant, clone the pinned revision of VRMMetalKit into a scratch dir and grep:

```bash
TEMP_DIR=$(mktemp -d)
git clone --depth=1 https://github.com/<upstream-repo-from-Package.swift> "$TEMP_DIR/VRMMetalKit"
cd "$TEMP_DIR/VRMMetalKit"
git checkout <pinned-revision>
grep -rn -i "180\|flip.*Y\|VRM 0\|isVRM0\|vrm0" Sources/
```

Record:
- File + line(s) where the flip is applied.
- Whether the flip is unconditional or guarded by spec-version detection.

- [ ] **Step 4: Record finding in docs/findings.md**

Append to `docs/findings.md`:

```markdown
## 2026-MM-DD — VMK 180° flip on VRM 0.x: location and structurality

**Pinned VRMMetalKit revision:** `<revision>` (from `adapters/vrm-metal-kit/Package.swift`).

**Location.** <ADAPTER_SHIM | UPSTREAM_LIBRARY>. Line(s): <file:line(s)>.

**Structurality.** <VESTIGIAL | LOAD_BEARING>. Reasoning: <why — e.g., guarded by VRM 0.x detection; tied to ARKit alignment; etc.>.

**Implication.**
- ADAPTER_SHIM + VESTIGIAL → one-line adapter fix possible; slice 1 still surfaces the conformance failure for the published site (demonstrates the suite catches it), then fix lands as a separate PR.
- UPSTREAM_LIBRARY → file `docs/upstream/VMK-vrm-0x-orientation.md` issue; flag stays open through slices 1–4.
- LOAD_BEARING (ARKit-coupled) → issue filed but doesn't close quickly; methodology doc explains why the conformance flag stands.
```

- [ ] **Step 5: If upstream issue needed, file the upstream issue stub**

If the result is UPSTREAM_LIBRARY, create `docs/upstream/VMK-vrm-0x-orientation.md` following the pattern of existing files in that directory (look at e.g. `docs/upstream/VMK-vrma-lookat-renderer-propagation.md`). Include:
- Reproducer (path to slice 1's 0.x asset + which renderer call triggers the wrong-direction render).
- Spec citation (`docs/upstream-specs/vrm-specification/specification/0.0/README.md:238`).
- Suggested fix shape (gate the 180° flip on `specVersion == "1.0"` only, or remove).

- [ ] **Step 6: Commit**

```bash
git add docs/findings.md docs/upstream/VMK-vrm-0x-orientation.md 2>/dev/null || git add docs/findings.md
git commit -m "docs(findings): VMK 180° flip diagnostic — <location, structurality>"
```

---

## Task 10: Empirical check — `mrxz/vrm-validator` 0.x coverage

**Files:**
- Modify: `docs/findings.md` (append entry)

Investigation task. Determines whether `vrm-validator-wrap` can validate 0.x assets, or whether the slice 1 0.x corpus needs a validator exemption in CI.

- [ ] **Step 1: Install the validator if not already present**

```bash
ls .tools/vrm-validator-cli 2>/dev/null || scripts/install-validator.sh
```

- [ ] **Step 2: Try validating an existing 0.x asset**

```bash
.tools/vrm-validator-cli validate assets/humanoid/avatarA_0_0.vrm 2>&1
```

Record:
- Exit code.
- Stderr/stdout output.
- Whether 0.x is accepted, rejected, or accepted-with-warnings.

- [ ] **Step 3: Check the wrap crate's behavior**

```bash
cargo test -p vrm-validator-wrap -- --ignored
```

Find tests touching VRM 0.x. If none exist, write a quick smoke:

```rust
// crates/vrm-validator-wrap/tests/vrm_0x_smoke.rs (new)
#[test]
#[ignore = "requires .tools/vrm-validator-cli"]
fn validates_avatar_a_0_0() {
    let result = vrm_validator_wrap::validate("assets/humanoid/avatarA_0_0.vrm");
    eprintln!("validator result on VRM 0.x: {result:?}");
    // Record empirically — don't assert until we know the expected behavior.
}
```

```bash
cargo test -p vrm-validator-wrap --test vrm_0x_smoke -- --ignored --nocapture
```

- [ ] **Step 4: Record finding in docs/findings.md**

Append:

```markdown
## 2026-MM-DD — mrxz/vrm-validator coverage of VRM 0.x

**Validator binary:** `.tools/vrm-validator-cli` (installed via `scripts/install-validator.sh`).

**Result on `avatarA_0_0.vrm`:** <ACCEPTED | REJECTED | ACCEPTED_WITH_WARNINGS>. Output: <relevant lines>.

**Implication.**
- ACCEPTED → no validator-side work needed for slice 1 0.x corpus; CI validator gate applies uniformly to 0.x and 1.0.
- REJECTED → validator exemption needed for 0.x entries in CI. Fall back to a thin schema-validation pass against `docs/upstream-specs/vrm-specification/specification/0.0/schema/`. Implementation: `scripts/install-validator.sh` checks asset's `extensionsUsed`; routes 0.x to schema-only path.
- ACCEPTED_WITH_WARNINGS → document the warnings; slice 1 corpus continues but each warning becomes a methodology note.
```

- [ ] **Step 5: Commit**

```bash
git add docs/findings.md crates/vrm-validator-wrap/tests/vrm_0x_smoke.rs 2>/dev/null || git add docs/findings.md
git commit -m "docs(findings): vrm-validator coverage of VRM 0.x — <ACCEPTED | REJECTED | WARN>"
```

**End of Phase A.** Mid-phase checkpoint: review findings entries; confirm Phase B can proceed with the schema and crate skeleton in place.

---

# Phase B — Generator emit (days 4–9)

Gate at end of Phase B: v0 assets emit and pass the validator (or documented exemption); sweep registry symmetry assertion passes; `mtoon_basic_v0` 3-variant sweep + `expressions_preset_basic` v0+v1 pair both buildable.

## Task 11: `--spec-version` CLI flag

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (add the flag)
- Modify: `crates/vrm-asset-generator/src/main.rs` (thread to subcommands)
- Test: integration test `crates/vrm-asset-generator/tests/cli_spec_version.rs` (new)

- [ ] **Step 1: Inspect existing CLI**

Read `crates/vrm-asset-generator/src/cli.rs` to understand the existing `clap` structure. The flag should be at the subcommand level (per-emit-command) since not all subcommands need a spec version (e.g., `describe` is meta).

- [ ] **Step 2: Write failing integration test**

Create `crates/vrm-asset-generator/tests/cli_spec_version.rs`:

```rust
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_vrm-asset-generator")
}

#[test]
fn emit_default_accepts_spec_version_v0() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id", "smoke_v0",
            "--spec-version", "0.x",
            "--output-dir", tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(tmp.path().join("smoke_v0.vrm").exists());
}

#[test]
fn emit_default_accepts_spec_version_v1() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id", "smoke_v1",
            "--spec-version", "1.0",
            "--output-dir", tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(tmp.path().join("smoke_v1.vrm").exists());
}

#[test]
fn emit_default_defaults_to_v1_when_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id", "smoke_default",
            "--output-dir", tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(tmp.path().join("smoke_default.vrm").exists());
}

#[test]
fn emit_default_rejects_invalid_spec_version() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args([
            "emit-default",
            "--id", "smoke_bad",
            "--spec-version", "2.0",
            "--output-dir", tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("run vrm-asset-generator");
    assert!(!out.status.success(), "expected failure for spec_version=2.0");
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `crates/vrm-asset-generator/Cargo.toml` if absent.

- [ ] **Step 3: Run test to verify failure**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version`
Expected: FAIL (flag not recognized).

- [ ] **Step 4: Add the flag to the CLI struct**

Edit `crates/vrm-asset-generator/src/cli.rs`. Add to the relevant subcommand args (e.g., `EmitDefault`, `EmitSweep`, etc.):

```rust
/// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
#[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
pub spec_version: vrm_ops::SpecVersion,
```

Add the parser helper at module-level:

```rust
fn parse_spec_version(s: &str) -> Result<vrm_ops::SpecVersion, String> {
    match s {
        "0.x" => Ok(vrm_ops::SpecVersion::V0),
        "1.0" => Ok(vrm_ops::SpecVersion::V1),
        other => Err(format!("unsupported spec_version {other:?}; expected \"0.x\" or \"1.0\"")),
    }
}
```

- [ ] **Step 5: Thread `spec_version` through `main.rs` to the emit functions**

For Phase B's foundation, every emit function (`emit_default`, etc.) accepts a `spec_version: SpecVersion` parameter. The actual branching to v0-vs-v1 emit lives in the subsequent tasks; for now, document that v0 emit is `unimplemented!()` if the path isn't yet wired:

```rust
// In each emit fn:
match args.spec_version {
    SpecVersion::V1 => existing_v1_emit_path(/*...*/),
    SpecVersion::V0 => v0_emit_path(/*...*/),
}
```

Where `v0_emit_path` initially returns an error or a stub. Subsequent tasks (12–14) populate it.

For the CLI test to pass on `emit-default --spec-version 0.x`, the stub needs to emit at minimum an empty `.vrm` glb with the 0.x extension namespace — even an empty `VRM` extension is acceptable. Task 12 fills in real emit.

For the immediate CLI test, route `emit-default --spec-version 0.x` to a temporary path that calls `vrm_ext_v0::emit_stub` returning an empty-but-valid 0.x asset. The stub is replaced in Task 12.

Create `crates/vrm-asset-generator/src/vrm_ext_v0.rs` with a stub:

```rust
//! VRM 0.x extension emit. Wiring only — shared math lives in mtoon_common.rs.
//! Strictly emit; no parser here (round-tripping is not a v1 goal).

use serde_json::{json, Value};

/// Stub: emits a minimal `VRM` extension block. Real content lands in
/// Tasks 12–14.
pub fn emit_stub_vrm_extension() -> Value {
    json!({
        "exporterVersion": "vrm-asset-generator-0.x-stub",
        "specVersion": "0.0",
        "meta": {
            "title": "stub",
            "version": "1",
            "author": "vrm-asset-generator",
            "licenseName": "CC0",
        },
        "humanoid": { "humanBones": [] },
        "firstPerson": {},
        "blendShapeMaster": { "blendShapeGroups": [] },
        "secondaryAnimation": { "boneGroups": [], "colliderGroups": [] },
        "materialProperties": [],
    })
}
```

Declare the module in `crates/vrm-asset-generator/src/lib.rs`:

```rust
pub mod vrm_ext_v0;
```

In the emit pipeline (e.g., `emit.rs`), branch on `spec_version`: for `V0`, write the `VRM` extension under glTF `extensions`; for `V1`, write `VRMC_vrm` as today. The minimum bar for the Task-11 CLI test is "emit-default with --spec-version 0.x produces a file at the output path."

- [ ] **Step 6: Run test to verify pass**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version`
Expected: 4 tests pass.

- [ ] **Step 7: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/src/main.rs crates/vrm-asset-generator/src/lib.rs crates/vrm-asset-generator/src/vrm_ext_v0.rs crates/vrm-asset-generator/tests/cli_spec_version.rs crates/vrm-asset-generator/Cargo.toml
git commit -m "feat(vrm-asset-generator): --spec-version CLI flag + vrm_ext_v0.rs stub"
```

---

## Task 12: Extract shared MToon math to `mtoon_common.rs`

**Files:**
- Create: `crates/vrm-asset-generator/src/mtoon_common.rs`
- Modify: `crates/vrm-asset-generator/src/mtoon.rs` (delegate to shared math; thin wiring)
- Modify: `crates/vrm-asset-generator/src/lib.rs` (declare new module)
- Test: existing MToon tests must still pass; add a `mtoon_common` unit test

- [ ] **Step 1: Identify the math vs wiring split in current mtoon.rs**

Read `crates/vrm-asset-generator/src/mtoon.rs` (likely ~200–400 lines). The "math" is: PBR-input → MToon-output transformations (e.g., shading-shift sampling, outline-width-mode computation, gi-equalization clamping). The "wiring" is: emit JSON in the 1.0 `VRMC_materials_mtoon` shape (key names, structure).

In `mtoon.rs`, identify pure-function blocks that take MToon-typed params and produce numeric/string outputs without any JSON-shape awareness. Move those to `mtoon_common.rs`. Keep the JSON emit wiring in `mtoon.rs`.

- [ ] **Step 2: Write the shared-math module**

Create `crates/vrm-asset-generator/src/mtoon_common.rs`:

```rust
//! Shared MToon math, independent of 0.x vs 1.0 JSON shape.
//!
//! MToon 0.x (`materialProperties`) and 1.0 (`VRMC_materials_mtoon`) share
//! the same underlying shading math — only the JSON key names and layout
//! differ. This module holds the math; mtoon.rs and mtoon_v0.rs each emit
//! their respective JSON shape over this shared math.

use crate::params::MToonParams;

/// Returns the validated/clamped shading shift factor.
pub fn shading_shift_factor(p: &MToonParams) -> f32 {
    p.shading_shift_factor.clamp(-1.0, 1.0)
}

/// Returns the validated/clamped shading toony factor.
pub fn shading_toony_factor(p: &MToonParams) -> f32 {
    p.shading_toony_factor.clamp(0.0, 1.0)
}

/// Returns the GI equalization factor as a 0–1 ratio.
pub fn gi_equalization_factor(p: &MToonParams) -> f32 {
    p.gi_equalization_factor.clamp(0.0, 1.0)
}

/// Returns the parametric rim color factor as a 3-tuple (R, G, B).
pub fn parametric_rim_color_factor(p: &MToonParams) -> [f32; 3] {
    p.parametric_rim_color_factor
}

/// Returns the rim lighting mix factor as a 0–1 scalar.
pub fn rim_lighting_mix_factor(p: &MToonParams) -> f32 {
    p.rim_lighting_mix_factor.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::MToonParams;

    #[test]
    fn shading_shift_clamps_to_range() {
        let mut p = MToonParams::defaults("test");
        p.shading_shift_factor = 5.0;
        assert_eq!(shading_shift_factor(&p), 1.0);
        p.shading_shift_factor = -5.0;
        assert_eq!(shading_shift_factor(&p), -1.0);
    }

    #[test]
    fn gi_equalization_clamps_to_unit_range() {
        let mut p = MToonParams::defaults("test");
        p.gi_equalization_factor = 1.5;
        assert_eq!(gi_equalization_factor(&p), 1.0);
    }
}
```

(Add more shared-math helpers as you identify them; the above is the starting set. The principle: every numeric transform that's *the same* in 0.x and 1.0 goes here.)

- [ ] **Step 3: Declare the module**

Add to `crates/vrm-asset-generator/src/lib.rs`:

```rust
pub mod mtoon_common;
```

- [ ] **Step 4: Delegate `mtoon.rs` to `mtoon_common.rs`**

In `crates/vrm-asset-generator/src/mtoon.rs`, replace inline math expressions with calls to `mtoon_common::*`. For example:

```rust
// Before:
"shadingShiftFactor": params.shading_shift_factor,

// After:
"shadingShiftFactor": crate::mtoon_common::shading_shift_factor(params),
```

- [ ] **Step 5: Run all MToon tests; ensure pass**

Run: `cargo test -p vrm-asset-generator mtoon`
Expected: all existing MToon tests still pass.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/mtoon_common.rs crates/vrm-asset-generator/src/mtoon.rs crates/vrm-asset-generator/src/lib.rs
git commit -m "refactor(vrm-asset-generator): extract shared MToon math to mtoon_common.rs"
```

---

## Task 13: `mtoon_v0.rs` — emit 0.x `materialProperties`

**Files:**
- Create: `crates/vrm-asset-generator/src/mtoon_v0.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs` (declare module)
- Test: inline `#[cfg(test)]` in `mtoon_v0.rs`

- [ ] **Step 1: Inspect the 0.x MToon schema**

Open `docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.material.schema.json` (or similar location). Confirm the shape:
- `materialProperties` is a top-level array under the `VRM` extension.
- Each entry has `name`, `shader: "VRM/MToon"`, `floatProperties`, `vectorProperties`, `textureProperties`, `keywordMap`, `tagMap`, `renderQueue`.
- Float properties keys: `_Color`, `_ShadeColor`, `_ShadeShift`, `_ShadeToony`, `_OutlineWidth`, `_OutlineColor`, `_OutlineLightingMix` (not present in 0.x — Unity-shader artifact), etc.
- Key naming is **Unity shader convention** (leading underscore), not glTF JSON convention.

- [ ] **Step 2: Write failing test**

Append to `crates/vrm-asset-generator/src/mtoon_v0.rs` (create file with test first):

```rust
//! VRM 0.x MToon emit. Produces `materialProperties[]` entries in the
//! Unity-shader-style key-value shape. Shared math lives in mtoon_common.rs.
//!
//! Schema reference: `docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.material.schema.json`.

use crate::params::MToonParams;
use serde_json::{json, Value};

/// Emit one entry of the 0.x `materialProperties` array for the given
/// MToon parameter set. Caller assembles the surrounding array.
pub fn emit_material_property(params: &MToonParams) -> Value {
    let shading_shift = crate::mtoon_common::shading_shift_factor(params);
    let shading_toony = crate::mtoon_common::shading_toony_factor(params);
    let [rim_r, rim_g, rim_b] = crate::mtoon_common::parametric_rim_color_factor(params);

    json!({
        "name": params.id,
        "renderQueue": 2000 + params.render_queue_offset_number,
        "shader": "VRM/MToon",
        "floatProperties": {
            "_ShadeShift": shading_shift,
            "_ShadeToony": shading_toony,
            "_OutlineWidth": params.outline_width_factor,
            "_OutlineWidthMode": match params.outline_width_mode {
                crate::params::OutlineWidthMode::None => 0,
                crate::params::OutlineWidthMode::WorldCoordinates => 1,
                crate::params::OutlineWidthMode::ScreenCoordinates => 2,
            },
            // Note: 0.x MToon does NOT have _OutlineLightingMix —
            // outline_lighting_mix_factor is v1-only. Sweep variants that
            // exercise it are NotApplicable on the 0.x side.
        },
        "vectorProperties": {
            "_Color": [1.0, 1.0, 1.0, 1.0],
            "_ShadeColor": [0.7, 0.7, 0.7, 1.0],
            "_OutlineColor": params.outline_color_factor.iter().copied().chain([1.0]).collect::<Vec<_>>(),
            "_RimColor": [rim_r, rim_g, rim_b, 1.0],
        },
        "textureProperties": {},
        "keywordMap": {},
        "tagMap": {},
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::MToonParams;

    #[test]
    fn emits_unity_shader_key_naming() {
        let p = MToonParams::defaults("test_v0");
        let v = emit_material_property(&p);
        assert_eq!(v["shader"], "VRM/MToon");
        // Unity-shader keys MUST have leading underscores.
        let float_props = v["floatProperties"].as_object().unwrap();
        for k in float_props.keys() {
            assert!(k.starts_with('_'), "key {k} missing Unity-shader leading underscore");
        }
    }

    #[test]
    fn does_not_emit_outline_lighting_mix_v1_only_field() {
        let p = MToonParams::defaults("test_v0");
        let v = emit_material_property(&p);
        // 0.x has no outline lighting mix; sweep variants exercising it are NotApplicable.
        let float_props = v["floatProperties"].as_object().unwrap();
        assert!(!float_props.contains_key("_OutlineLightingMix"));
    }

    #[test]
    fn outline_width_mode_maps_to_int_enum() {
        let mut p = MToonParams::defaults("test_v0");
        p.outline_width_mode = crate::params::OutlineWidthMode::WorldCoordinates;
        let v = emit_material_property(&p);
        assert_eq!(v["floatProperties"]["_OutlineWidthMode"], 1);
        p.outline_width_mode = crate::params::OutlineWidthMode::ScreenCoordinates;
        let v = emit_material_property(&p);
        assert_eq!(v["floatProperties"]["_OutlineWidthMode"], 2);
    }
}
```

- [ ] **Step 3: Declare the module**

Add to `crates/vrm-asset-generator/src/lib.rs`:

```rust
pub mod mtoon_v0;
```

- [ ] **Step 4: Run tests; iterate until pass**

Run: `cargo test -p vrm-asset-generator mtoon_v0`
Expected: 3 tests pass after fixing any compile errors.

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/mtoon_v0.rs crates/vrm-asset-generator/src/lib.rs
git commit -m "feat(vrm-asset-generator): mtoon_v0.rs — emit 0.x materialProperties (Unity-shader keys)"
```

---

## Task 14: `expressions_v0.rs` — emit `blendShapeMaster`

**Files:**
- Create: `crates/vrm-asset-generator/src/expressions_v0.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs` (declare module)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Inspect 0.x expression schema**

`docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.blendshape.schema.json`. Expected shape:

```jsonc
"blendShapeMaster": {
  "blendShapeGroups": [
    {
      "name": "joy",
      "presetName": "joy",         // canonical preset names: neutral, joy, angry, sorrow, fun, a, i, u, e, o, blink, blink_l, blink_r, lookup, lookdown, lookleft, lookright
      "binds": [
        { "mesh": <meshIndex>, "index": <morphTargetIndex>, "weight": 100.0 }   // 0–100 range in 0.x (not 0–1!)
      ],
      "materialValues": [],
      "isBinary": false
    }
  ]
}
```

- [ ] **Step 2: Write failing test**

Create `crates/vrm-asset-generator/src/expressions_v0.rs`:

```rust
//! VRM 0.x expressions emit. Produces `blendShapeMaster.blendShapeGroups[]`
//! entries with preset names (joy, neutral, sorrow, etc.) and per-mesh
//! morph-target bindings.
//!
//! Note: 0.x weight range is 0–100 (Unity convention), NOT 0–1 like 1.0.
//! Schema: `docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.blendshape.schema.json`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// 0.x preset names. Canonical per spec §VRM/BlendShape.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlendShapePreset {
    Neutral,
    Joy,
    Angry,
    Sorrow,
    Fun,
    A,
    I,
    U,
    E,
    O,
    Blink,
    BlinkL,
    BlinkR,
    Lookup,
    Lookdown,
    Lookleft,
    Lookright,
    Unknown, // For custom presets.
}

impl BlendShapePreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Joy => "joy",
            Self::Angry => "angry",
            Self::Sorrow => "sorrow",
            Self::Fun => "fun",
            Self::A => "a",
            Self::I => "i",
            Self::U => "u",
            Self::E => "e",
            Self::O => "o",
            Self::Blink => "blink",
            Self::BlinkL => "blink_l",
            Self::BlinkR => "blink_r",
            Self::Lookup => "lookup",
            Self::Lookdown => "lookdown",
            Self::Lookleft => "lookleft",
            Self::Lookright => "lookright",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpressionsV0Params {
    pub groups: Vec<BlendShapeGroup>,
}

#[derive(Clone, Debug)]
pub struct BlendShapeGroup {
    pub name: String,
    pub preset: BlendShapePreset,
    pub binds: Vec<BlendShapeBind>,
}

#[derive(Clone, Debug)]
pub struct BlendShapeBind {
    pub mesh_index: u32,
    pub morph_target_index: u32,
    pub weight_0_to_100: f32,
}

pub fn emit_blend_shape_master(params: &ExpressionsV0Params) -> Value {
    let groups: Vec<Value> = params.groups.iter().map(|g| {
        json!({
            "name": g.name,
            "presetName": g.preset.as_str(),
            "binds": g.binds.iter().map(|b| json!({
                "mesh": b.mesh_index,
                "index": b.morph_target_index,
                "weight": b.weight_0_to_100,
            })).collect::<Vec<_>>(),
            "materialValues": [],
            "isBinary": false,
        })
    }).collect();

    json!({ "blendShapeGroups": groups })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names_match_spec_canonical() {
        // Per spec, presets are lowercase tokens.
        assert_eq!(BlendShapePreset::Joy.as_str(), "joy");
        assert_eq!(BlendShapePreset::Neutral.as_str(), "neutral");
        assert_eq!(BlendShapePreset::BlinkL.as_str(), "blink_l");
        assert_eq!(BlendShapePreset::Lookup.as_str(), "lookup");
    }

    #[test]
    fn emits_blend_shape_master_with_joy_group() {
        let params = ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "Joy".into(),
                preset: BlendShapePreset::Joy,
                binds: vec![BlendShapeBind {
                    mesh_index: 0,
                    morph_target_index: 3,
                    weight_0_to_100: 100.0,
                }],
            }],
        };
        let v = emit_blend_shape_master(&params);
        assert_eq!(v["blendShapeGroups"][0]["presetName"], "joy");
        assert_eq!(v["blendShapeGroups"][0]["binds"][0]["mesh"], 0);
        assert_eq!(v["blendShapeGroups"][0]["binds"][0]["weight"], 100.0);
    }

    #[test]
    fn weight_uses_0_100_range_not_0_1() {
        // Methodology pin: 0.x weights are Unity-convention 0–100, not 0–1.
        let params = ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "Half".into(),
                preset: BlendShapePreset::Joy,
                binds: vec![BlendShapeBind {
                    mesh_index: 0,
                    morph_target_index: 0,
                    weight_0_to_100: 50.0,
                }],
            }],
        };
        let v = emit_blend_shape_master(&params);
        assert_eq!(v["blendShapeGroups"][0]["binds"][0]["weight"], 50.0);
    }
}
```

- [ ] **Step 3: Declare module**

Add to `crates/vrm-asset-generator/src/lib.rs`:

```rust
pub mod expressions_v0;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vrm-asset-generator expressions_v0`
Expected: 3 tests pass.

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/expressions_v0.rs crates/vrm-asset-generator/src/lib.rs
git commit -m "feat(vrm-asset-generator): expressions_v0.rs — emit 0.x blendShapeMaster (0–100 weight range)"
```

---

## Task 15: Wire `emit-default --spec-version 0.x` to produce a full v0 asset

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (branch on spec_version)
- Modify: `crates/vrm-asset-generator/src/vrm_ext_v0.rs` (replace stub with full assembly)

- [ ] **Step 1: Replace the stub in vrm_ext_v0.rs**

Edit `crates/vrm-asset-generator/src/vrm_ext_v0.rs`:

```rust
//! VRM 0.x extension emit. Wiring only — shared math via mtoon_common.rs.
//! Strictly emit; no parser here.

use serde_json::{json, Value};
use crate::params::MToonParams;
use crate::expressions_v0::ExpressionsV0Params;

/// Build the full `VRM` extension block for a 0.x asset.
pub fn emit_vrm_extension(
    title: &str,
    materials: &[MToonParams],
    expressions: &ExpressionsV0Params,
) -> Value {
    let material_props: Vec<Value> = materials.iter()
        .map(crate::mtoon_v0::emit_material_property)
        .collect();

    json!({
        "exporterVersion": "vrm-asset-generator/0.x",
        "specVersion": "0.0",
        "meta": {
            "title": title,
            "version": "1",
            "author": "vrm-asset-generator",
            "contactInformation": "",
            "reference": "",
            "texture": -1,
            "allowedUserName": "OnlyAuthor",
            "violentUssageName": "Disallow",
            "sexualUssageName": "Disallow",
            "commercialUssageName": "Disallow",
            "otherPermissionUrl": "",
            "licenseName": "CC0",
            "otherLicenseUrl": ""
        },
        "humanoid": {
            "humanBones": [],   // populated by humanoid emit; out of slice 1 scope
            "armStretch": 0.05,
            "legStretch": 0.05,
            "upperArmTwist": 0.5,
            "lowerArmTwist": 0.5,
            "upperLegTwist": 0.5,
            "lowerLegTwist": 0.5,
            "feetSpacing": 0.0,
            "hasTranslationDoF": false
        },
        "firstPerson": {
            "firstPersonBone": -1,
            "firstPersonBoneOffset": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "meshAnnotations": [],
            "lookAtTypeName": "Bone",
            "lookAtHorizontalInner": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 },
            "lookAtHorizontalOuter": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 },
            "lookAtVerticalDown": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 },
            "lookAtVerticalUp": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 }
        },
        "blendShapeMaster": crate::expressions_v0::emit_blend_shape_master(expressions),
        "secondaryAnimation": {
            "boneGroups": [],
            "colliderGroups": []
        },
        "materialProperties": material_props
    })
}
```

- [ ] **Step 2: Branch the emit pipeline on `spec_version`**

In `crates/vrm-asset-generator/src/emit.rs` (or wherever `emit_default` glb assembly happens), add:

```rust
// After building the glTF JSON skeleton:
let extension = match spec_version {
    SpecVersion::V0 => {
        let materials: Vec<_> = vec![/* per-asset MToonParams set */];
        let expressions = ExpressionsV0Params { groups: vec![] };
        let mut exts = serde_json::Map::new();
        exts.insert("VRM".to_string(), vrm_ext_v0::emit_vrm_extension(asset_id, &materials, &expressions));
        json!(exts)
    }
    SpecVersion::V1 => {
        // ... existing VRMC_vrm emit path ...
        existing_v1_extensions
    }
};
gltf_json["extensions"] = extension;

// Also: extensionsUsed array must list the correct extension namespace.
let extensions_used = match spec_version {
    SpecVersion::V0 => vec!["VRM"],
    SpecVersion::V1 => vec!["VRMC_vrm", "VRMC_materials_mtoon", "VRMC_springBone", "KHR_materials_unlit"],
};
gltf_json["extensionsUsed"] = json!(extensions_used);
```

- [ ] **Step 3: Verify emit-default --spec-version 0.x produces a parseable .vrm**

```bash
cargo run -p vrm-asset-generator -- emit-default --id smoke_v0 --spec-version 0.x --output-dir /tmp/v0
ls /tmp/v0/
# Expected: smoke_v0.vrm, smoke_v0.meta.json, smoke_v0.test.yaml
```

- [ ] **Step 4: Verify the emitted .vrm has the VRM extension and 0.0 specVersion**

```bash
# Extract the JSON chunk and check the extension shape:
cargo run -p vrm-asset-generator -- describe --format json | head -50
# Or use a quick python:
python3 -c "
import struct, json, sys
with open('/tmp/v0/smoke_v0.vrm', 'rb') as f:
    f.read(12)  # glb header
    length, chunk_type = struct.unpack('<II', f.read(8))
    j = json.loads(f.read(length))
print(json.dumps(j.get('extensions', {}).get('VRM', {}).get('specVersion'), indent=2))
print('extensionsUsed:', j.get('extensionsUsed'))
"
```

Expected: `"0.0"` and `["VRM"]`.

- [ ] **Step 5: Verify validator (if 0.x supported per Task 10 finding)**

```bash
.tools/vrm-validator-cli validate /tmp/v0/smoke_v0.vrm 2>&1
```

If validator rejects per Task 10 finding, this is the documented exemption path; skip.

- [ ] **Step 6: Run CLI integration tests**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version`
Expected: 4 tests pass (the stub is now a full emit).

- [ ] **Step 7: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/vrm-asset-generator/src/vrm_ext_v0.rs crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(vrm-asset-generator): wire emit-default --spec-version 0.x to produce full v0 .vrm"
```

---

## Task 16: `mtoon_basic_v0_sweep` — 3 variants including one `NotApplicable`

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs` (add the v0 sweep function)
- Modify: `crates/vrm-asset-generator/src/cli.rs` (add `EmitMtoonBasicV0Sweep` subcommand if not auto-derived from existing sweep dispatch)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Define the sweep**

Append to `crates/vrm-asset-generator/src/sweep.rs`:

```rust
use crate::{NotApplicableReason, SweepApplicability};

/// VRM 0.x MToon basic sweep — slice 1 of the conformance corpus.
///
/// 3 variants:
/// - `mtoon_basic_v0_lit_001` — neutral lit baseline; Applicable.
/// - `mtoon_basic_v0_shadeShift_neg05` — shadingShift -0.5; Applicable.
/// - `mtoon_basic_v0_outline_lighting_mix` — registered as
///   NotApplicable { reason: OutlineLightingMixV1Only } because 0.x has no
///   `_OutlineLightingMix` Unity-shader key (v1-only axis).
///
/// Each Applicable variant has a counterpart in `mtoon_basic_sweep` (1.0).
pub fn mtoon_basic_v0_sweep() -> Vec<(MToonParams, SweepApplicability)> {
    let mut out = Vec::new();

    let lit = MToonParams::defaults("mtoon_basic_v0_lit_001");
    out.push((lit, SweepApplicability::Applicable));

    let mut shade_shift = MToonParams::defaults("mtoon_basic_v0_shadeShift_neg05");
    shade_shift.shading_shift_factor = -0.5;
    out.push((shade_shift, SweepApplicability::Applicable));

    let mut lighting_mix = MToonParams::defaults("mtoon_basic_v0_outline_lighting_mix");
    lighting_mix.outline_lighting_mix_factor = 0.5; // Only matters in 1.0.
    out.push((lighting_mix, SweepApplicability::NotApplicable {
        reason: NotApplicableReason::OutlineLightingMixV1Only,
    }));

    out
}

#[cfg(test)]
mod sweep_v0_tests {
    use super::*;

    #[test]
    fn mtoon_basic_v0_sweep_has_three_variants() {
        let sweep = mtoon_basic_v0_sweep();
        assert_eq!(sweep.len(), 3);
    }

    #[test]
    fn mtoon_basic_v0_sweep_has_one_not_applicable() {
        let sweep = mtoon_basic_v0_sweep();
        let na_count = sweep.iter().filter(|(_, app)| matches!(app, SweepApplicability::NotApplicable { .. })).count();
        assert_eq!(na_count, 1);
    }

    #[test]
    fn not_applicable_reason_is_outline_lighting_mix() {
        let sweep = mtoon_basic_v0_sweep();
        let na = sweep.iter().find_map(|(_, app)| match app {
            SweepApplicability::NotApplicable { reason } => Some(*reason),
            _ => None,
        }).unwrap();
        assert_eq!(na, NotApplicableReason::OutlineLightingMixV1Only);
    }
}
```

- [ ] **Step 2: Add the emit-mtoon-basic-v0-sweep subcommand**

In `cli.rs`, if subcommand dispatch is centralized, add:

```rust
#[derive(Args)]
pub struct EmitMtoonBasicV0Sweep {
    #[arg(long)]
    pub output_dir: PathBuf,
}
```

In `main.rs` dispatch, route to:

```rust
Command::EmitMtoonBasicV0Sweep(args) => {
    let sweep = vrm_asset_generator::sweep::mtoon_basic_v0_sweep();
    for (params, applicability) in sweep {
        match applicability {
            SweepApplicability::Applicable => {
                let out_path = args.output_dir.join(format!("{}.vrm", params.id));
                emit::emit_v0_asset(&params, &out_path)?;
                emit::emit_v0_meta(&params, &args.output_dir)?;
                emit::emit_v0_test_plan(&params, &args.output_dir)?;
            }
            SweepApplicability::NotApplicable { reason } => {
                emit::emit_not_applicable_marker(&params, reason, &args.output_dir)?;
            }
        }
    }
    Ok(())
}
```

The `emit_not_applicable_marker` writes a `<id>.skipped.json` with `{ "kind": "NotApplicable", "reason": "OutlineLightingMixV1Only" }`. The runner / site treat this as "skipped, structured reason" rather than "missing asset."

- [ ] **Step 3: Run tests**

Run: `cargo test -p vrm-asset-generator mtoon_basic_v0`
Expected: 3 tests pass.

- [ ] **Step 4: Smoke the sweep emit**

```bash
cargo run -p vrm-asset-generator -- emit-mtoon-basic-v0-sweep --output-dir /tmp/v0-sweep
ls /tmp/v0-sweep/
# Expected:
# mtoon_basic_v0_lit_001.vrm + .meta.json + .test.yaml
# mtoon_basic_v0_shadeShift_neg05.vrm + .meta.json + .test.yaml
# mtoon_basic_v0_outline_lighting_mix.skipped.json
```

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/sweep.rs crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/src/main.rs crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(vrm-asset-generator): mtoon_basic_v0_sweep — 3 variants, one NotApplicable (OutlineLightingMixV1Only)"
```

---

## Task 17: `expressions_preset_basic` sweep — v0+v1 pair (2 variants each)

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs` (add the paired sweep function)
- Modify: `crates/vrm-asset-generator/src/cli.rs` + `main.rs` (subcommand dispatch)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Define the paired sweep**

Append to `crates/vrm-asset-generator/src/sweep.rs`:

```rust
use crate::expressions_v0::{BlendShapeBind, BlendShapeGroup, BlendShapePreset, ExpressionsV0Params};

/// Expression preset basic — canonical normalization test pair.
///
/// 2 variants per version (4 assets total):
/// - `expressions_preset_basic_v0_joy` — v0, joy preset @ weight 100.
/// - `expressions_preset_basic_v0_neutral` — v0, neutral preset @ weight 100.
/// - `expressions_preset_basic_happy` — v1, happy preset @ weight 1.0.
/// - `expressions_preset_basic_neutral` — v1, neutral preset @ weight 1.0.
///
/// Smallest test surface that validates: vrm-normalize crate, source_spec_version
/// echo, as_spec_version param, round-trip property (dump(V1) from adapter A ≡
/// from adapter B on losslessly-equivalent shapes).
pub fn expressions_preset_basic_v0_sweep() -> Vec<ExpressionsV0Params> {
    vec![
        ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "expressions_preset_basic_v0_joy".into(),
                preset: BlendShapePreset::Joy,
                binds: vec![BlendShapeBind {
                    mesh_index: 0,
                    morph_target_index: 0,
                    weight_0_to_100: 100.0,
                }],
            }],
        },
        ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "expressions_preset_basic_v0_neutral".into(),
                preset: BlendShapePreset::Neutral,
                binds: vec![BlendShapeBind {
                    mesh_index: 0,
                    morph_target_index: 0,
                    weight_0_to_100: 100.0,
                }],
            }],
        },
    ]
}

#[cfg(test)]
mod expressions_basic_tests {
    use super::*;
    #[test]
    fn paired_v0_v1_presets_have_consistent_naming() {
        let v0_sweep = expressions_preset_basic_v0_sweep();
        assert_eq!(v0_sweep.len(), 2);
        let names: Vec<_> = v0_sweep.iter().map(|p| &p.groups[0].name).collect();
        assert!(names.iter().any(|n| n.contains("joy")));
        assert!(names.iter().any(|n| n.contains("neutral")));
    }
}
```

For the v1 side, the existing expression sweep emit path (or a new `expressions_preset_basic_v1_sweep`) emits `expressions_preset_basic_happy` and `expressions_preset_basic_neutral` using the `VRMC_vrm.expressions.preset` shape.

- [ ] **Step 2: Add a `EmitExpressionsPresetBasic` subcommand emitting both versions**

In `cli.rs`:

```rust
#[derive(Args)]
pub struct EmitExpressionsPresetBasic {
    #[arg(long)]
    pub output_dir: PathBuf,
}
```

In `main.rs` dispatch:

```rust
Command::EmitExpressionsPresetBasic(args) => {
    // Emit v0 side (joy, neutral).
    for params in sweep::expressions_preset_basic_v0_sweep() {
        emit_v0_with_expressions(&args.output_dir, &params)?;
    }
    // Emit v1 side (happy, neutral).
    for params in sweep::expressions_preset_basic_v1_sweep() {
        emit_v1_with_expressions(&args.output_dir, &params)?;
    }
    Ok(())
}
```

`emit_v0_with_expressions` constructs the .vrm with the given `ExpressionsV0Params` plugged into the `VRM.blendShapeMaster` slot; `emit_v1_with_expressions` does likewise for `VRMC_vrm.expressions`. Both also emit paired `.meta.json` and `.test.yaml`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p vrm-asset-generator expressions_basic`
Expected: pass.

- [ ] **Step 4: Smoke**

```bash
cargo run -p vrm-asset-generator -- emit-expressions-preset-basic --output-dir /tmp/exp-pair
ls /tmp/exp-pair/
# Expected: 4 .vrm files + 4 .meta.json + 4 .test.yaml
```

- [ ] **Step 5: Verify test plans carry `spec_version`**

```bash
grep -l 'spec_version: "0.x"' /tmp/exp-pair/*.test.yaml
grep -l 'spec_version: "1.0"' /tmp/exp-pair/*.test.yaml
# Each list should have 2 files.
```

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/sweep.rs crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/src/main.rs crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(vrm-asset-generator): expressions_preset_basic v0+v1 pair (joy/neutral × happy/neutral)"
```

---

## Task 18: Sweep registry symmetry assertion (compile-time invariant)

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs` (add the assertion test)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the symmetry assertion**

Append to `crates/vrm-asset-generator/src/sweep.rs`:

```rust
#[cfg(test)]
mod registry_symmetry_tests {
    use super::*;

    /// Every sweep ID that ends in `_v0` (or contains `_v0_`) must have a
    /// 1.0 counterpart registered. Counterpart = same ID with `_v0` removed
    /// (or `_v0_` replaced with `_`). Either the counterpart exists as
    /// Applicable, OR the v0 entry is registered as NotApplicable.
    #[test]
    fn sweep_registry_symmetric_across_versions() {
        let v1_ids: std::collections::HashSet<String> = mtoon_basic_sweep()
            .into_iter()
            .map(|p| p.id.clone())
            .collect();
        let v0_entries = mtoon_basic_v0_sweep();

        for (params, applicability) in v0_entries {
            let v0_id = &params.id;
            let counterpart_id = v0_id
                .replace("_basic_v0_", "_basic_")
                .replace("_v0_", "_");
            let counterpart_exists = v1_ids.contains(&counterpart_id);

            match applicability {
                SweepApplicability::Applicable => {
                    assert!(
                        counterpart_exists,
                        "v0 sweep entry {v0_id} (Applicable) is missing 1.0 counterpart {counterpart_id}"
                    );
                }
                SweepApplicability::NotApplicable { .. } => {
                    // No 1.0 counterpart required when the v0 entry is explicitly NotApplicable —
                    // the reason variant documents why.
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p vrm-asset-generator sweep_registry_symmetric`
Expected: pass. If it fails, either (a) a v0 Applicable entry doesn't have a 1.0 counterpart (add one), or (b) the renaming logic doesn't match the naming conventions (fix the renaming or the conventions).

- [ ] **Step 3: Verify it would fail on accidental drift**

Temporarily rename one of the 1.0 entries (e.g., `mtoon_default` → `mtoon_default_oops`); re-run the test; confirm it fails with a clear error. Revert.

- [ ] **Step 4: Add the test to CI**

This test runs as part of `cargo test -p vrm-asset-generator --lib`, which CI already executes. No separate workflow needed.

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/sweep.rs
git commit -m "test(vrm-asset-generator): sweep registry symmetry assertion across spec versions"
```

---

## Task 19: Tier 2 canonical fixture install — `vroid_default_F_0_0` (gated on Task 8 finding)

**Files:**
- Modify: `scripts/install-humanoid-fixtures.sh` (add 0.x branch)
- Modify: `assets/humanoid/` (add `.gitignore` for the binary if S3-pulled; or symlink from cache)

This task is gated on Task 8's empirical-check outcome. Two paths:

### Path AVAILABLE — Studio re-export landed

- [ ] **Step 1: Re-export VRoid default character through Studio 2.12.0's 0.x path**

Manual: in VRoid Studio, open the default character, File → Export → VRM 0.x. Save as `vroid_default_F_0_0.vrm`. Upload to the project's S3 fixture bucket (whatever `scripts/install-humanoid-fixtures.sh` currently uses).

- [ ] **Step 2: Update install script**

Edit `scripts/install-humanoid-fixtures.sh` to fetch the new fixture:

```bash
# Append to the existing fixture-fetch list:
fetch_fixture "vroid_default_F_0_0.vrm" "https://<s3-bucket>/fixtures/vroid_default_F_0_0.vrm"
fetch_fixture "vroid_default_F_0_0.meta.json" "https://<s3-bucket>/fixtures/vroid_default_F_0_0.meta.json"
```

- [ ] **Step 3: Run install**

```bash
scripts/install-humanoid-fixtures.sh
ls assets/humanoid/vroid_default_F_0_0.vrm
```

Expected: file present, BLAKE3 matches the meta.

- [ ] **Step 4: Author the test plan**

Create `test-plans/manual/humanoid/vroid_default_F_0_0.test.yaml`:

```yaml
id: vroid_default_F_0_0_lit_baseline
spec_version: "0.x"
spec_section: "VRM 0.x — Tier 2 canonical content"
asset: assets/humanoid/vroid_default_F_0_0.vrm
camera:
  # VRM 0.x camera convention: avatar faces -Z, camera at -Z target origin.
  position: [0.0, 1.3, -1.5]
  target: [0.0, 1.3, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -0.5, -1.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.2
post_processing:
  tone_mapping: none
  exposure: 1.0
output:
  width: 1024
  height: 1024
diff:
  ssim_threshold: 0.95
```

- [ ] **Step 5: Commit**

```bash
git add scripts/install-humanoid-fixtures.sh test-plans/manual/humanoid/vroid_default_F_0_0.test.yaml assets/humanoid/vroid_default_F_0_0.meta.json
git commit -m "feat(corpus): Tier 2 canonical VRoid default exported as 0.x"
```

### Path REMOVED — fall back to avatarA_0_0 only

- [ ] **Step 1: Document the fallback decision**

Add a note to `docs/findings.md` (extending the Task 8 entry):

```markdown
**Slice 1 Tier 2 sourcing decision.** With VRoid Studio 0.x export removed, slice 1 ships Tier 2 canonical content via the existing `avatarA_0_0.vrm` fixture alone. The `vroid_default_F_0_0` slot reopens when an alternate source materializes (older Studio installer, Hub-sourced content).
```

- [ ] **Step 2: Commit**

```bash
git add docs/findings.md
git commit -m "docs(findings): Tier 2 slice-1 fallback — avatarA_0_0 alone (Studio 0.x export REMOVED)"
```

Proceed to Task 20.

---

## Task 20: `avatarA_0_0.vrm` test plan

**Files:**
- Create: `test-plans/manual/humanoid/avatarA_0_0.test.yaml`

- [ ] **Step 1: Write the plan**

Create `test-plans/manual/humanoid/avatarA_0_0.test.yaml`:

```yaml
id: avatarA_0_0_lit_baseline
spec_version: "0.x"
spec_section: "VRM 0.x — humanoid baseline"
asset: assets/humanoid/avatarA_0_0.vrm
camera:
  # VRM 0.x camera convention: avatar faces -Z per
  # docs/upstream-specs/vrm-specification/specification/0.0/README.md:238.
  # Camera placed at -Z so it sees the front of a spec-conformant render.
  position: [0.0, 1.3, -1.5]
  target: [0.0, 1.3, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -0.5, -1.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [1.0, 1.0, 1.0]
    intensity: 0.2
post_processing:
  tone_mapping: none   # Methodology pin — see docs/methodology.md
  exposure: 1.0
output:
  width: 1024
  height: 1024
diff:
  ssim_threshold: 0.92    # Slightly relaxed for 0.x corpus (first run; tighten after consensus baselines)
```

- [ ] **Step 2: Verify plan parses through vrm-test-plan**

Write a small smoke test (one-off, can be deleted after):

```bash
cargo run -p vrm-runner -- describe --format json | grep -q "execute-test-plan"
# Verify the runner is buildable; the plan parses when the runner loads it later.
```

Better: run the runner against this plan in dry-run mode (if such a flag exists; otherwise wait for Phase C).

- [ ] **Step 3: Commit**

```bash
git add test-plans/manual/humanoid/avatarA_0_0.test.yaml
git commit -m "feat(corpus): avatarA_0_0 paired test plan — first 0.x corpus entry"
```

**End of Phase B.** Mid-slice checkpoint approaches. Phase C wires adapters.

---

# Phase C — Adapter wiring + mid-slice checkpoint (days 10–17)

Gate at mid-slice (day 10): three-vrm + VMK produce renders for the slice-1 assets; first two-adapter diff produced; VMK 180° flip surfaces as a clear visual signature in the published output.

Gate at end of Phase C (day 17): all four adapters render the slice-1 assets; `source_spec_version` echoed on every dump response; runner cross-checks plan ↔ manifest ↔ adapter `source_spec_version` as three hard-error gates.

## Task 21: `source_spec_version` field on dump op result types

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs` (add field to DumpExpressionWeightsResult, DumpHumanoidPoseResult, DumpLookAtStateResult)
- Test: append inline `#[cfg(test)]` blocks for serde round-trip on each result type

- [ ] **Step 1: Locate the dump result types**

In `crates/vrm-ops/src/tools.rs`, find `DumpExpressionWeightsResult`, `DumpHumanoidPoseResult`, `DumpLookAtStateResult` (or whatever the equivalents are).

- [ ] **Step 2: Write failing tests**

Append:

```rust
#[cfg(test)]
mod source_spec_version_tests {
    use super::*;
    use crate::SpecVersion;

    #[test]
    fn dump_expression_weights_result_carries_source_spec_version() {
        let r = DumpExpressionWeightsResult {
            source_spec_version: SpecVersion::V0,
            weights: Default::default(),
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["source_spec_version"], "0.x");
    }

    #[test]
    fn dump_humanoid_pose_result_carries_source_spec_version() {
        let r = DumpHumanoidPoseResult {
            source_spec_version: SpecVersion::V1,
            bones: Default::default(),
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["source_spec_version"], "1.0");
    }

    #[test]
    fn dump_look_at_state_result_carries_source_spec_version() {
        let r = DumpLookAtStateResult {
            source_spec_version: SpecVersion::V0,
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["source_spec_version"], "0.x");
    }
}
```

(Adapt field names — `bones`, `weights`, etc. — to match the actual existing struct shapes.)

- [ ] **Step 3: Add the field to each result type**

For each:

```rust
pub struct DumpExpressionWeightsResult {
    pub source_spec_version: SpecVersion,
    // ... existing fields ...
}
```

Repeat for the other two.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vrm-ops source_spec_version`
Expected: 3 tests pass.

- [ ] **Step 5: Update operation-contract.md**

Append to `docs/operation-contract.md` under the `dump_*` op sections:

```markdown
### Response field: `source_spec_version`

Every dump operation's response carries a required `source_spec_version: "0.x" | "1.0"` field, echoing what the adapter parsed from the loaded asset. Cross-checks at runner level: must match the test plan's declared `spec_version` and the manifest entry's `spec_version`. Mismatches trigger hard-error `-32002 SpecVersionMismatch`.
```

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p vrm-ops --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-ops/src/tools.rs docs/operation-contract.md
git commit -m "feat(vrm-ops): source_spec_version response field on dump ops"
```

---

## Task 22: `as_spec_version` param on dump ops

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs` (add optional field to DumpExpressionWeightsParams, etc.)
- Test: inline serde tests

- [ ] **Step 1: Write failing tests**

Append:

```rust
#[cfg(test)]
mod as_spec_version_tests {
    use super::*;
    use crate::SpecVersion;

    #[test]
    fn dump_expression_weights_params_as_spec_version_omitted_when_none() {
        let p = DumpExpressionWeightsParams {
            session_id: "s".into(),
            as_spec_version: None,
        };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert!(v.get("as_spec_version").is_none(), "got {v}");
    }

    #[test]
    fn dump_expression_weights_params_as_spec_version_v1_serializes() {
        let p = DumpExpressionWeightsParams {
            session_id: "s".into(),
            as_spec_version: Some(SpecVersion::V1),
        };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["as_spec_version"], "1.0");
    }
}
```

- [ ] **Step 2: Add field**

For each dump params struct:

```rust
pub struct DumpExpressionWeightsParams {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_spec_version: Option<SpecVersion>,
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vrm-ops as_spec_version`
Expected: pass.

- [ ] **Step 4: Update operation-contract.md**

Append:

```markdown
### Request param: `as_spec_version`

Optional on `dump_humanoid_pose`, `dump_expression_weights`, `dump_look_at_state`. Wire form `"0.x" | "1.0"`.

- **Absent (default)**: adapter returns the dump in its **native** spec-version shape — never normalize unless asked.
- **`"1.0"` against a 0.x asset**: runner normalizes via `vrm-normalize` (joy→happy preset mapping, etc.).
- **`"0.x"` against a 1.0 asset**: rejected with error `-32001 NormalizationDirectionUnsupported`. v1→v0 has no lossless mapping.
- Custom blendshapes without a v1 preset equivalent pass through with `custom:<name>` markers, never dropped.
```

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p vrm-ops --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-ops/src/tools.rs docs/operation-contract.md
git commit -m "feat(vrm-ops): as_spec_version param on dump ops (V0 / V1; default = native)"
```

---

## Task 23: three-vrm adapter — populate `source_spec_version` on dumps

**Files:**
- Modify: `adapters/three-vrm/src/operations.ts` (every dump handler)
- Modify: `adapters/three-vrm/src/loader.ts` (or wherever load determines version)
- Test: `adapters/three-vrm/test/source_spec_version.test.ts` (new)

- [ ] **Step 1: Detect spec version at load time**

In the load handler, after parsing the glTF JSON, inspect `extensionsUsed`:

```typescript
function detectSpecVersion(gltfJson: any): "0.x" | "1.0" {
    const used: string[] = gltfJson.extensionsUsed ?? [];
    if (used.includes("VRMC_vrm")) return "1.0";
    if (used.includes("VRM")) return "0.x";
    throw new Error("No VRM extension found in extensionsUsed");
}
```

Store the detected version in the session state:

```typescript
session.sourceSpecVersion = detectSpecVersion(gltfJson);
```

- [ ] **Step 2: Echo it on every dump response**

In each `dump_*` handler:

```typescript
function handleDumpExpressionWeights(params: any, session: Session) {
    const weights = readNativeExpressionWeights(session);
    return {
        source_spec_version: session.sourceSpecVersion,
        weights,
    };
}
```

Repeat for `dump_humanoid_pose` and `dump_look_at_state`.

- [ ] **Step 3: Write failing test**

Create `adapters/three-vrm/test/source_spec_version.test.ts`:

```typescript
import { test, expect } from "@playwright/test";
import { rpcCall, startAdapter } from "./harness";

test("three-vrm reports source_spec_version: 0.x for VRM 0.x asset", async () => {
    const proc = await startAdapter();
    try {
        const load = await rpcCall(proc, "load_vrm", {
            path: "../../assets/humanoid/avatarA_0_0.vrm",
        });
        const session_id = load.session_id;
        const dump = await rpcCall(proc, "dump_expression_weights", { session_id });
        expect(dump.source_spec_version).toBe("0.x");
    } finally {
        proc.kill();
    }
});

test("three-vrm reports source_spec_version: 1.0 for VRM 1.0 asset", async () => {
    const proc = await startAdapter();
    try {
        const load = await rpcCall(proc, "load_vrm", {
            path: "../../assets/humanoid/vroid_default_F_1_0.vrm",
        });
        const session_id = load.session_id;
        const dump = await rpcCall(proc, "dump_expression_weights", { session_id });
        expect(dump.source_spec_version).toBe("1.0");
    } finally {
        proc.kill();
    }
});
```

- [ ] **Step 4: Run tests**

```bash
cd adapters/three-vrm
npm test
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add adapters/three-vrm/src/ adapters/three-vrm/test/source_spec_version.test.ts
git commit -m "feat(adapter:three-vrm): detect + report source_spec_version on dumps"
```

---

## Task 24: VMK adapter — populate `source_spec_version` on dumps

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Session.swift` (or wherever session state lives)
- Test: `adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/SourceSpecVersionTests.swift`

- [ ] **Step 1: Detect spec version at load**

In the Swift load handler, after parsing glTF JSON, examine `extensionsUsed`. Store detected version on the session:

```swift
enum SpecVersion: String, Codable {
    case v0 = "0.x"
    case v1 = "1.0"
}

func detectSpecVersion(from gltfJson: [String: Any]) throws -> SpecVersion {
    let used = gltfJson["extensionsUsed"] as? [String] ?? []
    if used.contains("VRMC_vrm") { return .v1 }
    if used.contains("VRM") { return .v0 }
    throw AdapterError.noVRMExtension
}

// In load handler:
session.sourceSpecVersion = try detectSpecVersion(from: gltfJson)
```

- [ ] **Step 2: Echo on every dump response**

```swift
struct DumpExpressionWeightsResult: Codable {
    let sourceSpecVersion: SpecVersion
    let weights: [String: Float]

    enum CodingKeys: String, CodingKey {
        case sourceSpecVersion = "source_spec_version"
        case weights
    }
}

func handleDumpExpressionWeights(params: DumpExpressionWeightsParams, session: Session) -> DumpExpressionWeightsResult {
    let weights = session.readNativeExpressionWeights()
    return DumpExpressionWeightsResult(
        sourceSpecVersion: session.sourceSpecVersion,
        weights: weights,
    )
}
```

Repeat for the other two dump handlers.

- [ ] **Step 3: Write Swift test**

```swift
// adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/SourceSpecVersionTests.swift
import XCTest
@testable import VRMMetalKitAdapter

final class SourceSpecVersionTests: XCTestCase {
    func testReportsV0ForVrm0xAsset() throws {
        let adapter = AdapterTestHarness()
        let loadResult = try adapter.send("load_vrm", ["path": "../../../assets/humanoid/avatarA_0_0.vrm"])
        let sessionId = loadResult["session_id"] as! String
        let dump = try adapter.send("dump_expression_weights", ["session_id": sessionId])
        XCTAssertEqual(dump["source_spec_version"] as? String, "0.x")
    }
}
```

- [ ] **Step 4: Run Swift tests**

```bash
cd adapters/vrm-metal-kit
swift test
```

Expected: pass on a machine with Xcode 26.

- [ ] **Step 5: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/ adapters/vrm-metal-kit/Tests/
git commit -m "feat(adapter:vmk): detect + report source_spec_version on dumps"
```

---

## Task 25: Runner camera-convention enforcement per `spec_version`

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs` (add camera-convention check)
- Modify: `crates/vrm-runner/src/lib.rs` (or wherever the validation lives)
- Test: `crates/vrm-runner/tests/camera_convention.rs` (new)

- [ ] **Step 1: Write failing test**

Create `crates/vrm-runner/tests/camera_convention.rs`:

```rust
use vrm_runner::execute::validate_camera_convention;
use vrm_test_plan::{Camera, TestPlan};
use vrm_ops::SpecVersion;

fn plan_with(spec_version: SpecVersion, camera_z: f32) -> TestPlan {
    TestPlan {
        id: "t".into(),
        spec_version,
        spec_section: "test".into(),
        asset: "a.vrm".into(),
        camera: Camera {
            position: [0.0, 1.3, camera_z],
            target: [0.0, 1.3, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        // ... rest with defaults; adapt to TestPlan shape ...
        lighting: Default::default(),
        post_processing: Default::default(),
        output: Default::default(),
        diff: Default::default(),
        ignore_renderers: vec![],
        properties: vec![],
        physics: None,
        animation: None,
        render_sequence: None,
    }
}

#[test]
fn rejects_v0_plan_with_positive_z_camera() {
    let plan = plan_with(SpecVersion::V0, 1.5);   // camera at +Z but avatar faces -Z
    assert!(validate_camera_convention(&plan).is_err());
}

#[test]
fn accepts_v0_plan_with_negative_z_camera() {
    let plan = plan_with(SpecVersion::V0, -1.5);
    assert!(validate_camera_convention(&plan).is_ok());
}

#[test]
fn rejects_v1_plan_with_negative_z_camera() {
    let plan = plan_with(SpecVersion::V1, -1.5);   // camera at -Z but avatar faces +Z
    assert!(validate_camera_convention(&plan).is_err());
}

#[test]
fn accepts_v1_plan_with_positive_z_camera() {
    let plan = plan_with(SpecVersion::V1, 1.5);
    assert!(validate_camera_convention(&plan).is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-runner --test camera_convention`
Expected: FAIL — `validate_camera_convention` not yet exported.

- [ ] **Step 3: Implement the validator**

In `crates/vrm-runner/src/execute.rs`:

```rust
use vrm_ops::SpecVersion;
use vrm_test_plan::TestPlan;
use anyhow::{anyhow, Result};

pub fn validate_camera_convention(plan: &TestPlan) -> Result<()> {
    let camera_z = plan.camera.position[2];
    match plan.spec_version {
        SpecVersion::V0 => {
            // 0.x spec: avatar faces -Z; camera must be at negative Z to see the front.
            if camera_z >= 0.0 {
                return Err(anyhow!(
                    "test plan {} declares spec_version 0.x but camera at z={camera_z} is on the wrong side (0.x avatars face -Z; camera must be at negative Z)",
                    plan.id
                ));
            }
        }
        SpecVersion::V1 => {
            // 1.0 spec: avatar faces +Z; camera must be at positive Z to see the front.
            if camera_z <= 0.0 {
                return Err(anyhow!(
                    "test plan {} declares spec_version 1.0 but camera at z={camera_z} is on the wrong side (1.0 avatars face +Z; camera must be at positive Z)",
                    plan.id
                ));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Wire the check into `execute-test-plan`**

In the main runner entry-point, call `validate_camera_convention(&plan)?` before dispatching to the adapter. A failure aborts the run with a clear error.

- [ ] **Step 5: Run test to verify pass**

Run: `cargo test -p vrm-runner --test camera_convention`
Expected: 4 tests pass.

- [ ] **Step 6: Re-verify existing 1.0 plans still pass camera-convention check**

```bash
cargo run -p vrm-runner -- execute-test-plan \
    --plan test-plans/manual/humanoid/<one-existing-1.0-plan>.test.yaml \
    --adapter-bin target/release/vrm-mock-renderer \
    --asset-dir assets/humanoid \
    --output-dir /tmp/runner-smoke \
    --renderer-name mock
```

Expected: runs to completion (mock renderer accepts the camera).

- [ ] **Step 7: Clippy**

Run: `cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/vrm-runner/src/execute.rs crates/vrm-runner/tests/camera_convention.rs
git commit -m "feat(vrm-runner): enforce camera convention per spec_version (V0→-Z, V1→+Z)"
```

---

## Task 26: Runner cross-checks adapter `source_spec_version` against plan

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs` (add cross-check after first dump)
- Test: `crates/vrm-runner/tests/spec_version_cross_check.rs` (new)

- [ ] **Step 1: Write failing test**

Create `crates/vrm-runner/tests/spec_version_cross_check.rs`:

```rust
use vrm_runner::execute::cross_check_source_spec_version;
use vrm_ops::SpecVersion;

#[test]
fn ok_when_plan_and_adapter_agree_on_v0() {
    let result = cross_check_source_spec_version(SpecVersion::V0, SpecVersion::V0, "test_id");
    assert!(result.is_ok());
}

#[test]
fn ok_when_plan_and_adapter_agree_on_v1() {
    let result = cross_check_source_spec_version(SpecVersion::V1, SpecVersion::V1, "test_id");
    assert!(result.is_ok());
}

#[test]
fn err_when_plan_v0_but_adapter_reports_v1() {
    let result = cross_check_source_spec_version(SpecVersion::V0, SpecVersion::V1, "mtoon_basic_v0_lit_001");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("mtoon_basic_v0_lit_001"), "err msg should reference plan id; got {err}");
    assert!(err.contains("0.x"));
    assert!(err.contains("1.0"));
}

#[test]
fn err_when_plan_v1_but_adapter_reports_v0() {
    let result = cross_check_source_spec_version(SpecVersion::V1, SpecVersion::V0, "mtoon_basic_lit");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Implement the cross-check**

In `crates/vrm-runner/src/execute.rs`:

```rust
pub fn cross_check_source_spec_version(
    declared: SpecVersion,
    adapter_reported: SpecVersion,
    test_id: &str,
) -> Result<()> {
    if declared != adapter_reported {
        return Err(anyhow!(
            "spec_version mismatch on plan {test_id}: declared {} but adapter parsed {} from the asset. This indicates the asset's extensionsUsed disagrees with the test plan's spec_version declaration.",
            declared.as_str(),
            adapter_reported.as_str(),
        ));
    }
    Ok(())
}
```

Then wire it into the runner's execute loop: after the first dump-op response from the adapter, read its `source_spec_version` and call `cross_check_source_spec_version(plan.spec_version, dump_response.source_spec_version, &plan.id)?`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p vrm-runner --test spec_version_cross_check`
Expected: 4 tests pass.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p vrm-runner --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-runner/src/execute.rs crates/vrm-runner/tests/spec_version_cross_check.rs
git commit -m "feat(vrm-runner): cross-check adapter source_spec_version against plan declaration"
```

---

## Task 27: Mid-slice checkpoint — produce first two-adapter diff (three-vrm + VMK)

**Files:**
- Run scripts; produce diff output

- [ ] **Step 1: Bootstrap goldens from three-vrm + VMK**

Build adapters:

```bash
cd adapters/three-vrm && npm install && npm run build && cd ../..
cd adapters/vrm-metal-kit && swift build && cd ../..
```

- [ ] **Step 2: Run goldens bootstrap for slice 1 assets**

```bash
SKIP_VRM_METAL_KIT=0 SKIP_THREE_VRM=0 \
    scripts/bootstrap-goldens.sh \
    --plans test-plans/manual/humanoid/avatarA_0_0.test.yaml \
    --output-dir goldens-cache/slice1-checkpoint-day10/
```

(Or adapt the bootstrap script's invocation pattern to the local environment.)

- [ ] **Step 3: Run pairwise diff**

```bash
scripts/consensus-report.sh --manifest goldens-cache/slice1-checkpoint-day10/manifest.json
cat goldens-cache/slice1-checkpoint-day10/consensus-report.json | jq '.'
```

Expected: pair `three-vrm` × `vrm-metal-kit` on `avatarA_0_0_lit_baseline` shows **high SSIM divergence** — three-vrm renders the front (spec-correct), VMK renders the back (180° flipped).

- [ ] **Step 4: Visually verify the failure mode**

Inspect both rendered PNGs:

```bash
open goldens-cache/slice1-checkpoint-day10/three-vrm/avatarA_0_0_lit_baseline.png
open goldens-cache/slice1-checkpoint-day10/vrm-metal-kit/avatarA_0_0_lit_baseline.png
```

Expected: three-vrm shows the front of the avatar; VMK shows the back of the head. This is the VMK 180° flip surfacing as a conformance failure — exactly what the slice was designed to surface.

- [ ] **Step 5: Save the failure-mode image for methodology doc**

```bash
mkdir -p docs/images/
cp goldens-cache/slice1-checkpoint-day10/vrm-metal-kit/avatarA_0_0_lit_baseline.png docs/images/vmk_vrm0x_back_view_failure.png
```

This image gets embedded in `docs/methodology.md` in Task 35.

- [ ] **Step 6: Record the checkpoint outcome in docs/findings.md**

```markdown
## 2026-MM-DD — Slice 1 mid-slice checkpoint: VMK 0.x back-view flip surfaced

**Setup.** three-vrm + vrm-metal-kit rendered `avatarA_0_0_lit_baseline` (spec_version 0.x, camera at -Z).

**Result.** SSIM(three-vrm, VMK) = <value> — far below the 0.92 threshold; consensus diff flags VMK as the outlier (only one adapter on this side of the pair).

**Visual.** three-vrm renders the front of the avatar; VMK renders the back of the head. This is the VMK 180° orientation flip surfaced as a conformance failure — the slice 1 design goal.

**Next.** Phase D produces the methodology doc + site update + UniVRM + godot-vrm wiring (Phase C tasks 28+).
```

- [ ] **Step 7: Commit**

```bash
git add docs/findings.md docs/images/vmk_vrm0x_back_view_failure.png
git commit -m "docs(findings): slice 1 mid-slice checkpoint — VMK 0.x back-view flip confirmed"
```

---

## Task 28: UniVRM adapter — `canLoadVrm0X: true` + `source_spec_version` reporting

**Files:**
- Modify: `adapters/univrm/UniVrmAdapter/Operations.cs` (or wherever `Vrm10.LoadPathAsync` is called)
- Modify: `adapters/univrm/UniVrmAdapter/Session.cs` (session state)
- Test: `adapters/univrm/Tests/SourceSpecVersionTests.cs`

- [ ] **Step 1: Find the LoadPathAsync call**

```bash
grep -rn "LoadPathAsync\|canLoadVrm0X" adapters/univrm/
```

- [ ] **Step 2: Flip the flag**

Change the call site from:

```csharp
var vrm = await Vrm10.LoadPathAsync(path, canLoadVrm0X: false, ...);
```

to:

```csharp
var vrm = await Vrm10.LoadPathAsync(path, canLoadVrm0X: true, ...);
```

- [ ] **Step 3: Detect + store source_spec_version**

In the load handler, after parse, examine the loaded asset's extensions:

```csharp
private SpecVersion DetectSpecVersion(GltfRoot gltf) {
    var used = gltf.ExtensionsUsed ?? new List<string>();
    if (used.Contains("VRMC_vrm")) return SpecVersion.V1;
    if (used.Contains("VRM")) return SpecVersion.V0;
    throw new InvalidOperationException("No VRM extension found");
}

session.SourceSpecVersion = DetectSpecVersion(loadedGltf);
```

- [ ] **Step 4: Echo on every dump**

In each `dump_*` handler, include `source_spec_version` in the response.

- [ ] **Step 5: Write test**

```csharp
// adapters/univrm/Tests/SourceSpecVersionTests.cs
[Test]
public async Task UniVrmReportsV0ForVrm0xAsset() {
    var adapter = new AdapterTestHarness();
    var load = await adapter.Send("load_vrm", new { path = "../../../assets/humanoid/avatarA_0_0.vrm" });
    var sessionId = (string)load["session_id"];
    var dump = await adapter.Send("dump_expression_weights", new { session_id = sessionId });
    Assert.AreEqual("0.x", dump["source_spec_version"]);
}
```

- [ ] **Step 6: Run UniVRM build (CI build-validate)**

```bash
cd adapters/univrm
./launcher.sh build-validate
```

Expected: build succeeds. (PlayMode test runs locally only; CI is build-only per CLAUDE.md.)

- [ ] **Step 7: Commit**

```bash
git add adapters/univrm/UniVrmAdapter/ adapters/univrm/Tests/
git commit -m "feat(adapter:univrm): canLoadVrm0X: true + source_spec_version reporting"
```

---

## Task 29: UniVRM coord-handling repro through corpus

**Files:**
- Run UniVRM on the slice-1 assets; record divergence; possibly file upstream

- [ ] **Step 1: Run UniVRM against the slice-1 0.x assets**

```bash
cargo run -p vrm-runner -- execute-test-plan \
    --plan test-plans/manual/humanoid/avatarA_0_0.test.yaml \
    --adapter-bin adapters/univrm/launcher.sh \
    --asset-dir assets/humanoid \
    --output-dir /tmp/univrm-vrm0x \
    --renderer-name univrm
```

- [ ] **Step 2: Compare against three-vrm (spec-correct baseline)**

```bash
cargo run -p vrm-runner -- diff \
    --plan test-plans/manual/humanoid/avatarA_0_0.test.yaml \
    --render /tmp/univrm-vrm0x/avatarA_0_0_lit_baseline_univrm.png \
    --reference goldens-cache/slice1-checkpoint-day10/three-vrm/avatarA_0_0_lit_baseline.png \
    --renderer-name univrm \
    --json
```

- [ ] **Step 3: Inspect the failure mode**

Expected based on the design doc: UniVRM has an adapter coord-handling bug (Unity Z-flips on glTF import; our adapter doesn't compensate). Visual signature is likely also a wrong-side render (back of head) but for a different reason than VMK.

- [ ] **Step 4: Check UniVRM issue tracker**

```bash
# Manually browse: https://github.com/vrm-c/UniVRM/issues?q=is%3Aissue+vrm+0+coord
```

Search terms: "vrm 0", "vrm0x", "coord", "z-flip", "orientation". Record:
- Whether a related issue exists.
- Link if so.
- If not, file a new issue with the slice-1 reproducer attached.

- [ ] **Step 5: Record finding in docs/findings.md**

```markdown
## 2026-MM-DD — UniVRM coord-handling on VRM 0.x: reproducer

**Setup.** UniVRM (v0.131.0) loaded `avatarA_0_0.vrm` (spec_version 0.x) via the slice-1 corpus.

**Result.** <describe the rendered image — back of head? sideways? expected-but-with-different-tone?>

**Comparison.** three-vrm renders the front correctly. UniVRM renders <X>.

**Upstream status.** <Either: linked existing issue at https://github.com/vrm-c/UniVRM/issues/N; or: filed new issue at https://github.com/vrm-c/UniVRM/issues/M with the slice-1 reproducer attached.>

**Slice 1 impact.** UniVRM's 0.x render flagged as a conformance failure in the published site, alongside VMK's. The failure modes differ (VMK = 180° flip in render layer; UniVRM = Unity glTF importer Z-flip without adapter-side compensation), making the consensus diff a cleaner signal — two adapters pass (three-vrm + godot-vrm), two fail differently.
```

- [ ] **Step 6: Commit**

```bash
git add docs/findings.md
git commit -m "docs(findings): UniVRM VRM 0.x coord-handling repro through slice 1 corpus"
```

---

## Task 30: godot-vrm adapter — `source_spec_version` reporting

**Files:**
- Modify: `adapters/godot-vrm/src/operations.gd` (echo source_spec_version on dumps)
- Modify: `adapters/godot-vrm/src/session.gd` (store detected version on load)
- Modify: `crates/vrm-godot-shim/src/bridge.rs` (pass-through)
- Test: smoke via the runner

- [ ] **Step 1: Detect spec version on load (GDScript side)**

In `adapters/godot-vrm/src/operations.gd` (or `loader.gd`), after parsing the glTF JSON:

```gdscript
func _detect_spec_version(gltf_json: Dictionary) -> String:
    var used: Array = gltf_json.get("extensionsUsed", [])
    if "VRMC_vrm" in used:
        return "1.0"
    if "VRM" in used:
        return "0.x"
    push_error("No VRM extension found")
    return ""

# In load handler:
session.source_spec_version = _detect_spec_version(gltf_json)
```

- [ ] **Step 2: Echo on each dump**

```gdscript
func handle_dump_expression_weights(params, session):
    var weights = _read_native_expression_weights(session)
    return {
        "source_spec_version": session.source_spec_version,
        "weights": weights,
    }
```

Repeat for the other two dump handlers.

- [ ] **Step 3: Update shim**

`crates/vrm-godot-shim/src/bridge.rs` likely already does serde pass-through. Verify that `source_spec_version` flows through without truncation. If the shim does typed Rust deserialization, ensure the corresponding result types include the field (would have been picked up automatically from Task 21).

- [ ] **Step 4: Smoke through the runner**

```bash
cargo build --release -p vrm-godot-shim
cargo run -p vrm-runner -- execute-test-plan \
    --plan test-plans/manual/humanoid/avatarA_0_0.test.yaml \
    --adapter-bin target/release/vrm-godot-shim \
    --asset-dir assets/humanoid \
    --output-dir /tmp/godot-vrm0x \
    --renderer-name godot-vrm \
    --json | jq '.dumps[0].source_spec_version'
```

Expected: `"0.x"`.

- [ ] **Step 5: Commit**

```bash
git add adapters/godot-vrm/src/ crates/vrm-godot-shim/src/bridge.rs
git commit -m "feat(adapter:godot-vrm): detect + report source_spec_version on dumps"
```

---

## Task 31: Mock renderer — `source_spec_version` reporting (for CI smoke)

**Files:**
- Modify: `crates/vrm-mock-renderer/src/handlers.rs` (echo source_spec_version on dumps)
- Modify: `crates/vrm-mock-renderer/src/session.rs` (or wherever session state lives)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Detect spec version at load**

In the mock renderer's load handler, parse the .vrm glb's JSON chunk and read `extensionsUsed`. Store detected version on the session.

- [ ] **Step 2: Echo on dumps**

Add `source_spec_version` to every dump response.

- [ ] **Step 3: Write test**

```rust
#[test]
fn mock_renderer_reports_source_spec_version_v0() {
    let mut handler = MockRenderer::new();
    let load = handler.handle_load_vrm(LoadVrmParams { path: "assets/humanoid/avatarA_0_0.vrm".into() }).unwrap();
    let dump = handler.handle_dump_expression_weights(DumpExpressionWeightsParams {
        session_id: load.session_id,
        as_spec_version: None,
    }).unwrap();
    assert_eq!(dump.source_spec_version, SpecVersion::V0);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vrm-mock-renderer`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-mock-renderer/src/
git commit -m "feat(vrm-mock-renderer): detect + report source_spec_version on dumps"
```

**End of Phase C.** All four adapters report `source_spec_version`; mid-slice checkpoint surfaced VMK 0.x flip; UniVRM coord bug isolated. Phase D produces the normalization, methodology doc, site, and external-announcement closure.

---

# Phase D — Normalization + closure (days 18–21)

Gate at end-of-slice (day 21): four-adapter diff produced; methodology doc live; site spec_version filter chip + badge deployed; round-trip property test passes in CI; announcement-ready.

## Task 32: `vrm_normalize::expressions` — joy→happy mapping table

**Files:**
- Modify: `crates/vrm-normalize/src/expressions.rs` (fill the stub)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the mapping table + failing test**

Replace `crates/vrm-normalize/src/expressions.rs` content:

```rust
//! Expression preset normalization. v0 `blendShapeMaster` preset names
//! → v1 `VRMC_vrm.expressions.preset` preset names.
//!
//! v0 weight range is 0–100; v1 weight range is 0–1. Normalization
//! divides by 100.

use std::collections::HashMap;

/// Returns the v1 preset name for the given v0 preset name. v0 customs
/// pass through with `custom:<name>` markers; unrecognized standard
/// presets are an error (would indicate an asset using an off-spec preset
/// name).
pub fn normalize_preset_name_v0_to_v1(v0_preset: &str) -> String {
    match v0_preset {
        "joy" => "happy".into(),
        "angry" => "angry".into(),
        "sorrow" => "sad".into(),
        "fun" => "relaxed".into(),
        "neutral" => "neutral".into(),
        "a" => "aa".into(),
        "i" => "ih".into(),
        "u" => "ou".into(),
        "e" => "ee".into(),
        "o" => "oh".into(),
        "blink" => "blink".into(),
        "blink_l" => "blinkLeft".into(),
        "blink_r" => "blinkRight".into(),
        "lookup" => "lookUp".into(),
        "lookdown" => "lookDown".into(),
        "lookleft" => "lookLeft".into(),
        "lookright" => "lookRight".into(),
        // Anything that doesn't match a known v0 preset is treated as custom.
        custom => format!("custom:{custom}"),
    }
}

/// Normalize a complete v0 weight map (preset → weight 0..100) to v1
/// (preset → weight 0..1).
pub fn normalize_weights_v0_to_v1(v0_weights: &HashMap<String, f32>) -> HashMap<String, f32> {
    v0_weights.iter()
        .map(|(k, v)| (normalize_preset_name_v0_to_v1(k), v / 100.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joy_maps_to_happy() {
        assert_eq!(normalize_preset_name_v0_to_v1("joy"), "happy");
    }

    #[test]
    fn sorrow_maps_to_sad() {
        assert_eq!(normalize_preset_name_v0_to_v1("sorrow"), "sad");
    }

    #[test]
    fn fun_maps_to_relaxed() {
        assert_eq!(normalize_preset_name_v0_to_v1("fun"), "relaxed");
    }

    #[test]
    fn vowels_map_to_phonetic() {
        assert_eq!(normalize_preset_name_v0_to_v1("a"), "aa");
        assert_eq!(normalize_preset_name_v0_to_v1("i"), "ih");
        assert_eq!(normalize_preset_name_v0_to_v1("u"), "ou");
        assert_eq!(normalize_preset_name_v0_to_v1("e"), "ee");
        assert_eq!(normalize_preset_name_v0_to_v1("o"), "oh");
    }

    #[test]
    fn blink_variants_map_correctly() {
        assert_eq!(normalize_preset_name_v0_to_v1("blink_l"), "blinkLeft");
        assert_eq!(normalize_preset_name_v0_to_v1("blink_r"), "blinkRight");
    }

    #[test]
    fn custom_passes_through_with_marker() {
        assert_eq!(normalize_preset_name_v0_to_v1("MyCustom"), "custom:MyCustom");
    }

    #[test]
    fn weights_scale_from_0_100_to_0_1() {
        let mut v0 = HashMap::new();
        v0.insert("joy".into(), 100.0);
        v0.insert("neutral".into(), 50.0);
        let v1 = normalize_weights_v0_to_v1(&v0);
        assert!((v1["happy"] - 1.0).abs() < 1e-6);
        assert!((v1["neutral"] - 0.5).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vrm-normalize expressions`
Expected: 7 tests pass.

- [ ] **Step 3: Clippy**

Run: `cargo clippy -p vrm-normalize --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-normalize/src/expressions.rs
git commit -m "feat(vrm-normalize): expression preset mapping (v0 joy→v1 happy etc.) + weight scaling"
```

---

## Task 33: `vrm_normalize::humanoid` — bone-name normalization

**Files:**
- Modify: `crates/vrm-normalize/src/humanoid.rs`
- Test: inline

- [ ] **Step 1: Identify the v0↔v1 bone-name diff**

Per VRM specs, humanoid bone names are mostly identical between 0.x and 1.0. The few differences:
- `leftUpperLeg` vs `leftUpperLeg` (same)
- VRM 1.0 normalized a handful of names; check `docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.humanoid.bone.schema.json` against `docs/upstream-specs/vrm-specification/specification/VRMC_vrm-1.0/humanoid.md` for the canonical lists.

Most are identity; the normalizer's job is to pass through identical names and flag anything truly renamed.

- [ ] **Step 2: Write the mapping + tests**

Replace `crates/vrm-normalize/src/humanoid.rs`:

```rust
//! Humanoid bone-name normalization v0 ↔ v1.
//!
//! Bone names are mostly identical between specs. This module exists for
//! API symmetry and to flag any renames that emerge (currently: none
//! confirmed in slice 1; module is mostly identity).

pub fn normalize_bone_name_v0_to_v1(v0_name: &str) -> String {
    // Slice 1: identity mapping. If specific renames surface during
    // adapter testing, they get added here as named match arms with
    // citations to the relevant spec section.
    v0_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_bone_names_pass_through() {
        for name in &["hips", "spine", "chest", "neck", "head",
                      "leftUpperArm", "rightUpperLeg", "leftEye", "rightEye"] {
            assert_eq!(normalize_bone_name_v0_to_v1(name), *name);
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vrm-normalize humanoid`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-normalize/src/humanoid.rs
git commit -m "feat(vrm-normalize): humanoid bone-name normalization (identity for slice 1)"
```

---

## Task 34: `vrm_normalize::look_at` — normalization

**Files:**
- Modify: `crates/vrm-normalize/src/look_at.rs`
- Test: inline

- [ ] **Step 1: Inspect the v0 vs v1 look_at shape diff**

- VRM 0.x: `firstPerson.lookAtTypeName: "Bone" | "BlendShape"` + curve-based offsets (`lookAtHorizontalInner`, etc.).
- VRM 1.0: `VRMC_vrm.lookAt.type: "bone" | "expression"` + range mapping.

Both ultimately produce yaw/pitch state. The dump op should return yaw/pitch in degrees; normalization to v1 is a struct-shape rewrite (no value transformation needed if both adapters return native yaw/pitch).

- [ ] **Step 2: Write the module + tests**

Replace `crates/vrm-normalize/src/look_at.rs`:

```rust
//! `look_at` state normalization v0 ↔ v1.
//!
//! Both spec versions ultimately express look_at as yaw + pitch angles
//! (degrees). Normalization is a passthrough of those scalars; the
//! difference is in how the *adapter* computes them from the asset
//! definition, which is the adapter's responsibility.

pub fn normalize_look_at_state_v0_to_v1(yaw_degrees: f32, pitch_degrees: f32) -> (f32, f32) {
    (yaw_degrees, pitch_degrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_pitch_pass_through() {
        let (yaw, pitch) = normalize_look_at_state_v0_to_v1(10.0, -5.0);
        assert_eq!(yaw, 10.0);
        assert_eq!(pitch, -5.0);
    }
}
```

- [ ] **Step 3: Run tests; commit**

```bash
cargo test -p vrm-normalize look_at
git add crates/vrm-normalize/src/look_at.rs
git commit -m "feat(vrm-normalize): look_at yaw/pitch normalization (passthrough; structural)"
```

---

## Task 35: Runner applies normalization when `as_spec_version=V1` requested

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs` (apply vrm-normalize post-dump)
- Modify: `crates/vrm-runner/Cargo.toml` (add vrm-normalize dep)
- Test: integration test

- [ ] **Step 1: Add the dep**

In `crates/vrm-runner/Cargo.toml`:

```toml
vrm-normalize = { path = "../vrm-normalize" }
```

- [ ] **Step 2: Write the runner-side helper**

In `crates/vrm-runner/src/execute.rs`:

```rust
use vrm_normalize::expressions::normalize_weights_v0_to_v1;
use vrm_ops::SpecVersion;

pub fn apply_normalization_if_requested(
    weights: &HashMap<String, f32>,
    source: SpecVersion,
    requested: Option<SpecVersion>,
) -> Result<HashMap<String, f32>> {
    match (source, requested) {
        (_, None) => Ok(weights.clone()),
        (SpecVersion::V0, Some(SpecVersion::V1)) => Ok(normalize_weights_v0_to_v1(weights)),
        (SpecVersion::V1, Some(SpecVersion::V1)) => Ok(weights.clone()),
        (SpecVersion::V0, Some(SpecVersion::V0)) => Ok(weights.clone()),
        (SpecVersion::V1, Some(SpecVersion::V0)) => Err(anyhow!(
            "-32001 NormalizationDirectionUnsupported: cannot project a 1.0 dump as 0.x — no lossless mapping exists for v1 presets like 'surprised'"
        )),
    }
}
```

- [ ] **Step 3: Write integration test**

```rust
// crates/vrm-runner/tests/normalize_dispatch.rs
use vrm_runner::execute::apply_normalization_if_requested;
use vrm_ops::SpecVersion;
use std::collections::HashMap;

fn weights_v0() -> HashMap<String, f32> {
    let mut m = HashMap::new();
    m.insert("joy".into(), 100.0);
    m
}

#[test]
fn no_as_spec_version_returns_native() {
    let w = weights_v0();
    let out = apply_normalization_if_requested(&w, SpecVersion::V0, None).unwrap();
    assert_eq!(out.get("joy"), Some(&100.0));
    assert!(out.get("happy").is_none());
}

#[test]
fn as_spec_version_v1_against_v0_normalizes() {
    let w = weights_v0();
    let out = apply_normalization_if_requested(&w, SpecVersion::V0, Some(SpecVersion::V1)).unwrap();
    assert_eq!(out.get("happy"), Some(&1.0));
    assert!(out.get("joy").is_none());
}

#[test]
fn as_spec_version_v0_against_v1_rejected() {
    let mut w = HashMap::new();
    w.insert("happy".into(), 1.0);
    let result = apply_normalization_if_requested(&w, SpecVersion::V1, Some(SpecVersion::V0));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("-32001"));
}
```

- [ ] **Step 4: Run; clippy; commit**

```bash
cargo test -p vrm-runner --test normalize_dispatch
cargo clippy -p vrm-runner --all-targets -- -D warnings
git add crates/vrm-runner/src/execute.rs crates/vrm-runner/Cargo.toml crates/vrm-runner/tests/normalize_dispatch.rs
git commit -m "feat(vrm-runner): apply vrm-normalize when as_spec_version requested; reject v1→v0"
```

---

## Task 36: Normalization round-trip property test in CI

**Files:**
- Create: `crates/vrm-normalize/tests/round_trip_property.rs`

This task delivers slice-1 success criterion #3 (vrm-normalize round-trip property test in CI) via a pure-Rust unit-level property test against the normalization function. The stronger cross-renderer round-trip (real adapters compared after normalization) is a follow-up that's deferred to slice 2's retrospective — it requires runner support for dumping adapter dump-op JSON to stdout, which is out of slice 1 scope.

- [ ] **Step 1: Write the property test**

Create `crates/vrm-normalize/tests/round_trip_property.rs`:

```rust
//! Round-trip property: for any pair of v0-shape weight maps that are
//! losslessly equivalent (same preset names, same weights), normalization
//! to v1 produces equal v1-shape maps.
//!
//! This is the unit-level guarantee that satisfies slice 1 success
//! criterion #3 ("vrm-normalize round-trip property test passes in CI").
//! The stronger cross-renderer property — adapter A and adapter B agree
//! on `dump(as_spec_version=V1)` even when native dumps differ — depends
//! on this guarantee plus correct adapter implementation. Slice 1 ships
//! the unit-level proof; cross-renderer extension lands later.

use std::collections::HashMap;
use vrm_normalize::expressions::{normalize_preset_name_v0_to_v1, normalize_weights_v0_to_v1};

#[test]
fn equivalent_v0_maps_normalize_to_equal_v1_maps() {
    let mut a: HashMap<String, f32> = HashMap::new();
    a.insert("joy".into(), 100.0);
    a.insert("neutral".into(), 50.0);
    a.insert("blink_l".into(), 25.0);

    // Same v0-shape data, constructed independently.
    let mut b: HashMap<String, f32> = HashMap::new();
    b.insert("joy".into(), 100.0);
    b.insert("neutral".into(), 50.0);
    b.insert("blink_l".into(), 25.0);

    let norm_a = normalize_weights_v0_to_v1(&a);
    let norm_b = normalize_weights_v0_to_v1(&b);
    assert_eq!(norm_a, norm_b);

    // Sanity check the mapping itself.
    assert_eq!(norm_a.get("happy"), Some(&1.0));
    assert_eq!(norm_a.get("neutral"), Some(&0.5));
    assert_eq!(norm_a.get("blinkLeft"), Some(&0.25));
}

#[test]
fn preset_mapping_is_deterministic() {
    // Calling normalize_preset_name_v0_to_v1 on the same input always
    // produces the same output.
    for v0 in ["joy", "neutral", "blink_l", "lookleft", "a", "i", "u", "e", "o", "MyCustomShape"] {
        let one = normalize_preset_name_v0_to_v1(v0);
        let two = normalize_preset_name_v0_to_v1(v0);
        assert_eq!(one, two, "mapping non-deterministic for {v0}");
    }
}

#[test]
fn weight_scaling_round_trips_within_float_precision() {
    // 0..100 (v0) → divide by 100 → 0..1 (v1). Round-trip equivalence
    // for the corpus's canonical weight values.
    for v0_weight in [0.0_f32, 25.0, 50.0, 75.0, 100.0] {
        let mut m = HashMap::new();
        m.insert("joy".into(), v0_weight);
        let v1 = normalize_weights_v0_to_v1(&m);
        let v1_weight = *v1.get("happy").unwrap();
        let expected = v0_weight / 100.0;
        assert!((v1_weight - expected).abs() < 1e-6,
            "v0_weight={v0_weight}, v1_weight={v1_weight}, expected={expected}");
    }
}

#[test]
fn custom_preset_passthrough_preserves_uniqueness() {
    // Two distinct custom names normalize to two distinct v1 markers.
    let mut a: HashMap<String, f32> = HashMap::new();
    a.insert("MyCustom1".into(), 50.0);
    a.insert("MyCustom2".into(), 75.0);
    let norm_a = normalize_weights_v0_to_v1(&a);
    assert_eq!(norm_a.get("custom:MyCustom1"), Some(&0.5));
    assert_eq!(norm_a.get("custom:MyCustom2"), Some(&0.75));
    // Distinct keys preserved.
    assert_eq!(norm_a.len(), 2);
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p vrm-normalize --test round_trip_property`
Expected: 4 tests pass.

- [ ] **Step 3: Verify the test runs in default CI invocation**

Run: `cargo test --workspace`
Expected: includes the round-trip property test; passes.

CI's `rust.yml` workflow already runs `cargo test --workspace` (or equivalent), so no workflow file changes needed. Verify by inspecting `.github/workflows/rust.yml`.

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p vrm-normalize --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-normalize/tests/round_trip_property.rs
git commit -m "test(vrm-normalize): round-trip property test in CI (slice 1 criterion #3)"
```

---

## Task 37: Methodology doc — new "VRM 0.x conformance" section

**Files:**
- Modify: `docs/methodology.md` (append new section)

- [ ] **Step 1: Write the section**

Append to `docs/methodology.md`:

````markdown
## VRM 0.x conformance

The corpus exercises VRM 0.x assets in parallel with VRM 1.0. Spec-version metadata threads through the manifest, test plan, and runner (`spec_version: "0.x" | "1.0"`); the runner enforces version-specific methodology pins below.

### Camera convention (per-spec-version)

The two spec versions specify **opposite** default avatar orientations in glTF coordinates:

- **VRM 0.x:** avatar faces -Z. [Spec ref: specification/0.0/README.md:238.] Test plans place the camera at -Z (target = origin) to see the front of a spec-conformant render.
- **VRM 1.0:** avatar faces +Z. [Spec ref: VRMC_vrm-1.0/tpose.md Definition 1.1.] Test plans place the camera at +Z.

The runner enforces this — a test plan declaring `spec_version: "0.x"` with a camera at positive Z is rejected with a clear error.

### Failure mode: back-of-head render

A renderer that applies a non-spec 180° rotation on VRM 0.x assets (relative to the spec's -Z facing direction) produces a back-of-head image when the camera is correctly placed at -Z. This is exactly the slice-1 finding against `vrm-metal-kit`:

![VMK back-view failure on VRM 0.x](images/vmk_vrm0x_back_view_failure.png)

**How to read this in the consensus diff:** when three-vrm + godot-vrm + (post-fix) UniVRM render the front and VMK renders the back, the consensus diff flags VMK as the outlier. The visual difference is large enough that SSIM collapses near zero — there's no methodology-tolerance band that would obscure this; it is a hard conformance failure.

### Normalization is one-directional and lossy

The runner can normalize 0.x dumps to a 1.0-equivalent shape via `vrm-normalize` (called via `dump_expression_weights --as_spec_version=1.0` etc.). This is a **view** concern, not a control concern — write/control ops always operate in the asset's native namespace. Default behavior on dumps is `native`: never normalize unless explicitly asked.

| v0 (`blendShapeMaster`) preset | v1 (`VRMC_vrm.expressions.preset`) preset |
|---|---|
| `joy` | `happy` |
| `angry` | `angry` |
| `sorrow` | `sad` |
| `fun` | `relaxed` |
| `neutral` | `neutral` |
| `a`, `i`, `u`, `e`, `o` | `aa`, `ih`, `ou`, `ee`, `oh` |
| `blink` / `blink_l` / `blink_r` | `blink` / `blinkLeft` / `blinkRight` |
| `lookup` / `lookdown` / `lookleft` / `lookright` | `lookUp` / `lookDown` / `lookLeft` / `lookRight` |
| custom (any other) | `custom:<original-name>` (passed through, never dropped) |

**v1→v0 is rejected.** v1 has presets like `surprised` without a lossless v0 equivalent; the runner rejects `as_spec_version=V0` against a v1 asset with error `-32001 NormalizationDirectionUnsupported`.

**Weight scaling.** v0 weight range is 0–100 (Unity convention); v1 is 0–1. Normalization divides by 100.

### Spec-version detection and three hard-error gates

The runner cross-checks `spec_version` at three places:

1. **Plan ↔ manifest**: `test_plan.spec_version` must equal `manifest.spec_version` for the corresponding entry.
2. **Plan camera ↔ spec_version**: camera Z direction must match the avatar-facing direction for the declared spec.
3. **Plan ↔ adapter-reported `source_spec_version`**: the adapter parses the asset's `extensionsUsed` and echoes the detected version on every dump response; the runner cross-checks.

A failure on any of these aborts the run with a clear error message.

### v0-specific quirk-sweep families (slice 2+)

The `_v0_quirk_*` prefix denotes intentional probes of 0.x spec corners that adapters often silently correct. Examples (landing in slice 2):

- `stiffinessForce` — canonical typo in the 0.x spec spring-bone field. An adapter that "fixes" the typo by also accepting `stiffness` is silently non-conformant.
- centerNode-as-transform vs centerNode-ignored.
- Single-bone-per-group spring-bone topology.
- Sphere-collider-only enforcement (capsule colliders must be rejected on 0.x, not silently handled).

These exist explicitly to surface adapter behavior on the weird parts of 0.x.
````

- [ ] **Step 2: Verify image link works**

```bash
ls docs/images/vmk_vrm0x_back_view_failure.png
```

Expected: present (from Task 27).

- [ ] **Step 3: Commit**

```bash
git add docs/methodology.md
git commit -m "docs(methodology): new VRM 0.x conformance section (camera, normalization, hard-error gates)"
```

---

## Task 38: Site — `spec_version` filter chip + per-card badge

**Files:**
- Modify: `site/src/manifest.ts` (or wherever the type lives) — add spec_version field
- Modify: `site/src/components/FilterBar.tsx` (or equivalent) — add filter chip
- Modify: `site/src/components/ComparisonCard.tsx` (or equivalent) — add badge
- Test: site builds + visual check via `npm run dev`

- [ ] **Step 1: Add `spec_version` to the manifest TypeScript type**

```typescript
// site/src/manifest.ts
export interface ManifestEntry {
  test_id: string;
  spec_version: "0.x" | "1.0";   // NEW
  // ... existing fields ...
}
```

- [ ] **Step 2: Add filter chip**

In the filter UI component (likely a React or Vue component, depending on the stack):

```tsx
type SpecVersionFilter = "all" | "0.x" | "1.0";

function FilterBar({ value, onChange }: { value: SpecVersionFilter; onChange: (v: SpecVersionFilter) => void }) {
    return (
        <div className="filter-chips">
            <Chip active={value === "all"} onClick={() => onChange("all")}>All</Chip>
            <Chip active={value === "0.x"} onClick={() => onChange("0.x")}>VRM 0.x</Chip>
            <Chip active={value === "1.0"} onClick={() => onChange("1.0")}>VRM 1.0</Chip>
        </div>
    );
}
```

Wire the filter to the manifest-entry-list filter logic.

- [ ] **Step 3: Add per-card badge**

In the comparison-card component:

```tsx
<div className="spec-version-badge" data-spec={entry.spec_version}>
    {entry.spec_version === "0.x" ? "0.x" : "1.0"}
</div>
```

With CSS:

```css
.spec-version-badge {
    display: inline-block;
    padding: 2px 6px;
    font-size: 11px;
    border-radius: 4px;
    background: var(--badge-bg, #eee);
}
.spec-version-badge[data-spec="0.x"] { background: #e8d8f0; color: #5b2a82; }
.spec-version-badge[data-spec="1.0"] { background: #d8e8f0; color: #2a5b82; }
```

- [ ] **Step 4: Run dev server + visual check**

```bash
cd site && npm install && npm run dev
```

Open the dev URL, verify:
- Filter chips render and toggle.
- Per-card badges show "0.x" or "1.0" based on `spec_version`.
- Filtering to "VRM 0.x" hides all 1.0 entries and vice versa.

- [ ] **Step 5: Run build**

```bash
cd site && npm run build
```

Expected: builds cleanly; `site/dist/` produced.

- [ ] **Step 6: Commit**

```bash
git add site/src/
git commit -m "feat(site): spec_version filter chip + per-card badge"
```

---

## Task 39: End-of-slice bootstrap + announcement materials

**Files:**
- Run end-to-end bootstrap; update manifest; produce announcement summary

- [ ] **Step 1: Run full bootstrap across all four real adapters**

```bash
scripts/bootstrap-goldens.sh --plans test-plans/manual/humanoid/avatarA_0_0.test.yaml
scripts/bootstrap-goldens.sh --plans <path-to-mtoon-basic-v0-and-expressions-plans>
```

(Adapt the script invocation to whatever the actual interface is — see CLAUDE.md "End-to-end smoke and goldens scripts" for the canonical command.)

- [ ] **Step 2: Verify manifest correctness**

```bash
cargo run -p vrm-s3 --bin validate-manifest -- goldens/manifest.json
```

Expected: zero errors.

- [ ] **Step 3: Push goldens + manifest update**

If `VRM_GOLDENS_BUCKET` is set, the bootstrap will have pushed the new PNGs to S3 and updated the manifest with real `s3://` URLs. Verify:

```bash
jq '.entries[] | select(.spec_version == "0.x") | .image_url' goldens/manifest.json
```

Expected: real `s3://` URLs (not `file://`).

- [ ] **Step 4: Run consensus report**

```bash
scripts/consensus-report.sh
cat goldens-cache/consensus-report.json | jq '.outliers'
```

Expected: VMK (and possibly UniVRM, depending on Task 29 outcome) flagged as outliers on `avatarA_0_0_lit_baseline` and other 0.x entries.

- [ ] **Step 5: Update site `goldens/manifest.json` reference**

The site reads `goldens/manifest.json` at build time; the new entries should appear automatically. Re-run `npm run build`:

```bash
cd site && npm run build
```

- [ ] **Step 6: Deploy site to GitHub Pages**

Via the existing `site.yml` workflow (auto-deploys on `main`). Just push the slice-1 commits.

- [ ] **Step 7: Draft announcement summary in docs/findings.md**

```markdown
## 2026-MM-DD — Slice 1 end-of-slice: VRM 0.x conformance ships

**Scope.** Four-adapter conformance coverage for VRM 0.x landed:
- Tier 2 canonical: `<vroid_default_F_0_0 | avatarA_0_0>` (per Task 8/19 outcome).
- Parametric: `mtoon_basic_v0` (3 variants, one NotApplicable: OutlineLightingMixV1Only).
- Normalization: `expressions_preset_basic_v0` (joy, neutral) ↔ `expressions_preset_basic` (happy, neutral).

**Cross-cutting infrastructure.**
- `SpecVersion::{V0, V1}` enum in `vrm-ops`, threaded through manifest, test plan, ops contract.
- `vrm-normalize` crate (joy→happy preset mapping; weight scaling; passthrough-with-marker for custom blendshapes).
- `SweepApplicability::{Applicable, NotApplicable{reason}}` registry; symmetry assertion in CI.
- Runner enforces three hard-error gates: plan↔manifest, plan camera↔spec_version, plan↔adapter source_spec_version.
- Read-side `as_spec_version` param on `dump_*` ops; default native; v1→v0 rejected.

**Findings.**
- **VMK 180° flip on VRM 0.x confirmed and surfaced** as a conformance failure with clear visual signal in the published site. <Adapter-shim or upstream-library location per Task 9.>
- **UniVRM coord-handling bug isolated** through the slice 1 corpus; <issue link from Task 29>.
- three-vrm + godot-vrm pass cleanly on the spec-mandated -Z camera convention.

**External announcement.** Share with Frans / 0b5vr / Lyuma. Site URL: <https://<gh-pages-url>>. Announcement copy emphasises the four-adapter consensus diff, the deliberate cross-renderer divergence on VRM 0.x orientation, and the slice 1 design that surfaced the failures.

**Next.** Slice 2 (spring-bone v0 + full MToon parametric parity) — implementation plan to follow.
```

- [ ] **Step 8: Commit**

```bash
git add docs/findings.md goldens/manifest.json
git commit -m "docs(findings): slice 1 end-of-slice — four-adapter VRM 0.x conformance shipped"
```

- [ ] **Step 9: Verify slice 1 success criteria**

Run through the success criteria from the plan header:

1. ✅ Four-adapter diff produced on `mtoon_basic_v0_lit_001` and `expressions_preset_basic_v0`.
2. ✅ VMK 180° flip flagged in published site (visible at `/site/index.html` filtered to 0.x).
3. ✅ `vrm-normalize` round-trip property test passes in CI (rust.yml step from Task 36).
4. ✅ Methodology doc section live (`docs/methodology.md`, "VRM 0.x conformance").
5. ✅ `spec_version` field present on every manifest entry; validator enforces (Task 5).
6. ✅ Sweep registry symmetry assertion passes (`cargo test sweep_registry_symmetric` — Task 18).

If any criterion fails, **slice 1 does not ship** — return to the relevant task and fix before announcement.

- [ ] **Step 10: Update RFC 0006 status (optional, since slice 1 is one of four)**

If wanted: edit `rfcs/0006-vrm-0x-conformance.md` Status line from "Draft (scope sketch — design TBD)" to "Accepted — see `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`". Commit. (Or wait until slice 4 to mark Accepted.)

---

**End of slice 1.** Next: slice 2 implementation plan, written after end-of-slice retrospective so unknowns surfaced by slice 1 feed forward.
