# RFC 0005: Canonical-content tier — VRoid Studio baselines alongside parametric sweeps

- **Status:** Draft
- **Author(s):** Paul Flynn
- **Date:** 2026-05-24

## Summary

Introduce a second corpus tier alongside the existing parametric synthetic sweeps: **canonical real-content fixtures** sourced from VRoid Studio with permissive license fields set at export time. Tier 1 (existing) tests renderer-axis isolation through one-axis-at-a-time parametric sweeps on stripped baseline rigs. Tier 2 (new) tests downstream-realism conformance on the bone topology, multi-chain spring-bone layout, and in-file collider declarations that real VRoid Hub content actually ships. Both tiers belong; neither replaces the other. First Tier 2 fixture (`vroid_default_F_1_0.vrm`) lands alongside this RFC.

## Motivation

The project's stated downstream goal is "VRoid Hub `.vrm` imported into a game with physics and collisions working out of the box, with VRoid Studio as the recommended creator entry point." Our existing 263-plan corpus is entirely parametric synthetic. That choice was deliberate and remains correct for renderer-axis regression isolation — Khronos's `glTF-Render-Fidelity` methodology, which this suite is donation-aligned with, is built on parametric synthetic exactly for that reason. But the parametric synthetic corpus structurally cannot answer the downstream goal's question: it doesn't represent the bone topology, multi-chain physics, or in-file collider declarations that real content has.

Three independent gaps in the suite converge on the same missing fixture shape:

1. **VMK#267 (`writeBonesToNodes` 1-frame lag).** Open upstream issue against VRMMetalKit, flagged in our prior findings catalog with the note "synthetic swing sweep at 0.2 m / 0.25 s may not surface 1-frame lag; avatarA_bosom_swing more realistic." Frame-timing bugs surface against realistic chain mass + decay characteristics, not against single-chain synthetic excitation.
2. **Muse 0.16.0 procedural-collider diagnosis** (`docs/findings.md` entry dated 2026-05-24). A downstream tester reported VMK's host app computing procedural body-blocker spheres that never reach the spring-bone simulation. The diagnosis-of-fix turned on a single empirical question — "what collider groups does VRoid actually ship in the default export?" — that the synthetic corpus could not answer. Once answered against a real VRoid baseline, the apparent need for a new upstream API dissolved.
3. **`VRMC_springBone_extended_collider` containment realism.** Our 36 extended-shape variants (phase 3) render against synthetic rigs that never enter the inverted-containment shell of `insideSphere` / `insideCapsule` colliders. The SSIM signal compresses tightly across renderers and the conformance question becomes harder to read.

A canonical real-content fixture provides direct purchase on all three. It is also the single cheapest methodology improvement available: VRoid Studio exports take ≈30 minutes of UI work and ship with VRM-spec-defined license metadata that obviates the per-fixture licensing audit hand-authored or Hub-sourced fixtures would require.

This is a methodology pin update, not just a fixture addition. Codifying the two-tier model explicitly is the point of doing it as an RFC rather than a quiet PR.

## Detailed design

### The two tiers

**Tier 1 — Parametric synthetic (existing).** Owned by `crates/vrm-asset-generator/`. One-axis-at-a-time sweeps of MToon parameters, spring-bone physics parameters, collider geometries, gravity directions, etc. Baseline rig is a stripped humanoid skeleton with single chains attached only as needed for the swept axis. Plans + assets emitted as paired triplets (`<id>.vrm`, `<id>.meta.json`, `<id>.test.yaml`) from one parameter dictionary. Methodology-aligned with Khronos `glTF-Render-Fidelity`. Renderer-axis isolation is the design goal.

**Tier 2 — Canonical real-content (new).** Authored in VRoid Studio at a pinned version + template + export configuration. License fields set at export time so redistribution + Khronos donation are clean by the file's own metadata. Multi-chain spring-bone layouts, comprehensive in-file collider declarations, MToon parameters in realistic ranges. Plans are hand-authored in `test-plans/manual/humanoid/` (paralleling the existing avatarA pattern). Downstream-realism is the design goal.

Tier 1 fails: "renderer X mis-applies MToon shadingShift at 0.5." Tier 2 fails: "renderer X loads a VRoid avatar but hair clips through the chest because in-file collider groups aren't applied to declared springs." These are different failure classes; both are real; both belong.

### Fixture authoring + provenance

Every Tier 2 fixture lands with a sidecar `.meta.json` recording:

```jsonc
{
  "id": "vroid_default_F_1_0",
  "tier": "canonical_content",
  "source": {
    "tool": "VRoid Studio",
    "tool_version": "<x.y.z>",            // pin so re-export is reproducible
    "template": "default_female",         // VRoid template identifier
    "export_format": "VRM 1.0",
    "export_settings": {
      "mesh_reduction": false,
      "atlas_baking": false,
      "mtoon_to_standard_conversion": false
      // Whatever preserves spring-bone + collider declarations verbatim.
    }
  },
  "license": {                            // copied from VRMC_vrm.meta for at-a-glance audit
    "licenseUrl": "https://vrm.dev/licenses/1.0/",
    "avatarPermission": "everyone",
    "commercialUsage": "corporation",
    "allowRedistribution": true,
    "modification": "allowModificationRedistribution"
  },
  "topology": {                           // inspection summary, not authoritative
    "spring_bone_spec_version": "1.0",
    "colliders": 28,
    "collider_groups": 12,
    "springs": 44,
    "extensions_used": ["VRMC_vrm", "VRMC_springBone", "VRMC_materials_mtoon", "..."]
  },
  "blake3": "blake3:<64-hex>",
  "byte_size": <int>,
  "spec_section": "VRMC_vrm + VRMC_springBone + VRMC_materials_mtoon (real content)"
}
```

