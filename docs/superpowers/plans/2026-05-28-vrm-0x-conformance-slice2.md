# VRM 0.x Conformance — Slice 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the full VRM 1.0 sweep corpus (MToon material sweeps + spring-bone sweeps) to also emit at VRM 0.x, so the same cross-renderer conformance suite that runs on 1.0 can run on 0.0 and produce a four-adapter consensus diff.

**Architecture:** Thread the existing `vrm_ops::SpecVersion` enum (already used by `emit-default`) through every `Emit*Sweep` clap arm in `crates/vrm-asset-generator/src/cli.rs`. MToon material sweeps route trivially — `emit_with_sidecars_v0(params, stem)` already exists with the *identical signature* to `emit_with_sidecars(params, stem)`, so the sweep-registry functions (which return `Vec<MToonParams>`) are unchanged; the arm just selects the emit fn by `spec_version`. Spring-bone sweeps require a new `spring_bone_v0.rs` emit path (VRM 0.x `secondaryAnimation` instead of `VRMC_springBone`) which does not exist yet — this is the bulk of the slice. Per-sweep `SweepApplicability` marks axes absent from 0.x (e.g. `KHR_texture_transform` predates 0.x, capsule colliders are v1-only) as structured `NotApplicable` reasons rather than silently emitting wrong assets.

**Tech Stack:** Rust 1.88 (workspace toolchain), `clap` derive subcommands, `vrm_ops::SpecVersion`, `serde_json`, the `mrxz/vrm-validator` shim via `crates/vrm-validator-wrap` (ignored-test gate), `cargo test --workspace`.

**Source of truth note:** Several tasks reference fields of `MToonParams` (`crates/vrm-asset-generator/src/params.rs`) and `SpringBoneParams` (`src/spring_bone.rs`). The exact field names MUST be read from the live source at execution time — the TDD loop (write test → run → see compile error → fix) surfaces any mismatch immediately. Where this plan says "mirror the existing v0 path," the canonical reference is `emit_with_sidecars_v0` (emit.rs:433) and `mtoon_v0::emit_material_property` (mtoon_v0.rs:14), which already produce spec-valid 0.x MToon.

**Design reference:** `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`, "Slice 2 — Spring-bone v0 + MToon parametric parity". This plan was deliberately deferred until the slice-1 retrospective (design line 55); slice 1 is merged (`main` includes the `SpecVersion` enum, `vrm_ext_v0.rs`, `mtoon_v0.rs`, `mtoon_common.rs`, `expressions_v0.rs`, and the `SweepApplicability` enum).

---

## Background: what already works vs. what's missing

**Already works (slice 1, merged):**
- `vrm-asset-generator emit-default --spec-version 0.x` emits a spec-valid VRM 0.0 sphere (`emit_with_sidecars_v0` at `emit.rs:433`).
- `mtoon_basic_v0_sweep()` (`sweep.rs:717`) returns `Vec<(MToonParams, SweepApplicability)>` — the 3-variant slice-1 0.x MToon proof, with one `NotApplicable { reason: OutlineLightingMixV1Only }`.
- `SweepApplicability` / `NotApplicableReason` enums (`lib.rs:29` / `lib.rs:35`).
- The compile-time symmetry test `sweep_registry_symmetric_across_versions` (sweep.rs `registry_symmetry_tests` module ~line 2140).
- `emit-mtoon-basic-v0-sweep` clap arm + handler (`cli.rs:342` variant, `cli.rs:1668` handler) — the template for the 0.x emit-arm pattern.

