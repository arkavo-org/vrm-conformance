# RFC 0006: VRM 0.x conformance coverage

- **Status:** Draft (scope sketch — design TBD)
- **Author(s):** Paul Flynn
- **Date:** 2026-05-24

## Summary

Add first-class coverage for the legacy VRM 0.x format alongside the existing VRM 1.0 corpus. Today the suite is VRM-1.0-only across asset generation, test plans, and adapter coverage. VRoid Hub and the broader downstream ecosystem still contain enormous amounts of VRM 0.x content (every avatar uploaded before the ~2022 1.0 transition), and the project's stated downstream goal — "VRM imported into a game with physics and collisions working out of the box" — is incomplete without it. This RFC scopes the work; design follows in a v2 once the cross-cutting questions are answered.

## Motivation

Three independent signals point to VRM 0.x being a real coverage gap:

1. **Asset corpus reality.** The fixture `avatarA_0_0.vrm` already exists in `assets/humanoid/` (installed by `scripts/install-humanoid-fixtures.sh`) but is exercised by zero test plans. The asset was inherited from VRMMetalKit and predates the suite's VRM 1.0 focus.
2. **Adapter coverage matrix is currently lopsided.** Smoke-tested 2026-05-24 (see `docs/findings.md` post-camera-flip subsection): VMK, three-vrm, and godot-vrm all load `avatarA_0_0.vrm` cleanly. UniVRM's adapter rejects it with `Failed to load as VRM 1.0` because the adapter passes `canLoadVrm0X: false` to `Vrm10.LoadPathAsync`. One-line adapter config change unblocks UniVRM for VRM 0.x.
3. **Cross-spec-version orientation behavior diverges in surprising ways.** Same smoke surfaced that adapters disagree on whether VRM 0.x avatars receive a 180° rotation on load (VMK applies the rotation, three-vrm and godot don't), while all three apply the rotation on VRM 1.0 files (contrary to VRM 1.0 spec). The conformance question of "does the spec-version-appropriate orientation actually round-trip through each adapter" is currently invisible to the corpus.

## Detailed design (scope sketch)

The actual design is deferred. This section enumerates the major decisions a v2 will need to make.

### Asset coverage

- **Tier 2 canonical content** (RFC 0005 methodology): re-export the VRoid Studio default character as VRM 0.x (Studio's "Export → VRM 0.x" path produces it). Land as `vroid_default_F_0_0.vrm` alongside the existing `vroid_default_F_1_0.vrm`. License metadata is set the same way (VRM 0.x has its own meta schema with `licenseName: CC_BY` etc.; covered by RFC 0005's sidecar provenance pattern).
- **Tier 1 parametric synthetic**: significant chunk of work in `crates/vrm-asset-generator/`. New emit paths for the `VRM` extension namespace (not `VRMC_vrm`), `secondaryAnimation` (not `VRMC_springBone`), `materialProperties` (not `VRMC_materials_mtoon`), and the 0.x meta schema. Parametric MToon sweep needs duplication for 0.x MToon's different JSON shape.

### Test plan schema changes

The existing `TestPlan` schema in `crates/vrm-test-plan/` is spec-version-agnostic — `spec_section` is a free-form string. No struct changes needed; just convention discipline. However:
- The runner needs to pick the right humanoid camera convention per spec version. Currently empirical: both 0.x and 1.0 specify the avatar facing -Z in glTF coordinates (VRM 0.x phrases it as "Unity +Z forward" but the export Z-flip lands at glTF -Z). Unified -Z camera pin works for both. Implementations diverge, but the spec convention is shared.
- Diff strategy: same SSIM + consensus story. No diff-engine changes.

### Adapter coverage

| Adapter | VRM 0.x load today | Fix path |
|---|---|---|
| VMK 0.16.0 | ✓ loads cleanly | None — works |
| three-vrm 3.5.0 | ✓ loads cleanly | None — works |
| godot-vrm | ✓ loads cleanly | None — works |
| UniVRM v0.131.0 | ✗ `canLoadVrm0X: false` in adapter | One-line: pass `canLoadVrm0X: true` to `Vrm10.LoadPathAsync` |

Universal load support is one adapter line away. **However**, "loads cleanly" is not the same as "renders correctly per VRM 0.x spec." See orientation divergence below.

### Orientation handling (the hard part)

Empirically observed 2026-05-24 from smoke through `avatarA_0_0.vrm` and `vroid_default_F_1_0.vrm`:

| Adapter | VRM 0.x: avatar faces | VRM 1.0: avatar faces | Spec-correct? |
|---|---|---|---|
| VMK | +Z (applies 180° flip) | +Z (applies 180° flip) | Neither matches spec (-Z) |
| three-vrm | -Z (native, no flip) | +Z (applies 180° flip) | VRM 0.x ✓, VRM 1.0 ✗ |
| godot-vrm | -Z (native, no flip) | +Z (applies 180° flip) | VRM 0.x ✓, VRM 1.0 ✗ |
| UniVRM | (cannot load) | -Z (no flip) | VRM 1.0 ✓ |

No single adapter is fully spec-correct across both spec versions. This means the corpus's "humanoid plans use -Z camera" methodology pin (also added today) produces:
- VRM 0.x plans: only three-vrm + godot show the front; VMK shows the back; UniVRM can't render.
- VRM 1.0 plans: only UniVRM shows the front; the other three show the back.

The conformance signal is clear (per-adapter, per-spec-version bug), but the consensus matrix is hard to interpret. Open question for the design phase: do we (a) accept the cross-spec inconsistency and let the corpus surface it as renderer bugs to file upstream, (b) document an "expected-divergence" map per spec version that filters out the known orientation bugs at the diff layer, or (c) author plans with both -Z and +Z cameras and let plan authors document which side they expect each renderer to see?

### Manifest schema

Add an optional `spec_version: "0.x" | "1.0"` field to manifest entries. Backward-compatible: absent value defaults to inferring from `spec_section` string.

### Methodology pins

- Camera convention: -Z (target origin) for both VRM 0.x and VRM 1.0. Avatar faces -Z in glTF per both specs (VRM 0.x via Unity-coord-spec + export Z-flip; VRM 1.0 directly).
- Spring-bone: VRM 0.x uses `secondaryAnimation` (no `centerNode`, different drag/stiffness semantics, no `colliderGroups[].name`); VRM 1.0 uses `VRMC_springBone`. Methodology hazard: 0.x and 1.0 spring-bone math is not bit-equivalent; consensus across spec versions on the same intended scene is out of scope.
- MToon: 0.x uses `materialProperties` (Unity-style key-value), 1.0 uses `VRMC_materials_mtoon` (structured). Parameter naming + ranges differ enough that the 1.0 MToon sweep enumeration is NOT directly re-usable for 0.x. Need a separate sweep for 0.x materials.

## Alternatives considered

**Skip VRM 0.x entirely.** Argument: 0.x is legacy, the format is fading, Khronos donation aligns with VRM 1.0 only. Counter-argument: the downstream goal is universal "import-and-play," and the installed base of 0.x content is too large to ignore. Most VRoid Hub avatars uploaded before the 1.0 transition. Skipping 0.x means the suite cannot answer "does this VRoid Hub avatar work in my game" for ≈half the realistic input distribution.

**Implement VRM 0.x via conversion-to-1.0 at load time.** Some adapters could in principle auto-convert 0.x → 1.0 on load (UniVRM has such a path). Argument: avoids the entire orientation/material/spring-bone spec divergence by treating 0.x as just a different input format for 1.0 conformance. Counter-argument: the conformance question we want to answer is "does this 0.x file work in renderer X *as 0.x*," because that's how downstream apps will load it. Pre-converting bypasses the bugs we want to catch.

**Author Tier 2 0.x fixtures only; skip Tier 1 parametric.** Argument: 0.x parametric work is large; canonical content alone exercises the surface. Counter-argument: parametric is where renderer-axis isolation lives. Without 0.x parametric, we can't isolate which axis (spring-bone, MToon, orientation, …) a 0.x-specific failure is on. Worth considering as a v1 cut where Tier 2 lands first and Tier 1 follows in a v2.

## Open questions

1. **Orientation methodology** (largest design question). See "Orientation handling" above. The corpus methodology pin is -Z; reality is that no adapter is fully spec-correct across both versions. Pick (a), (b), or (c) — each has implications for what "conformance" means here.
2. **VRoid Studio's 0.x export path** — does it still ship in VRoid Studio 2.12.0, or has it been removed in favor of 1.0-only? If removed, we have to source 0.x canonical content from somewhere else (re-export from older Studio version, or use Hub-sourced content).
3. **Adapter fix sequencing** — file the four upstream orientation issues (one per adapter) before or after authoring 0.x plans? Authoring first surfaces more data but renders consistently broken; filing first gives upstream maintainers an actionable reproducer.
4. **Spec-version detection in the runner.** Should the runner inspect the asset to detect spec version and warn/error if the test plan's `spec_section` doesn't match? Cheap correctness check.
5. **Site display** — do we want a spec-version filter / badge in the published comparison site? Manifest schema change above is the data piece; site work is a follow-on.

## References

- `docs/findings.md` entry dated 2026-05-24, "Downstream goal calibration — VRoid Hub baseline, Muse 0.16.0 diagnosis correction, two-tier corpus pivot" — RFC 0005 follow-on.
- `docs/findings.md` entry dated 2026-05-24, "First four-adapter bootstrap of `vroid_default_F_collider_settle`" — empirical data on cross-adapter orientation handling.
- `docs/findings.md` post-camera-flip subsection (same date) — VRM 0.x adapter-load smoke + orientation matrix that motivates the orientation methodology question.
- `rfcs/0005-canonical-content-tier.md` — Tier 2 canonical content methodology; this RFC extends Tier 2 coverage to VRM 0.x.
- VRM 0.x specification — `https://github.com/vrm-c/vrm-specification/tree/master/specification/0.0` (legacy, Unity-coord-centric).
- VRM 1.0 specification — `https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm-1.0` (Khronos-ratified, glTF-coord-centric).
- `assets/humanoid/avatarA_0_0.vrm` — existing fixture; will become the first VRM 0.x test target.