The `license` block is the authoritative legal layer for Tier 2; the suite's runner / manifest tooling can validate that every Tier 2 fixture has `allowRedistribution: true` before pushing to S3 or referencing in the published manifest.

### Where fixtures live on disk

Match the existing convention:

- `assets/humanoid/vroid_<template>_<variant>_<version>.vrm` — symlink to source-of-truth (see "Source-of-truth location" below).
- `assets/humanoid/vroid_<template>_<variant>_<version>.meta.json` — sidecar per the schema above.
- `test-plans/manual/humanoid/vroid_<template>_<variant>_<scenario>.test.yaml` — one or more plans per fixture covering distinct scenarios (settled-pose collider behavior, swing motion, lookAt application, etc.).

**Naming.** `vroid_<template>_<variant>_<version>.vrm`. `<template>` identifies the VRoid template; `<variant>` distinguishes downstream stylings (e.g., `default`, `stripped`, `accessorized`); `<version>` is the VRM spec version. First fixture: `vroid_default_F_1_0.vrm`.

**Source-of-truth location.** Existing convention (followed by `avatarA_0_0.vrm` / `avatarA_1_0.vrm`) places the canonical binary in `../VRMMetalKit/` and symlinks into `assets/humanoid/`. The new VRoid fixtures follow the same pattern. (Whether `../VRMMetalKit/` is the long-term right home for shared canonical content is a separate discussion; this RFC inherits the convention rather than relitigates it.)

### Diff strategy for Tier 2

Tier 2 fixtures have no oracle renderer — there is no "right answer" to compare a VRoid avatar's render against in the way there is for a parametric MToon material test. Two valid strategies:

1. **Cross-renderer consensus (`diff.mode: consensus`)**. Run the plan through every available real adapter, compute pairwise SSIM, flag outliers. This is the dominant Tier 2 mode. Bootstrap empirically — the threshold for "outlier" is calibrated against the first multi-renderer bootstrap of each plan.
2. **Pinned reference renderer (`diff.mode: ssim` + `reference_renderer: <name>`)**. Where one renderer is empirically known to be the most spec-compliant on the relevant axis, plans may pin it as oracle. Used sparingly and re-evaluated when the reference renderer ships a behavioral change.

Provisional thresholds are tagged as such in the plan YAML's `description` field or an inline `# provisional, calibrate on first bootstrap` comment. The first multi-renderer bootstrap of each plan replaces the provisional threshold with an empirically-grounded one.

### Manifest schema

Existing `goldens/manifest.json` entries gain an optional `tier: "parametric_synthetic" | "canonical_content"` field. Backward-compatible: missing `tier` defaults to `parametric_synthetic` (the current entire corpus is Tier 1). `validate-manifest` in `crates/vrm-s3/` adds a check: if `tier == "canonical_content"`, the manifest entry MUST include `license_url` + `allow_redistribution: true` mirrored from the fixture's sidecar, otherwise `push-manifest` refuses to publish.

### Bootstrap behavior

`scripts/bootstrap-goldens.sh` gains a `TIER` env var:

- `TIER=tier1` (default if unset, preserves existing behavior) — only parametric synthetic.
- `TIER=tier2` — only canonical content.
- `TIER=both` — both, sequentially.

Smoke and CI default to `tier1` (fast iteration on parametric regressions). Full periodic bootstraps run `both`.

### Fixture count discipline

Tier 2 is **not** parametric — adding 30 Tier 2 fixtures by varying VRoid Studio templates is bad design (the parametric Tier 1 already does that better against a tool we control). Open with a small, well-justified set:

- `vroid_default_F_1_0.vrm` — Studio default female, default physics, default hair / skirt / accessories. The most common downstream content shape. **Landing in this commit.**
- `vroid_default_M_1_0.vrm` — male base for bone-topology coverage. Some renderer bugs only surface on one humanoid binding. **Deferred** until Tier 1+2 infrastructure proves out on the F fixture.
- `vroid_stripped_F_1_0.vrm` — same default F template but bald + no skirt + one bust chain retained, for hair-vs-body-blocker signal isolation. **Deferred** until a signal-attribution problem on the default fixture demonstrates the need.

Resist scaling beyond ≈3 fixtures. The Tier 2 job is "representative real content," not "exhaustive coverage" — that's Tier 1's job.

### Refresh cadence

VRoid Studio ships updates that can change default template geometry, default physics parameters, default collider declarations, or MToon defaults. Tier 2 fixtures pin a specific Studio version in their sidecar provenance. On a Studio version bump:

- Re-export the fixture from the new Studio version into a new file (e.g., `vroid_default_F_1_0.vrm` stays; new export becomes `vroid_default_F_1_1.vrm` if it's still VRM 1.0). The old version is preserved for historical bootstrap reproducibility.
- Document the diff between Studio versions in a new findings.md entry — what changed in the export, whether thresholds need recalibration.

The corollary: fixtures don't get silently overwritten when re-exported. Each is content-addressed by its BLAKE3 + Studio version provenance.

## Alternatives considered

**Hand-authored Blender humanoid (e.g., the deferred `avatarA_collider_1_0.vrm` backlog item).** Rejected. Half a day of authoring vs. half an hour of Studio export; the Blender output represents one author's idea of "humanoid with collider" rather than the actual de facto downstream content shape; the licensing layer would have to be re-litigated per fixture rather than inheriting from the file's own `VRMC_vrm.meta`. Worth keeping the option open for very specific bespoke scenarios that Studio cannot produce (e.g., a humanoid with deliberately minimal collider declarations to isolate a specific Muse-class question — see fixture discipline above), but not as the default path.

**Sampling permissively-licensed VRMs from VRoid Hub or similar.** Rejected for the default case. Authors set permissive metadata for various reasons that may not align with our redistribution + Khronos donation needs; per-fixture license audits at Hub-scale are tedious and brittle; and we lose the reproducibility benefit (Studio version + template + export settings) that makes Tier 2 fixtures stable over time. Hub-sourced fixtures may make sense as a Tier 3 ("found content," explicitly distinct provenance) later. Out of scope for this RFC.

**Tier 2 as a replacement for Tier 1.** Rejected outright. Parametric synthetic + glTF-Render-Fidelity-style methodology is the project's load-bearing reason for being. Tier 2 is additive. Anything that catches a renderer bug on Tier 1 is signal we cannot afford to lose by moving to "realistic content only."

**Bundling provenance into the asset's `VRMC_vrm.meta` instead of a sidecar.** Rejected. `VRMC_vrm.meta` is the spec-defined license layer; suite-specific provenance (Studio version, export settings) belongs out-of-band so downstream importers reading the meta don't see suite-internal fields they don't recognize. Sidecar `.meta.json` is the existing convention for parametric synthetic and extends cleanly to Tier 2.

## Open questions

1. **VRoid Studio update cadence vs. fixture stability.** Studio ships changes regularly. If a new template revision changes default physics parameters, every Tier 2 plan referencing that fixture may need threshold recalibration. The RFC's stance — pin versions, never silently overwrite — keeps reproducibility clean but means Tier 2 maintenance overhead grows linearly with Studio releases. We'll learn the real cost from the first Studio version bump after Tier 2 lands; revisit if it exceeds Tier 1's parametric-corpus maintenance cost.

2. **Source-of-truth disk location.** Inheriting `../VRMMetalKit/` from the existing avatarA convention is convenient but couples this repo's fixture lifecycle to another repo's directory layout. Long term, a dedicated `vrm-fixtures` repo or a fixtures S3 bucket with content-addressed paths is cleaner. Out of scope for this RFC; flag for a future RFC if Tier 2 grows beyond 3 fixtures.

3. **Diff threshold calibration workflow.** Provisional thresholds + "calibrate on first bootstrap" is a soft commitment. We should harden it: a CI check that flags Tier 2 plans with `# provisional` markers older than N days, prompting an empirical recalibration. Out of scope for this RFC, but worth tracking once the first Tier 2 plans land.

4. **What happens when `npm/yarn`-style consumers of `goldens/manifest.json` see `tier: canonical_content` entries.** Site display, downstream tooling, etc. are presumed Tier-1-only today. The site at `site/` will need a visual indicator that Tier 2 entries exist and are different (e.g., a "real content" badge alongside the SSIM number). Out of scope for this RFC; the manifest schema change is backward-compatible, so site updates can land independently.

## References

- `docs/findings.md` entry dated 2026-05-24, "Downstream goal calibration — VRoid Hub baseline, Muse 0.16.0 diagnosis correction, two-tier corpus pivot" — full diagnosis trail this RFC formalizes.
- `docs/methodology.md` — current single-tier methodology pin; will be updated to reference this RFC once accepted.
- VRM 1.0 spec, `VRMC_vrm.meta` — the spec-defined license layer that obviates per-fixture licensing audit for Tier 2.
- Khronos `glTF-Render-Fidelity` — the parametric-synthetic methodology Tier 1 is donation-aligned with.
- Prior RFCs: `rfcs/0001-monorepo-confirmed.md`, `rfcs/0002-anti-fraud-submission-integrity.md`, `rfcs/0003-engine-idiom-divergence.md`, `rfcs/0004-render-sequence-op.md`.
- Upstream issue VMK#267 (`writeBonesToNodes` 1-frame lag) — open against VRMMetalKit, one of the three gaps that motivates Tier 2.
- Downstream Muse 0.16.0 procedural-collider report — the diagnostic incident that surfaced the methodology gap.