**Missing (this slice):**
- `--spec-version` on the 11 MToon material/texture sweep arms and the 9 spring-bone sweep arms (all 20 are 1.0-only today; verified against a freshly rebuilt binary).
- `spring_bone_v0.rs` — VRM 0.x `secondaryAnimation` emit. The design names this NEW for slice 2 (design line 154). No 0.x spring-bone asset can be emitted until this exists.
- Per-sweep `SweepApplicability` for the texture/collider sweeps so v1-only axes don't emit bogus 0.x assets.
- Bootstrap/methodology wiring to render the 0.x corpus through adapters and read the diff with the spring-bone triage-order pin.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/vrm-asset-generator/src/cli.rs` | clap subcommand enum + `run()` dispatch | Add `spec_version` arg to 20 `Emit*Sweep` variants; route each handler to v0/v1 emit |
| `crates/vrm-asset-generator/src/sweep.rs` | MToon sweep registries + symmetry test | Add per-sweep `SweepApplicability` tables for texture sweeps; extend symmetry test |
| `crates/vrm-asset-generator/src/spring_bone_v0.rs` | **NEW** — VRM 0.x `secondaryAnimation` emit | Create; mirrors `spring_bone.rs` topology but emits 0.x JSON |
| `crates/vrm-asset-generator/src/emit.rs` | sidecar emit orchestration | Add `emit_with_sidecars_spring_bone_v0` (+ swing variant) routing through `spring_bone_v0` |
| `crates/vrm-asset-generator/src/lib.rs` | crate root re-exports | Add `mod spring_bone_v0;` + re-export |
| `crates/vrm-asset-generator/src/vrm_ext_v0.rs` | VRM 0.x `VRM` extension block | Extend to carry `secondaryAnimation` (currently emits material/blendshape only) |
| `scripts/bootstrap-goldens.sh` | corpus generation + render loop | Add a `SPEC_VERSION` env knob that re-runs sweep emit at 0.x into a parallel staging dir |
| `docs/methodology.md` | methodology pins | Add the spring-bone triage-order (within-renderer-cross-version-first) section |
| `docs/findings.md` | cross-renderer findings | Append the slice-2 0.x corpus results entry (at execution close) |

---

## Phase A — MToon material sweeps at 0.x

The MToon sweeps are the easy win: `emit_with_sidecars_v0` is a drop-in for `emit_with_sidecars`. We add `--spec-version` to each material-sweep arm and route by it. Texture sweeps need `SweepApplicability` because several texture features post-date 0.x.

### Task A1: Add `--spec-version` to `emit-sweep` (the basic MToon sweep)

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (variant `EmitSweep` ~line 43; handler ~line 416)
- Test: `crates/vrm-asset-generator/tests/cli_spec_version.rs` (exists from slice 1; append)

- [ ] **Step 1: Write the failing test**

Append to `crates/vrm-asset-generator/tests/cli_spec_version.rs`:

```rust
#[test]
fn emit_sweep_v0_produces_vrm0_assets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sweep0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-sweep",
            "--spec-version",
            "0.x",
            "--output-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run emit-sweep");
    assert!(status.success(), "emit-sweep --spec-version 0.x must exit 0");

    // At least one emitted .vrm must declare the VRM 0.x extension namespace.
    let vrm = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "vrm"))
        .expect("at least one .vrm emitted");
    let bytes = std::fs::read(&vrm).unwrap();
    // GLB JSON chunk starts at byte 20; cheap substring check is enough here.
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("\"VRM\""),
        "0.x asset must carry the bare `VRM` extension, not VRMC_*"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version emit_sweep_v0_produces_vrm0_assets`
Expected: FAIL — `error: unexpected argument '--spec-version'` (the arm doesn't accept the flag yet).

- [ ] **Step 3: Add the `spec_version` field to the `EmitSweep` variant**

In `cli.rs`, change the `EmitSweep` variant (currently):

```rust
    EmitSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// Emit JSON progress on stderr (NDJSON) and a final JSON summary on stdout.
        #[arg(long)]
        json: bool,
    },
```

to:

```rust
    EmitSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        /// Emit JSON progress on stderr (NDJSON) and a final JSON summary on stdout.
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 4: Route the handler by spec_version**

In `cli.rs`, change the `Cmd::EmitSweep` match arm. Bind `spec_version`, and replace the `emit_with_sidecars(p, &stem)?;` call with a version switch. Add `emit_with_sidecars_v0` to the existing `use crate::emit::{...}` import at the top of the file (it already imports `emit_with_sidecars, emit_with_sidecars_v0`).

```rust
        Cmd::EmitSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_basic_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_basic_sweep();
            let total = assets.len();

            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }

                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {} assets to {}", emitted.len(), output_dir);
            }
            Ok(())
        }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version emit_sweep_v0_produces_vrm0_assets`
Expected: PASS.

- [ ] **Step 6: Verify the 1.0 path is unchanged (no regression)**

Run: `cargo test -p vrm-asset-generator`
Expected: PASS (all existing tests; `default_value = "1.0"` preserves prior behavior).

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/tests/cli_spec_version.rs
git commit -m "feat(generator): emit-sweep --spec-version 0.x routes MToon basic sweep through v0 emit"
```

### Task A2: Validator-gate the 0.x basic sweep

**Files:**
- Test: `crates/vrm-asset-generator/tests/cli_spec_version.rs` (append; `#[ignore]` validator-gated)

- [ ] **Step 1: Write the failing (ignored) validator test**

```rust
#[test]
#[ignore = "requires .tools/vrm-validator-cli (scripts/install-validator.sh)"]
fn emit_sweep_v0_assets_pass_validator() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sweep0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(["emit-sweep", "--spec-version", "0.x", "--output-dir", out.to_str().unwrap()])
        .status()
        .expect("run emit-sweep");
    assert!(status.success());

    for entry in std::fs::read_dir(&out).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|x| x == "vrm") {
            let report = vrm_validator_wrap::validate(&p).expect("validator runs");
            assert_eq!(
                report.num_errors, 0,
                "0.x asset {p:?} must validate with 0 errors; got {report:?}"
            );
        }
    }
}
```

