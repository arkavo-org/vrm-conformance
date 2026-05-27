# VRM 0.x conformance — design

**Date:** 2026-05-26
**Status:** Approved
**Author:** drafted via brainstorming session
**Supersedes:** [RFC 0006 — VRM 0.x conformance coverage](../../../rfcs/0006-vrm-0x-conformance.md) (scope sketch → actionable design)
**Spec references:**
- [VRM 0.x specification](https://github.com/vrm-c/vrm-specification/tree/master/specification/0.0)
- [VRM 1.0 specification](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm-1.0)
- Local mirrors: `docs/upstream-specs/vrm-specification/specification/{0.0,VRMC_vrm-1.0}/`

## Purpose

Add first-class VRM 0.x conformance coverage alongside the existing VRM 1.0 corpus across asset generation, test plans, runner ops, four-adapter coverage, and the published comparison site. Three independent signals make this real:

1. **Asset corpus reality.** The fixture `avatarA_0_0.vrm` exists in `assets/humanoid/` but is exercised by zero test plans; downstream goal of "VRoid Hub avatar imported and renders correctly" is incomplete without 0.x coverage (most pre-2022 VRoid Hub content is 0.x).
2. **Adapter coverage matrix is lopsided.** VMK, three-vrm, godot-vrm load `avatarA_0_0.vrm` cleanly; UniVRM rejects it with `Failed to load as VRM 1.0` because the adapter passes `canLoadVrm0X: false`. One-line adapter fix unblocks UniVRM.
3. **Cross-spec-version orientation behavior diverges** in ways the corpus is currently blind to. VMK applies a non-spec 180° flip on 0.x avatars; three-vrm and godot-vrm are spec-correct; UniVRM has an adapter coord-handling bug. This is exactly the kind of signal a conformance suite exists to surface.

## Methodology stance

**Conformance suites test the spec.** Where adapters diverge from spec, the corpus flags failures — including against our own renderer (VMK). The visible cross-renderer SSIM divergence in slice 1's published output **is the deliverable**, not a problem to soften. Methodology documents *why* the pin exists (spec citation), *what* the failure modes look like (back-view example image), and *how* to read the consensus diff. Methodology supplements failure flags; it does not replace them.

## Architecture overview

Single `vrm-asset-generator` crate gains a `--spec-version 0.x | 1.0` flag (default `1.0` preserves existing behavior). A `SpecVersion::{V0, V1}` enum at the crate root of `vrm-ops` threads through every layer — generator CLI, manifest schema, test plan metadata, runner dispatch, adapter contract. No string compares anywhere downstream of the parse boundary.

Read-side ops (`dump_humanoid_pose`, `dump_expression_weights`, `dump_look_at_state`) gain an optional `as_spec_version` enum param. Default is **native** — return the asset's parsed shape verbatim, never normalize unless asked. Every dump response carries a required `source_spec_version` field echoing what the adapter actually parsed. Normalization is one-directional (v0→v1 only) and lives in a new `vrm-normalize` crate called by the **runner**, not by adapters — four adapter implementations of normalization would be four bug surfaces.

Write/control ops (`set_expression_weight`, etc.) do **not** gain spec-version params. Normalization is a view concern, not a control concern.

Manifest gains a required `spec_version` field (`"0.x"` or `"1.0"`). Runner enforces test-plan ↔ manifest agreement as a hard error. Camera convention is per-spec-version per spec citation:
- **VRM 0.x:** camera at -Z (avatar faces -Z per `specification/0.0/README.md:238`).
- **VRM 1.0:** camera at +Z (avatar faces +Z per `specification/VRMC_vrm-1.0/tpose.md` Definition 1.1).

Sweep registry gains a `SweepApplicability::{Applicable, NotApplicable{reason}}` enum. Reasons are structured variants (`PerJointStiffnessV1Only`, `CapsuleColliderV1Only`, `OutlineLightingMixV1Only`, etc.) — never free text — so the Critic role can reason about absence. Compile-time assertion enforces sweep-ID symmetry across versions.

## Scope

V1 covers **full parametric parity** with VRM 1.0:

- Tier 2 canonical: `vroid_default_F_0_0` (re-exported from VRoid Studio's 0.x path) + the existing `avatarA_0_0.vrm` fixture.
- Tier 1 parametric: 0.x MToon sweep parity, spring-bone v0 (`secondaryAnimation`) parametric sweep, expressions (`blendShapeMaster`) sweep, VRMA × 0.x sweep, `render_sequence` 0.x sweep.
- 0.x-specific quirk-sweep families (`_v0_quirk_*`) testing spec corners adapters often "silently correct" — `stiffinessForce` typo, centerNode-as-transform vs ignored, single-bone-per-group, sphere-collider-only enforcement, 0.x meta schema.

Out of scope:
- Round-tripping (parse 0.x assets, not just emit). Trigger to revisit the single-crate decision if this becomes a goal.
- Side-channel "native orientation" renders as supplementary artifacts. Deferred to v2 — the conformance failure (back-view image) is already legibly diagnostic.
- VRM 1.1 plumbing — `SpecVersion` enum extends cleanly when 1.1 lands; not gated on this work.

## Rollout — four vertical slices

Each slice produces external-visible signal at its end. Cross-cutting contracts land in slice 1; slices 2–4 are mostly emit-side and adapter-validation work.

**Implementation-plan scope:** this design's rollout spans four slices. Implementation plans are scoped **one slice at a time** — slice 1 plan first; slices 2–4 each get their own plan written after their predecessor's end-of-slice retrospective, so unknowns surfaced by earlier slices feed forward.

### Slice 1 — Foundation + first signal (~3 weeks)

**Goal:** Four-adapter conformance signal for VRM 0.x on a thin asset surface, validating every architecturally novel contract before they're set in stone.

**Asset surface (chosen to exercise every novel contract, not just visual diff):**

| Asset / sweep | Purpose |
|---|---|
| `vroid_default_F_0_0` | Tier 2 canonical (re-export from VRoid Studio 0.x path) |
| `avatarA_0_0` | Existing fixture finally gets a real test plan |
| `mtoon_basic_v0` (3 variants) | Mirror 3 entries from `mtoon_basic_sweep`; **one variant exercises a v1-only axis** as a real `NotApplicable { reason: OutlineLightingMixV1Only }` |
| `expressions_preset_basic_v0` + matching v1 (2 variants each) | Canonical normalization case using `blendShapeMaster.{joy, neutral}` (v0) vs `VRMC_vrm.expressions.preset.{happy, neutral}` (v1). Smallest test that validates `vrm-normalize`, `source_spec_version`, `as_spec_version`, and the cross-renderer round-trip property |

**Internal sequence:**

| Days | Work | Gate |
|---|---|---|
| 1–3 | `SpecVersion` enum; manifest schema with `spec_version` field + migration backfill of 1.0 entries; `vrm-normalize` crate skeleton; `SweepApplicability` enum; **empirical checks** (VRoid Studio 2.12.0 0.x export path; VMK 180° flip location in VRMMetalKit-pinned revision; `mrxz/vrm-validator` 0.x coverage in `vrm-validator-wrap`) | Manifest schema committed before any v0 emit work begins |
| 4–9 | v0 generator emit (`vrm_ext_v0.rs`, `mtoon_v0.rs`, `expressions_v0.rs`); shared math extraction to `mtoon_common.rs`; sweep registry symmetry assertion test | v0 assets load through `vrm-validator-wrap` (or documented fallback) |
| 10 | **Mid-slice checkpoint** — three-vrm + VMK produce renders; first two-adapter diff with VMK 180° flip flagged | Review before continuing to UniVRM/godot |
| 11–17 | UniVRM `canLoadVrm0X: true` fix + coord-handling repro through corpus; godot-vrm wiring; runner `as_spec_version` param + dump-op `source_spec_version` echo | Four-adapter renders produced |
| 18–20 | `vrm-normalize` v0→v1 expression preset mapping; cross-renderer round-trip property test in CI; methodology doc section; site `spec_version` filter chip + badge | Failing-on-purpose entries triage cleanly |
| 21 | **End-of-slice checkpoint** — site deployed; methodology page live; announcement-ready | External announcement (Frans / 0b5vr / Lyuma) |

**Adapter ordering:** three-vrm (expected-clean) + VMK (expected-divergent) first, so the mid-slice diff has both consensus and a credible failure flag on day one of diff. UniVRM and godot-vrm second half.

**Slice 1 success criteria:**
1. Four-adapter diff produced on `mtoon_basic_v0_lit_001` and `expressions_preset_basic_v0`.
2. VMK 180° flip flagged as conformance failure with clear visual signal in published site.
3. `vrm-normalize` round-trip property test passes in CI.
4. Methodology doc section live with spec citations, camera-Z table, and at least one failure-mode example image.
5. `spec_version` field present on every manifest entry; CI validator enforces.
6. Sweep registry symmetry assertion passes — every `*_v0` sweep entry has a 1.0 counterpart or `NotApplicable` reason.

### Slice 2 — Spring-bone v0 + MToon parametric parity (~3 weeks)

- Full 0.x MToon sweep parity (~44 variants × 0.x, modulo `NotApplicable` entries for v1-only axes).
- Spring-bone v0 sweep (~18 variants) — gravity, drag, joint count, sphere radius. **Same axes that translate cleanly to per-group semantics**; no fabricated topology to fake per-joint variation.
- `_v0_quirk_*` family first wave: `stiffinessForce` typo, centerNode-as-transform vs ignored, single-bone-per-group topology (`springbone_singleton_groups_v0_quirk`), sphere-collider-only enforcement.
- Methodology doc: **spring-bone triage-order pin (within-renderer-cross-version first)** — integrator-sensitivity confounds make this the right reading order, reversed from other sweeps.

### Slice 3 — VRMA × 0.x (~2 weeks)

- VRMA sweep mirror on 0.x avatars: `vrma_lookat_basic_v0`, `vrma_pose_basic_v0`, `vrma_expressions_basic_v0`.
- `_v0_quirk_vrma_*` family — what each adapter does when VRMA expression channels reference 1.0 preset names against a 0.x avatar's `blendShapeMaster`.
- Day-5 design retrospective on slice 1 contracts. VRMA × 0.x may surface design questions touching the ops contract or normalization crate — if so, that's a deliberate slice 3 decision, not slice 1 churn.

### Slice 4 — `render_sequence` v0 + closure (~2 weeks)

- `render_sequence` 0.x parity: `swing_seq_*_v0` family (~20 variants).
- Final methodology pass; close out any divergences from slices 1–3 as methodology hazards vs per-renderer bugs.
- RFC 0006 graduates Draft → Accepted; this design becomes the canonical reference.

**Total: ~10 weeks end-to-end.**

## Cross-cutting architecture

### `SpecVersion` enum

In `crates/vrm-ops/src/lib.rs`:

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpecVersion {
    #[serde(rename = "0.x")]
    V0,
    #[serde(rename = "1.0")]
    V1,
}
```

Wire form is `"0.x"` / `"1.0"`. Threaded through generator CLI (`--spec-version`), manifest schema, test plan metadata, ops contract.

### Manifest schema delta

`goldens/manifest.json` entries gain a required `spec_version` field:

```jsonc
{
  "test_id": "mtoon_basic_v0_lit_001",
  "spec_version": "0.x",
  "spec_section": "VRM 0.x MToon",
  ...
}
```

Slice 1 days 1–3 includes a migration commit backfilling existing 1.0 entries. `crates/vrm-s3/validate-manifest` enforces presence on all new entries.

### Generator structure

```
crates/vrm-asset-generator/src/
  lib.rs                # SpecVersion re-exported, SweepApplicability enum
  vrm_ext.rs            # VRM 1.0 — VRMC_vrm emit (existing)
  vrm_ext_v0.rs         # VRM 0.x — VRM extension emit (NEW, slice 1; strictly emit, no parser)
  spring_bone.rs        # VRM 1.0 — VRMC_springBone (existing)
  spring_bone_v0.rs     # VRM 0.x — secondaryAnimation (NEW, slice 2)
  mtoon_common.rs       # shared math (NEW, slice 1 — extracted from current mtoon emit)
  mtoon.rs              # VRM 1.0 wiring (existing, slimmed in slice 1)
  mtoon_v0.rs           # VRM 0.x materialProperties wiring (NEW, slice 1)
  expressions_v0.rs     # VRM 0.x blendShapeMaster (NEW, slice 1)
  sweep.rs              # registry gains SweepApplicability arms
```

**Shared math, separate wiring:** MToon 0.x and 1.0 share ~60–70% of the actual shading math. Extract that to `mtoon_common.rs`; let v0/v1 modules be thin wiring over it. Same principle for spring-bone in slice 2: shared Verlet step, separate topology resolution.

**`vrm_ext_v0.rs` is strictly emit.** No parser there. Round-tripping is not a v1 goal; if it becomes one, that's the trigger to revisit the single-crate decision.

### `SweepApplicability` enum

In `crates/vrm-asset-generator/src/lib.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SweepApplicability {
    Applicable,
    NotApplicable { reason: NotApplicableReason },
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum NotApplicableReason {
    PerJointStiffnessV1Only,
    CapsuleColliderV1Only,
    ExtendedCollidersV1Only,
    OutlineLightingMixV1Only,
    VrmaIsVrm1Era,
    // (more as discovered — every reason gets a named variant)
}
```

`NotApplicable` entries are first-class registry citizens. Runner produces no render but emits a structured "skipped, reason X" record visible at diff and site layers.

**Compile-time symmetry assertion** in `sweep.rs`:

```rust
#[test]
fn sweep_registry_symmetric_across_versions() {
    // For every sweep_id ending in _v0, assert a 1.0 counterpart exists
    // (Applicable or NotApplicable; both versions registered).
}
```

### Ops contract: `as_spec_version` on read-side dumps

`dump_humanoid_pose`, `dump_expression_weights`, `dump_look_at_state` gain optional `as_spec_version` enum param. Wire form is the same `SpecVersion` enum.

```jsonc
// dump_expression_weights response shape
{
  "source_spec_version": "0.x",   // always present; required field
  "weights": {
    "joy": 1.0,                   // native 0.x preset names (no as_spec_version requested)
    "neutral": 0.0
  }
}
```

Rules:
- Absent `as_spec_version` = native (asset's parsed shape verbatim).
- `as_spec_version=V1` against v0 asset → normalized output (joy→happy, sorrow→sad, etc.).
- `as_spec_version=V0` against v1 asset → **rejected**. Error envelope: `-32001, NormalizationDirectionUnsupported`.
- v0 custom blendshapes with no v1 preset → passed through with `custom:<name>` marker, never dropped.
- Write/control ops do **not** gain this param.

### `vrm-normalize` crate

```
crates/vrm-normalize/
  src/lib.rs           # public API
  src/expressions.rs   # joy→happy etc. mapping table + custom:<name> passthrough
  src/humanoid.rs      # bone-name normalization (mostly identical; few 1.0 renames)
  src/look_at.rs       # look_at state normalization
```

- Called by the **runner**, not by adapters. Single bug surface.
- CI integration test: round-trip property — `dump(as_spec_version=V1)` from adapter A ≡ from adapter B on losslessly-equivalent shapes, even when native dumps differ.

### Runner spec-version handling

- Test plan and manifest must agree on `spec_version`; runner errors hard on mismatch.
- Methodology pins enforced per-spec-version: camera Z direction, default tone-mapping, etc. Test plans cannot hardcode the wrong-handed camera.
- **Adapter loads content-agnostically** — `load_vrm` does *not* gain a `spec_version` param. Adapters enable both 0.x and 1.0 parsing paths (UniVRM passes `canLoadVrm0X: true` at adapter init, not per-op), then detect the actual version from `extensionsUsed` in the asset. Detected version is reported as `source_spec_version` on every dump response. Runner cross-checks `source_spec_version` against the test plan's declared version — third hard-error gate, complementing the test-plan ↔ manifest check.

## Methodology pins (new `docs/methodology.md` section)

### Camera convention (per-spec-version, not unified)

- **VRM 0.x plans:** camera at -Z (target origin) — avatar faces -Z per `specification/0.0/README.md:238`.
- **VRM 1.0 plans:** camera at +Z (target origin) — avatar faces +Z per `specification/VRMC_vrm-1.0/tpose.md` Definition 1.1.

Failure-mode example image: VMK 0.x render showing back-of-head — clear visual signature of the 180° flip. Caption explains how to read this in the consensus diff.

### Spring-bone triage order (reversed)

For most sweeps the canonical read is within-version-cross-renderer. Spring-bone reverses this because the simulation is integrator-sensitive (Verlet vs semi-implicit Euler, sub-stepping, damping models). Triage order:

1. **Within-renderer cross-version first** — VMK 0.x vs VMK 1.0 on `gravity_sweep`. Disagreement = coordinate or unit bug in one of VMK's emit paths.
2. **Within-version cross-renderer second** — three-vrm 0.x vs VMK 0.x. Disagreement = genuine adapter divergence in 0.x semantics.
3. Cross-version-cross-renderer last.

### Spec-quirk sweeps as first-class signal

`_v0_quirk_*` prefix denotes intentional probes of 0.x spec corners adapters often silently correct:

- `stiffinessForce` — canonical typo in the 0.x spec field name.
- centerNode-as-transform vs centerNode-ignored.
- Single-bone-per-group topology (`springbone_singleton_groups_v0_quirk`).
- Sphere-collider-only enforcement (capsule colliders must be rejected, not silently handled).
- 0.x `firstPerson` flagging semantics.
- 0.x meta schema (`licenseName: CC_BY` etc., vs 1.0's structured `meta.licenseUrl`).

An adapter that "fixes" the typo by also accepting `stiffness` on a 0.x asset is silently non-conformant. These exist explicitly to catch such "helpful" silent corrections.

### VRMA × 0.x handling (slice 3)

VRMA is in-scope on 0.x avatars as a deliberate "what does each adapter do?" probe. Spec doesn't define VRMA-on-0.x semantics, so the conformance signal is "do adapters agree on what they're doing," not "do adapters follow a spec." Lands as `_v0_quirk_vrma_*` family.

### Normalization is one-directional and lossy

Methodology doc lists the canonical v0→v1 preset mapping table:

| v0 (`blendShapeMaster`) | v1 (`VRMC_vrm.expressions.preset`) |
|---|---|
| `joy` | `happy` |
| `angry` | `angry` |
| `sorrow` | `sad` |
| `fun` | `relaxed` |
| `neutral` | `neutral` |
| `a`, `i`, `u`, `e`, `o` | `aa`, `ih`, `ou`, `ee`, `oh` |
| `blink` / `blink_l` / `blink_r` | `blink` / `blinkLeft` / `blinkRight` |
| `lookup` / `lookdown` / `lookleft` / `lookright` | `lookUp` / `lookDown` / `lookLeft` / `lookRight` |
| custom (any other) | `custom:<original-name>` |

v1→v0 is rejected: no lossless mapping exists for v1's `surprised` and others.

### Site display

`site/` gains a `spec_version` filter chip ("All" / "0.x" / "1.0") and a small per-card badge. Powered by the new manifest field. Lands in slice 1.

## Risks worth tracking

**Slice 1: VRoid Studio 0.x export availability.** Empirical check days 1–3. Fallbacks if removed:
- Re-export from older Studio version (we'd source one).
- Use VRoid Hub-sourced 0.x content (license-vetted).
- Drop the VRoid canonical fixture; rely on `avatarA_0_0` alone. Slice schedule absorbs by dropping one fixture, not slipping.

**Slice 1: VMK 180° flip — structural vs vestigial.** Empirical check days 1–3 against the VRMMetalKit-pinned revision in `adapters/vrm-metal-kit/Package.swift`. Outcomes:
- Adapter-shim local fix: one-line, but conformance failure still gets surfaced in slice 1 site output (demonstrates the suite catches it).
- Upstream library: file `docs/upstream/VMK-vrm-0x-orientation.md`; flag stays open through slices.
- Load-bearing for ArkavoCreator's ARKit alignment: issue filed but won't close quickly; methodology doc explains why the conformance flag stands.

**Slice 1: UniVRM coord-handling repro.** Before publishing the slice 1 UniVRM failure flag, isolate the repro and check UniVRM's issue tracker (https://github.com/vrm-c/UniVRM/issues). File with the conformance suite as repro if unfiled. Days 11–17 budget covers this.

**Slice 1: `mrxz/vrm-validator` 0.x coverage.** Empirical check days 1–3 against `crates/vrm-validator-wrap/`. If 0.x unsupported:
- Skip validator in CI for 0.x entries (record exemption in `.github/workflows/manifest-validate.yml`).
- Fall back to thinner local schema check against `docs/upstream-specs/vrm-specification/specification/0.0/schema/`.

**Slice 2: integrator-sensitivity confounds.** Spring-bone v0 diffs may be noisy. Mitigated by the triage-order pin. If noise floor too high, slice 2 retrospective decides whether to add a per-renderer SSIM tolerance band for `swing_springbone_*_v0`.

## RFC 0006 open questions — disposition

| RFC Q | Disposition |
|---|---|
| Q1 — Orientation methodology | Spec-pin -Z; divergence is the deliverable |
| Q2 — VRoid Studio 0.x export availability | Empirical check slice 1 days 1–3; fallbacks defined |
| Q3 — Adapter fix sequencing | Author plans first; file issues with the conformance suite as repro (slice 1 sequencing handles this implicitly) |
| Q4 — Spec-version detection in runner | Hard error on test-plan ↔ manifest mismatch |
| Q5 — Site display | Filter chip + per-card badge in slice 1 |

## Followups (deferred, not unresolved)

| Item | Defer to | Reason |
|---|---|---|
| Side-channel "native orientation" render | v2 | Scope creep; back-view failure already legibly diagnostic |
| VRM 1.1 plumbing | When 1.1 lands | `SpecVersion` enum extends cleanly; sweep registry stays |
| Schema-conformance probe (`docs/backlog.md`) | Independent issue | Generator-side; not gated by 0.x work |
| Pose-level diff in `consensus-report.sh` | Independent issue | Would amplify slice 1 signal materially; not gated |
| Round-tripping (parse 0.x assets) | Trigger to revisit single-crate decision | Explicit gate per "Generator structure" |
| 0.x emit through `vrm-asset-generator`'s MCP wrapper | After slice 1 | Thin shim; `--spec-version` flag flows through trivially |

## Cross-slice invariants

Never violated across any slice:

- Sweep registry symmetry assertion runs in CI for every slice.
- Manifest schema validator enforces `spec_version` field presence.
- `vrm-normalize` round-trip property test runs in CI.
- No adapter implements normalization — runner-side, single bug surface.
- Camera convention pin enforced by runner per `spec_version`; test plans cannot hardcode wrong-handed camera.
- `vrm_ext_v0.rs` and siblings are emit-only; any parser work triggers re-evaluation of the single-crate architecture.

## References

- [RFC 0006 — VRM 0.x conformance coverage](../../../rfcs/0006-vrm-0x-conformance.md) (this design supersedes RFC 0006's scope sketch)
- [RFC 0005 — canonical content tier](../../../rfcs/0005-canonical-content-tier.md) — Tier 2 methodology this design extends to 0.x
- `docs/findings.md` 2026-05-24 — empirical four-adapter orientation matrix that motivated this work
- `docs/methodology.md` — gains the new "VRM 0.x conformance" section per the "Methodology pins" section of this design
- `assets/humanoid/avatarA_0_0.vrm` — existing fixture; first 0.x test target in slice 1