NOTE: confirm the `vrm_validator_wrap` public API name at execution (`validate` + the report's `num_errors` field) by reading `crates/vrm-validator-wrap/src/lib.rs`. If the existing v0 validator tests in slice 1 used a different call shape, mirror that shape exactly.

- [ ] **Step 2: Run to verify it is collected but skipped without the validator**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version emit_sweep_v0_assets_pass_validator`
Expected: `1 ignored` (skipped — validator gate).

- [ ] **Step 3: Run with validator installed**

Run: `scripts/install-validator.sh && cargo test -p vrm-asset-generator --test cli_spec_version -- --ignored emit_sweep_v0_assets_pass_validator`
Expected: PASS (0 errors per asset). If the validator reports the inherent 0.x warnings (`UNSUPPORTED_EXTENSION` / `INVALID_EXTENSION_NAME_FORMAT` on the bare `VRM` name), those are warnings, not errors — assert on `num_errors` only.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-asset-generator/tests/cli_spec_version.rs
git commit -m "test(generator): validator-gate the 0.x basic MToon sweep (0 errors)"
```

### Task A3: Per-sweep applicability table for texture sweeps

The texture sweeps cover features whose 0.x availability differs. We do NOT emit a 0.x asset for a v1-only axis — we mark it `NotApplicable` so the symmetry test stays honest and the corpus doesn't claim coverage it lacks.

**Applicability decisions (cite spec; confirm against `docs/upstream-specs/` at execution):**

| Sweep | 0.x status | Reason variant |
|---|---|---|
| `mtoon_basic_sweep` | Applicable (modulo outlineLightingMix) | — (handled in slice 1) |
| `mtoon_emissive_sweep` | Applicable — 0.x MToon has `_EmissionColor` | — |
| `mtoon_shade_multiply_texture_sweep` | Applicable — 0.x `_ShadeTexture` | — |
| `mtoon_shading_shift_texture_sweep` | `NotApplicable` — shadingShiftTexture is MToon-1.0-only (0.x had scalar shift only) | new `ShadingShiftTextureV1Only` |
| `mtoon_matcap_texture_sweep` | Applicable — 0.x `_SphereAdd` matcap | — |
| `mtoon_rim_multiply_texture_sweep` | `NotApplicable` — rimMultiplyTexture is 1.0-only | new `RimMultiplyTextureV1Only` |
| `mtoon_outline_width_multiply_texture_sweep` | Applicable — 0.x `_OutlineWidthTexture` | — |
| `mtoon_texture_transform_sweep` | `NotApplicable` — `KHR_texture_transform` not used by VRM 0.x exporters | new `KhrTextureTransformV1Only` |
| `mtoon_pbr_textures_sweep` | Applicable — glTF-core normal/occlusion exist in 0.x | — |
| `mtoon_first_person_sweep` | Applicable — 0.x `firstPerson.meshAnnotations` | — |

**Files:**
- Modify: `crates/vrm-asset-generator/src/lib.rs` (`NotApplicableReason` enum ~line 35 — add 3 variants)
- Test: `crates/vrm-asset-generator/src/lib.rs` (serde round-trip test module already present ~line 45)

- [ ] **Step 1: Write the failing test for the new reasons**

Append to the `lib.rs` test module:

```rust
#[test]
fn new_v1_only_reasons_serialize() {
    for r in [
        NotApplicableReason::ShadingShiftTextureV1Only,
        NotApplicableReason::RimMultiplyTextureV1Only,
        NotApplicableReason::KhrTextureTransformV1Only,
    ] {
        let s = serde_json::to_string(&r).unwrap();
        let back: NotApplicableReason = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator --lib new_v1_only_reasons_serialize`
Expected: FAIL — variants don't exist (compile error).

- [ ] **Step 3: Add the variants**

In `lib.rs`, extend `NotApplicableReason` (currently holds `OutlineLightingMixV1Only` plus the slice-1 set). Add:

```rust
    ShadingShiftTextureV1Only,
    RimMultiplyTextureV1Only,
    KhrTextureTransformV1Only,
```

Confirm the enum has `#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]` and `#[serde(rename_all = ...)]` matching the existing variants — mirror exactly, don't re-derive differently.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator --lib new_v1_only_reasons_serialize`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/lib.rs
git commit -m "feat(generator): add 3 v1-only NotApplicableReason variants for 0.x texture sweeps"
```

### Task A4: Add `--spec-version` to the Applicable texture/material sweeps

Apply the **exact same pattern as Task A1, Steps 3–4** to each of these Applicable arms. For each: add the `spec_version` field to the variant, bind it in the handler, and replace `emit_with_sidecars(p, &stem)?;` with the `match spec_version { V0 => emit_with_sidecars_v0(p, &stem)?, V1 => emit_with_sidecars(p, &stem)? }` block.

**Apply to (Applicable):**
- `EmitEmissiveSweep`
- `EmitShadeMultiplyTextureSweep`
- `EmitMatcapTextureSweep`
- `EmitOutlineWidthMultiplyTextureSweep`
- `EmitPbrTexturesSweep`
- `EmitFirstPersonSweep`

**Do NOT add `--spec-version` to (NotApplicable):** `EmitShadingShiftTextureSweep`, `EmitRimMultiplyTextureSweep`, `EmitTextureTransformSweep`. Instead, when `--spec-version 0.x` is *requested* for these, they must refuse cleanly (next task).

- [ ] **Step 1: Write one failing test per Applicable sweep (parameterized)**

Append to `tests/cli_spec_version.rs`:

```rust
#[test]
fn applicable_texture_sweeps_emit_v0() {
    for sub in [
        "emit-emissive-sweep",
        "emit-shade-multiply-texture-sweep",
        "emit-matcap-texture-sweep",
        "emit-outline-width-multiply-texture-sweep",
        "emit-pbr-textures-sweep",
        "emit-first-person-sweep",
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = tmp.path().join(sub);
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
            .args([sub, "--spec-version", "0.x", "--output-dir", out.to_str().unwrap()])
            .status()
            .expect("run sweep");
        assert!(status.success(), "{sub} --spec-version 0.x must exit 0");
        let has_vrm = std::fs::read_dir(&out)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().is_some_and(|x| x == "vrm"));
        assert!(has_vrm, "{sub} must emit at least one .vrm at 0.x");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version applicable_texture_sweeps_emit_v0`
Expected: FAIL — first sub errors on unknown `--spec-version`.

- [ ] **Step 3: Apply the Task-A1 pattern to all six Applicable arms**

Edit each of the six variants + handlers in `cli.rs` exactly as in Task A1 Steps 3–4.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version applicable_texture_sweeps_emit_v0`
Expected: PASS.

- [ ] **Step 5: Run full crate tests (no regression)**

Run: `cargo test -p vrm-asset-generator`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/tests/cli_spec_version.rs
git commit -m "feat(generator): --spec-version 0.x on the six applicable MToon texture/material sweeps"
```

### Task A5: NotApplicable sweeps reject `--spec-version 0.x` cleanly

For `emit-shading-shift-texture-sweep`, `emit-rim-multiply-texture-sweep`, `emit-texture-transform-sweep`: add the `spec_version` arg, but error with a structured message when `0.x` is requested, so a corpus driver gets a clear signal rather than a wrong asset.

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (the three NotApplicable variants + handlers)
- Test: `tests/cli_spec_version.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn notapplicable_sweeps_reject_v0() {
    for (sub, reason) in [
        ("emit-shading-shift-texture-sweep", "ShadingShiftTextureV1Only"),
        ("emit-rim-multiply-texture-sweep", "RimMultiplyTextureV1Only"),
        ("emit-texture-transform-sweep", "KhrTextureTransformV1Only"),
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = tmp.path().join(sub);
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
            .args([sub, "--spec-version", "0.x", "--output-dir", out.to_str().unwrap()])
            .output()
            .expect("run sweep");
        assert!(!output.status.success(), "{sub} must reject --spec-version 0.x");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(reason),
            "{sub} rejection must name the structured reason {reason}; got: {stderr}"
        );
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version notapplicable_sweeps_reject_v0`
Expected: FAIL — flag unknown.

- [ ] **Step 3: Add `spec_version` + the guard to each of the three handlers**

Add the `spec_version` field to each variant (same as A1 Step 3). In each handler, before the emit loop:

```rust
            if spec_version == vrm_ops::SpecVersion::V0 {
                anyhow::bail!(
                    "emit-shading-shift-texture-sweep has no VRM 0.x form: \
                     NotApplicableReason::ShadingShiftTextureV1Only"
                );
            }
```

(use the matching sweep name + reason variant in each of the three.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version notapplicable_sweeps_reject_v0`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/tests/cli_spec_version.rs
git commit -m "feat(generator): v1-only texture sweeps reject --spec-version 0.x with structured reason"
```

---

## Phase B — VRM 0.x spring-bone emit (`spring_bone_v0.rs`)

This is the load-bearing new module. VRM 1.0 uses `VRMC_springBone` (colliders, collider groups, joints with per-joint params). VRM 0.x uses `secondaryAnimation` with `boneGroups` + `colliderGroups`, different field names and a flatter model. The existing `spring_bone.rs` builds the 1.0 form; `spring_bone_v0.rs` builds the 0.x form from the **same `SpringBoneParams`** so the sweep registries are shared.

**Spec reference:** VRM 0.0 `secondaryAnimation` schema — `docs/upstream-specs/vrm-specification/specification/0.0/README.md` (and the `vrm.schema.json` under that tree). Read it at execution; key 0.x shape: `secondaryAnimation.boneGroups[]` each with `comment, stiffiness` (note the spec's typo `stiffiness`), `gravityPower, gravityDir{x,y,z}, dragForce, center, hitRadius, bones[], colliderGroups[]`.

### Task B1: `secondaryAnimation` JSON builder for a single chain

**Files:**
- Create: `crates/vrm-asset-generator/src/spring_bone_v0.rs`
- Modify: `crates/vrm-asset-generator/src/lib.rs` (add `mod spring_bone_v0;`)
- Test: in `spring_bone_v0.rs` (`#[cfg(test)]` module)

- [ ] **Step 1: Write the failing test**

Create `spring_bone_v0.rs` with only:

```rust
//! VRM 0.x `secondaryAnimation` emit. Mirrors `spring_bone.rs`'s topology but
//! produces the 0.0 schema (boneGroups + colliderGroups) instead of
//! VRMC_springBone. Shares `SpringBoneParams` so the sweep registries are
//! version-agnostic.

use crate::spring_bone::SpringBoneParams;
use serde_json::{json, Value};

/// Build the VRM 0.x `secondaryAnimation` object for one spring chain.
/// `first_bone_node` is the glTF node index the chain hangs from.
pub fn build_secondary_animation(params: &SpringBoneParams, first_bone_node: usize) -> Value {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spring_bone::SpringBoneParams;

    #[test]
    fn single_chain_has_one_bone_group_with_spec_field_names() {
        let p = SpringBoneParams::defaults("sb_v0_smoke");
        let sa = build_secondary_animation(&p, 1);
        let groups = sa["boneGroups"].as_array().expect("boneGroups array");
        assert_eq!(groups.len(), 1, "one chain → one boneGroup");
        let g = &groups[0];
        // VRM 0.x uses the spec's `stiffiness` spelling (the famous typo).
        assert!(g.get("stiffiness").is_some(), "must use 0.x `stiffiness` key");
        assert!(g.get("dragForce").is_some());
        assert!(g.get("gravityPower").is_some());
        assert!(g["bones"].as_array().is_some(), "bones index array present");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator --lib spring_bone_v0`
Expected: FAIL — `todo!()` panics / `mod spring_bone_v0` not declared. (Add `mod spring_bone_v0;` to `lib.rs` first if the compile error is "unresolved module".)

- [ ] **Step 3: Implement `build_secondary_animation`**

Read `SpringBoneParams` fields from `src/spring_bone.rs` first. Map them to the 0.x schema (field names from the 0.0 spec — confirm):

```rust
pub fn build_secondary_animation(params: &SpringBoneParams, first_bone_node: usize) -> Value {
    json!({
        "boneGroups": [{
            "comment": params.spring_name,
            "stiffiness": params.stiffness,       // 0.x typo spelling, intentional
            "gravityPower": params.gravity_power,
            "gravityDir": {
                "x": params.gravity_dir[0],
                "y": params.gravity_dir[1],
                "z": params.gravity_dir[2]
            },
            "dragForce": params.drag,
            "center": -1,
            "hitRadius": params.hit_radius,
            "bones": [first_bone_node],
            "colliderGroups": []
        }],
        "colliderGroups": []
    })
}
```

ADAPT the field accessors (`params.stiffness`, `params.gravity_power`, `params.drag`, `params.gravity_dir`, `params.hit_radius`, `params.spring_name`) to the real `SpringBoneParams` field names — the test's `assert!(g.get("stiffiness").is_some())` checks the *output* key, not the input field, so the input names must be read from source.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator --lib spring_bone_v0`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/spring_bone_v0.rs crates/vrm-asset-generator/src/lib.rs
git commit -m "feat(generator): spring_bone_v0 — VRM 0.x secondaryAnimation builder for a single chain"
```

### Task B2: Wire `secondaryAnimation` into the v0 `VRM` extension block

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrm_ext_v0.rs` (the `VRM` extension assembler)
- Test: `vrm_ext_v0.rs` `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Read `vrm_ext_v0.rs` to find the function that assembles the `VRM` extension object (slice 1 emits `meta` + `materialProperties` + `blendShapeMaster` + an empty `secondaryAnimation`). Add a test that a provided `secondaryAnimation` lands in the block:

```rust
#[test]
fn vrm_ext_carries_provided_secondary_animation() {
    let sa = serde_json::json!({"boneGroups": [{"comment": "x"}], "colliderGroups": []});
    let ext = build_vrm_extension_with_secondary(/* minimal args */, Some(sa.clone()));
    assert_eq!(ext["secondaryAnimation"]["boneGroups"][0]["comment"], "x");
}
```

Adapt the call to the real assembler signature in `vrm_ext_v0.rs`. If the assembler currently hardcodes an empty `secondaryAnimation`, the minimal change is to accept an `Option<Value>` and use it when `Some`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator --lib vrm_ext_carries_provided_secondary_animation`
Expected: FAIL.

- [ ] **Step 3: Thread the optional `secondaryAnimation` through the assembler**

Modify the v0 `VRM` extension builder to accept `secondary_animation: Option<Value>` and substitute it for the empty default when present. Keep the empty default for the material-only path (so Phase A's MToon-only 0.x assets are unchanged).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator --lib vrm_ext_carries_provided_secondary_animation`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/vrm_ext_v0.rs
git commit -m "feat(generator): v0 VRM extension block accepts a secondaryAnimation payload"
```

### Task B3: `emit_with_sidecars_spring_bone_v0` (settle) end-to-end

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (new fn near `emit_with_sidecars_spring_bone` at line 826)
- Test: `tests/cli_spec_version.rs` (validator-gated) + a unit test in `emit.rs`

- [ ] **Step 1: Write the failing test**

In `emit.rs` tests (or a new integration test), assert the function emits a triplet whose `.vrm` carries a non-empty `secondaryAnimation`:

```rust
#[test]
fn emit_spring_bone_v0_writes_secondary_animation() {
    let tmp = tempfile::tempdir().unwrap();
    let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("sb_v0")).unwrap();
    let mtoon = crate::params::MToonParams::defaults("sb_v0");
    let spring = crate::spring_bone::SpringBoneParams::defaults("sb_v0");
    emit_with_sidecars_spring_bone_v0(&mtoon, &spring, &stem).unwrap();
    let bytes = std::fs::read(stem.with_extension("vrm")).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("secondaryAnimation"));
    assert!(text.contains("boneGroups"));
    assert!(text.contains("stiffiness"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator emit_spring_bone_v0_writes_secondary_animation`
Expected: FAIL — fn doesn't exist.

- [ ] **Step 3: Implement `emit_with_sidecars_spring_bone_v0`**

Mirror `emit_with_sidecars_spring_bone` (emit.rs:826) but: build the chain mesh/nodes the same way, compute the chain's first-bone node index, call `spring_bone_v0::build_secondary_animation(spring, first_bone_node)`, route the material through `mtoon_v0::emit_material_property`, and assemble the `VRM` extension via the Task-B2 path. The 0.x test plan sidecar (`.test.yaml`) must set `spec_version: "0.x"` and the **-Z camera** convention (mirror what `emit_with_sidecars_v0` writes for the MToon case). Reuse the existing chain-geometry helper the 1.0 path uses — do not fork the mesh builder.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator emit_spring_bone_v0_writes_secondary_animation`
Expected: PASS.

- [ ] **Step 5: Add a validator-gated assertion**

Append an `#[ignore]` test that runs the emitted `.vrm` through `vrm_validator_wrap` and asserts `num_errors == 0` (same shape as Task A2).

- [ ] **Step 6: Run validator-gated**

Run: `cargo test -p vrm-asset-generator -- --ignored emit_spring_bone_v0`
Expected: PASS (0 errors) with validator installed.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/tests/cli_spec_version.rs
git commit -m "feat(generator): emit_with_sidecars_spring_bone_v0 — full 0.x spring-bone triplet"
```

### Task B4: Swing variant `emit_with_sidecars_spring_bone_v0_swing`

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Test: `emit.rs` unit test

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn emit_spring_bone_v0_swing_has_animation_block() {
    let tmp = tempfile::tempdir().unwrap();
    let stem = camino::Utf8PathBuf::from_path_buf(tmp.path().join("swing_sb_v0")).unwrap();
    let mtoon = crate::params::MToonParams::defaults("swing_sb_v0");
    let spring = crate::spring_bone::SpringBoneParams::defaults("swing_sb_v0");
    emit_with_sidecars_spring_bone_v0_swing(&mtoon, &spring, &stem).unwrap();
    let yaml = std::fs::read_to_string(stem.with_extension("test.yaml")).unwrap();
    assert!(yaml.contains("animation"), "swing plan must carry animate_root_transform");
    assert!(yaml.contains("spec_version: \"0.x\"") || yaml.contains("spec_version: 0.x"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator emit_spring_bone_v0_swing_has_animation_block`
Expected: FAIL.

- [ ] **Step 3: Implement the swing variant**

Mirror `emit_with_sidecars_spring_bone_swing` (emit.rs:854) — identical asset, but the `.test.yaml` carries the `animation.root_transform` block. Reuse the Task-B3 emit for the `.vrm`; only the test-plan sidecar differs.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator emit_spring_bone_v0_swing_has_animation_block`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(generator): emit_with_sidecars_spring_bone_v0_swing — 0.x swing spring-bone"
```

---

## Phase C — Spring-bone sweeps at 0.x

Now route the spring-bone sweep arms through the Phase-B emit path. The sweep registries (`spring_bone_basic_sweep()`, etc., returning `Vec<SpringBoneParams>`) are unchanged — same params, different emit fn.

### Task C1: `emit-springbone-sweep --spec-version 0.x`

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (`EmitSpringboneSweep` variant ~line 186; handler ~line 862)
- Test: `tests/cli_spec_version.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn springbone_sweep_v0_emits_secondary_animation() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sb0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(["emit-springbone-sweep", "--spec-version", "0.x", "--output-dir", out.to_str().unwrap()])
        .status().expect("run");
    assert!(status.success());
    let any_sa = std::fs::read_dir(&out).unwrap().filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "vrm"))
        .any(|e| String::from_utf8_lossy(&std::fs::read(e.path()).unwrap()).contains("secondaryAnimation"));
    assert!(any_sa, "0.x spring-bone sweep assets must carry secondaryAnimation");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version springbone_sweep_v0_emits_secondary_animation`
Expected: FAIL — flag unknown.

- [ ] **Step 3: Add `spec_version` + route the handler**

Add the field to `EmitSpringboneSweep`; in the handler, replace `emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?;` with:

```rust
                match spec_version {
                    vrm_ops::SpecVersion::V0 => {
                        emit_with_sidecars_spring_bone_v0(&mtoon, spring, &stem)?
                    }
                    vrm_ops::SpecVersion::V1 => {
                        emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?
                    }
                }
```

Add `emit_with_sidecars_spring_bone_v0` to the handler's `use crate::emit::{...}` line.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vrm-asset-generator --test cli_spec_version springbone_sweep_v0_emits_secondary_animation`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs crates/vrm-asset-generator/tests/cli_spec_version.rs
git commit -m "feat(generator): emit-springbone-sweep --spec-version 0.x via secondaryAnimation"
```

### Task C2: `emit-springbone-swing-sweep --spec-version 0.x`

**Files:**
- Modify: `cli.rs` (`EmitSpringboneSwingSweep` variant ~line 208; handler ~line 952)
- Test: `tests/cli_spec_version.rs`

- [ ] **Step 1: Write the failing test** (same shape as C1 but `emit-springbone-swing-sweep`, and assert the emitted `.test.yaml` files contain `animation`).
- [ ] **Step 2: Run — expect FAIL** (`cargo test -p vrm-asset-generator --test cli_spec_version springbone_swing_sweep_v0`).
- [ ] **Step 3: Add `spec_version` + route** the handler to `emit_with_sidecars_spring_bone_v0_swing` for `V0`, existing fn for `V1`.
- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Commit** `feat(generator): emit-springbone-swing-sweep --spec-version 0.x`.

### Task C3: Collider sweeps — applicability decision + arms

VRM 0.x supports **sphere colliders only** (capsule colliders are a VRM-1.0 addition; `extended_collider` is 1.0-only). So:
- `emit-springbone-collider-sweep`: **partially applicable** — emit only the sphere-collider cells at 0.x; mark capsule cells `NotApplicable { CapsuleColliderV1Only }`.
- `emit-springbone-extended-sweep`: **fully NotApplicable** at 0.x — reject `--spec-version 0.x` (`ExtendedColliderV1Only`).

**Files:**
- Modify: `src/lib.rs` (add `CapsuleColliderV1Only`, `ExtendedColliderV1Only` if not present)
- Modify: `src/cli.rs` (both arms)
- Modify: `src/spring_bone_v0.rs` (extend `build_secondary_animation` to accept sphere colliders)
- Test: `tests/cli_spec_version.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn extended_collider_sweep_rejects_v0() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ext0x");
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(["emit-springbone-extended-sweep", "--spec-version", "0.x", "--output-dir", out.to_str().unwrap()])
        .output().unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("ExtendedColliderV1Only"));
}

#[test]
fn collider_sweep_v0_emits_only_sphere_cells() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("coll0x");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args(["emit-springbone-collider-sweep", "--spec-version", "0.x", "--output-dir", out.to_str().unwrap()])
        .status().unwrap();
    assert!(status.success());
    // No emitted asset id should contain "capsule".
    let any_capsule = std::fs::read_dir(&out).unwrap().filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().contains("capsule"));
    assert!(!any_capsule, "0.x collider sweep must skip capsule cells");
}
```

- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3a:** Add the two `NotApplicableReason` variants (mirror Task A3).
- [ ] **Step 3b:** `EmitSpringboneExtendedSweep`: add `spec_version`, `anyhow::bail!` with `ExtendedColliderV1Only` when `V0`.
- [ ] **Step 3c:** `EmitSpringboneColliderSweep`: add `spec_version`; when `V0`, `continue` past any variant whose collider shape is capsule (filter on the variant's shape field — read `spring_bone_collider_sweep()` to find how shape is represented), and emit sphere cells via a collider-aware `secondaryAnimation`. Extend `spring_bone_v0::build_secondary_animation` to take an optional sphere-collider list and emit `colliderGroups[].colliders[].{offset,radius}` (0.x sphere collider schema).
- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Run full crate tests** (`cargo test -p vrm-asset-generator`) — expect PASS.
- [ ] **Step 6: Commit** `feat(generator): 0.x collider sweep (sphere-only); extended-collider rejects 0.x`.

### Task C4: Remaining spring-bone sweeps (gravity-dir, taper, multichain, coupling, sequence)

Decisions:
- `emit-springbone-gravity-dir-sweep`: **Applicable** — 0.x `gravityDir` exists. Route both settle + swing arms.
- `emit-springbone-coupling-sweep`: **Applicable** — exercises `gravityPower` parsing; route settle + swing.
- `emit-springbone-multichain-sweep`: **Applicable** — multiple `boneGroups`. Extend `build_secondary_animation` to take N chains, OR add `build_secondary_animation_multi`.
- `emit-springbone-taper-sweep`: **NotApplicable** — per-joint stiffness/drag vectors are a VRM-1.0 feature; 0.x `boneGroups` carry a single scalar per group. Reject with `PerJointStiffnessV1Only` (confirm this variant exists from slice 1; design lists it).
- `emit-sequence-sweep`: **Applicable** — `render_sequence` is renderer-side; the asset is just a 0.x spring-bone. Route to a sequence-mode v0 emit (mirror `emit_with_sidecars_spring_bone_swing_sequence` at emit.rs:880 over the v0 emit).

- [ ] **Step 1: Write failing tests** — one per sweep: Applicable ones assert `secondaryAnimation` present at 0.x; taper asserts rejection with `PerJointStiffnessV1Only`. (Same shapes as C1/C3.)
- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3:** Implement: add `spec_version` to all five arms; route Applicable ones to the v0 emit (adding `build_secondary_animation_multi` for multichain and a v0 sequence emit fn in emit.rs for sequence); `bail!` for taper.
- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Commit** `feat(generator): remaining spring-bone sweeps at 0.x (taper NotApplicable)`.

---

## Phase D — Corpus render, methodology, findings

### Task D1: Extend the symmetry test to the new sweeps

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs` (`registry_symmetry_tests` module ~line 2140)

- [ ] **Step 1: Write the failing test**

Add a test that every sweep with a 0.x form has either a 1.0 counterpart or a registered `NotApplicable` reason, covering the new sweeps:

```rust
#[test]
fn all_v0_capable_sweeps_have_counterpart_or_reason() {
    // Enumerate (sweep_name, applicability) for every sweep that Phase A–C
    // touched; assert Applicable ⇒ 1.0 counterpart exists, NotApplicable ⇒
    // reason recorded. Mirror the existing single-sweep symmetry assertion.
    // ... explicit table of the 20 sweeps with their decided applicability ...
}
```

Build the explicit table from the Phase A–C decisions (Applicable set vs the five NotApplicable: shadingShiftTexture, rimMultiplyTexture, textureTransform, extendedCollider, taper).

- [ ] **Step 2: Run — expect FAIL** (table references not yet asserted).
- [ ] **Step 3:** Implement the assertion using the existing `mtoon_basic_v0_sweep`-style counterpart-id check.
- [ ] **Step 4: Run — expect PASS** (`cargo test -p vrm-asset-generator --lib symmetric`).
- [ ] **Step 5:** Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`; fix any findings.
- [ ] **Step 6: Commit** `test(generator): symmetry assertion covers all 0.x-capable sweeps`.

### Task D2: Bootstrap a 0.x corpus staging dir

**Files:**
- Modify: `scripts/bootstrap-goldens.sh`

- [ ] **Step 1:** Add a `SPEC_VERSION` env knob (default `1.0`). When `SPEC_VERSION=0.x`, the sweep-emit block passes `--spec-version 0.x` to every Applicable sweep and skips the NotApplicable ones, writing into a `…_v0` staging dir. Reuse the existing emit invocations; append the flag conditionally.

- [ ] **Step 2: Smoke it (mock renderer, no GPU)**

Run a minimal manual check:
```bash
target/release/vrm-asset-generator emit-sweep --spec-version 0.x --output-dir /tmp/c0x
# pick one asset, confirm it carries VRM 0.x:
python3 - <<'PY'
import glob, struct, json
p = sorted(glob.glob('/tmp/c0x/*.vrm'))[0]
d = open(p,'rb').read(); off=12; clen,_=struct.unpack_from('<II',d,off)
j = json.loads(d[20:20+clen])
assert 'VRM' in j.get('extensions',{}), j.get('extensionsUsed')
print('OK 0.x:', p)
PY
```
Expected: `OK 0.x: ...`.

- [ ] **Step 3: Commit** `feat(bootstrap): SPEC_VERSION=0.x knob emits the 0.x sweep corpus`.

### Task D3: Methodology pin — spring-bone triage order

**Files:**
- Modify: `docs/methodology.md`

- [ ] **Step 1:** Add a section "Spring-bone cross-version triage order (reversed)" per design lines 251–257: for spring-bone sweeps, read **within-renderer cross-version first** (VMK 0.x vs VMK 1.0 on the same axis) because the simulation is integrator-sensitive (Verlet vs semi-implicit Euler, sub-stepping, damping) — a within-renderer cross-version disagreement isolates a coordinate/unit bug in one emit path, whereas cross-renderer noise on the same version may just be integrator variance. Cite the design doc.
- [ ] **Step 2: Commit** `docs(methodology): spring-bone cross-version triage-order pin`.

### Task D4: End-to-end 0.x corpus render + findings (execution-time, real adapters)

This task runs on the user's machine with real adapters built; it produces the deliverable signal.

- [ ] **Step 1:** Generate the 0.x corpus (`SPEC_VERSION=0.x scripts/bootstrap-goldens.sh` or the staged emit dir).
- [ ] **Step 2:** Render the 0.x sweep corpus through three-vrm + VMK + UniVRM (+ godot if available), using `execute-test-plan` (per-op adapters) and `execute-test-batch` (UniVRM), exactly as slice 1's humanoid render did.
- [ ] **Step 3:** `consensus-diff` each variant. For MToon material sweeps, the question is whether 0.x-delivered material params match 1.0 (within-version cross-renderer read). For spring-bone, apply the D3 triage order (within-renderer cross-version first).
- [ ] **Step 4:** Append a `docs/findings.md` entry: which 0.x sweep variants diverge from their 1.0 counterparts and which renderers cluster. Note the methodology caveat that the sweep assets are spheres, so the 180° orientation flip (VMK#299) does NOT confound material/physics comparison — this slice isolates MToon-math and spring-bone-physics-via-0.x-extension behavior, distinct from the humanoid orientation finding.
- [ ] **Step 5:** Commit the findings entry; do NOT commit `goldens-cache/` PNGs (gitignored; goldens go to S3 per the manifest trust model).

---

## Self-Review

**Spec coverage (design "Slice 2" lines 91–96):**
- "Full 0.x MToon sweep parity (~44 variants)" → Phase A (Tasks A1, A4) routes all Applicable MToon sweeps; A3/A5 handle the v1-only axes as NotApplicable. ✅
- "Spring-bone v0 sweep (~18 variants) — gravity, drag, joint count, sphere radius" → Phase B builds `spring_bone_v0.rs`; Phase C routes basic/gravity-dir/coupling/multichain/sequence sweeps; sphere colliders in C3. ✅
- "`_v0_quirk_*` family first wave (`stiffinessForce` typo, single-bone-per-group, sphere-collider-only)" → PARTIAL: the `stiffiness` typo is honored in B1; sphere-collider-only enforcement is C3; single-bone-per-group and the dedicated `_v0_quirk_*` named family are **not** in this plan. *Gap noted below.*
- "Methodology doc: spring-bone triage-order pin" → Task D3. ✅
- Sweep registry symmetry assertion (cross-slice invariant) → Task D1. ✅

**Identified gap (deliberate, flagged for approval):** the dedicated `_v0_quirk_*` sweep family (design line 95) is only partially covered — the `stiffiness` typo and sphere-only enforcement fall out of the main sweeps, but `springbone_singleton_groups_v0_quirk` and centerNode-as-transform quirks are not separate tasks. Recommend either (a) adding a Phase E "quirk sweeps" once the main corpus renders, or (b) deferring the named quirk family to a slice-2b. This keeps slice 2's core deliverable — full sweep parity at 0.x — unblocked. **Decide at approval.**

**Placeholder scan:** no "TBD"/"add error handling" placeholders. The one intentional deferral is the field-name confirmation note (struct fields must be read from live source) — this is a transport-reliability hedge, not a content gap; the TDD compile loop enforces correctness.

**Type consistency:** emit fn names are consistent — `emit_with_sidecars_v0` (existing), `emit_with_sidecars_spring_bone_v0` (B3), `emit_with_sidecars_spring_bone_v0_swing` (B4); `build_secondary_animation` (B1) extended in C3/C4. `NotApplicableReason` new variants (`ShadingShiftTextureV1Only`, `RimMultiplyTextureV1Only`, `KhrTextureTransformV1Only`, `CapsuleColliderV1Only`, `ExtendedColliderV1Only`) referenced consistently across A3, A5, C3.

**Constraint honored:** every task is in `vrm-conformance` (generator/scripts/docs). No task touches VRMMetalKit — consistent with the report-and-find boundary; the VMK orientation fix is tracked separately at VMK#299.
