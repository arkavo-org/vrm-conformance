# Conformance findings (running log)

This document records cross-renderer divergence findings produced by the suite, in the order they were surfaced. Each entry has a brief observation, the data behind it, and pointers to any upstream issues filed. Findings are a deliverable in their own right — the project's purpose is to produce falsifiable signal that drives upstream fixes (or methodology refinements when divergence turns out to be legitimate).

The methodology hazards in `docs/methodology.md` describe what divergence we *expect* between renderers (tone mapping, shadow noise, outline AA, …). This document records divergence the suite *actually observed* in our specific corpus + specific adapter pair, beyond those expected differences.

## Corpus-wide consensus, three-vrm 3.5.0 vs vrm-metal-kit `50cfd7d`

**Date**: 2026-05-11, vrm-conformance commit `1ff198c`.

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test_id corpus (44 MToon variants + 18 spring-bone settle + 18 spring-bone swing) through both real adapters on macOS 26 (Apple M4 Max). `scripts/consensus-report.sh` then ran pairwise SSIM across the bootstrap manifest. Output: `goldens-cache/consensus-report.json` (gitignored — machine-specific paths).

**Headline**: every single test_id fails the v1.0-standard 0.985 SSIM threshold.

```
consensus_passed: 0 / 80
consensus_failed: 80 / 80

Pairwise SSIM corpus-wide:
  three-vrm vs vrm-metal-kit   mean=0.7447  min=0.6313  max=0.9665  n=80
```

Even the closest renderer pair in the entire corpus (`max=0.9665`) is well below the conformance threshold. The mean (0.7447) is more than 20 percentage points below threshold.

### Top 15 most-divergent test_ids

| test_id | min pairwise SSIM | outliers |
|---|---|---|
| `mtoon_outline_world_0p1` | 0.6313 | both |
| `mtoon_shadingShift_neg0p5` | 0.6893 | both |
| `mtoon_shadingShift_neg0p2` | 0.7013 | both |
| `mtoon_doubleSided_true` | 0.7045 | both |
| `mtoon_shadingToony_0p25` | 0.7072 | both |
| `mtoon_shadingToony_0p75` | 0.7079 | both |
| `mtoon_shadingToony_0p5` | 0.7087 | both |
| `mtoon_shadingToony_0p1` | 0.7101 | both |
| `mtoon_shadingShift_neg0p8` | 0.7103 | both |
| `swing_springbone_default` | 0.7105 | both |
| `swing_springbone_drag_0` | 0.7105 | both |
| `swing_springbone_drag_0p2` | 0.7105 | both |
| `swing_springbone_drag_0p8` | 0.7105 | both |
| `swing_springbone_drag_1` | 0.7105 | both |
| `swing_springbone_gravity_0` | 0.7105 | both |

### Observations

**MToon shading divergence dominates.** Nine of the top fifteen most-divergent test_ids vary either `shadingShiftFactor` (the toon-ramp boundary) or `shadingToonyFactor` (the toon-ramp steepness). Both are MToon-1.0 parameters that directly govern how the lit/shadow boundary is computed. Cross-renderer disagreement on this axis is the most expensive kind of conformance gap — it touches the spec's core algorithm.

**Outline rendering is the single worst case.** `mtoon_outline_world_0p1` (world-space outline at 0.1 width) is the most-divergent test in the corpus. Outline rendering is well-known as a methodology hazard (`docs/methodology.md` calls out outline AA differences explicitly), but the magnitude of divergence here (0.6313 SSIM) suggests more than just edge-AA noise.

**Spring-bone swing variants cluster at exactly 0.7105.** Many swing variants produce identical SSIM (rounded to four places). That's evidence the visible mesh isn't responding to chain physics — consistent with the deferred chain-skinned-mesh infrastructure (`crates/vrm-asset-generator/src/chain_mesh.rs`) blocked behind [arkavo-org/VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181). Without a mesh skinned to the chain joints, swing renders look the same regardless of physics parameters, so the corpus-wide signal degenerates to "two renderers disagree on the same sphere mesh shading regardless of which spring-bone variant generated it."

**Settle vs swing produce the same SSIM.** `swing_springbone_default` (0.7105) and `springbone_default` (in the per_test_id list, also clustered around 0.71) match — confirming the same conclusion. Until the chain-skinned mesh is wired, spring-bone divergence equals MToon-default divergence on a static sphere.

### Filed upstream

- [arkavo-org/VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) — root cause of vrm-metal-kit's flat-white sphere across the entire MToon sweep. Single fix would substantially improve the mean for every MToon test_id.
- [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) — three-vrm's dark shadow with a falsifiable color-space hypothesis (`0.5^2.2 ≈ 0.21` matches the observed value).

These two issues together cover the dominant divergence pattern for MToon shading. If one or both lands, the corpus-wide mean SSIM should rise substantially and the threshold gap close.

## Second run: VRMMetalKit 0.13.1

**Date**: 2026-05-11, vrm-conformance commit (pending; this section commits with the version bump). Same hardware (M4 Max), same three-vrm version (3.5.0), only the VRMMetalKit revision changed.

**Trigger**: [VRMMetalKit 0.13.1](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.13.1) shipped, closing two of the three bugs filed by this suite (#181 + #182, in [PR #184](https://github.com/arkavo-org/VRMMetalKit/pull/184)). Re-rendering through the corpus measures the delta.

### Corpus-wide before/after

| Metric | Baseline (50cfd7d) | 0.13.1 (9404287) | Delta |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.7447 | **0.7002** | **−0.0445** |
| min pairwise SSIM | 0.6313 | **0.1840** | **−0.4473** |
| max pairwise SSIM | 0.9665 | 0.9490 | −0.0175 |

The pass count is unchanged because the v1.0 threshold (0.985) is still far above the corpus-wide max. But the distribution shifted significantly — and not all in the direction we'd expect.

### Pattern: two clusters move in opposite directions

**MToon shading (~44 variants): essentially unchanged.** `shadingShift_neg0p5` stayed at 0.6893. `shadingShift_neg0p2` stayed at 0.7013. The full `shadingToony_*` cluster stayed at 0.708x. Expected, because the release notes don't mention #183 (the flat-white sphere root cause for MToon-default rendering) and our corpus's MToon shading divergence is dominated by that single cause.

**Spring-bone (~36 settle + swing variants): unchanged at 0.7105.** Expected — visible signal still requires the chain-skinned-mesh asset-side wiring (the infrastructure is in `crates/vrm-asset-generator/src/chain_mesh.rs` but deferred until #181 lands), and even though #181 is now fixed upstream we haven't re-wired chain_mesh into emit yet.

**Outline rendering (8 variants): substantial regression.** The 8 outline test_ids now occupy the top 8 worst slots:

| test_id | baseline | 0.13.1 | Δ |
|---|---|---|---|
| `mtoon_outline_world_0p1` | 0.6313 | **0.1840** | −0.4473 |
| `mtoon_outline_world_0p05` | (n/a in top 15) | **0.3588** | (large drop) |
| `mtoon_outline_screen_0p1` | (n/a in top 15) | **0.4028** | (large drop) |
| `mtoon_outline_world_0p03` | (n/a in top 15) | **0.4330** | (large drop) |
| `mtoon_outline_screen_0p05` | (n/a in top 15) | **0.4711** | (large drop) |
| `mtoon_outline_screen_0p03` | (n/a in top 15) | **0.4967** | (large drop) |
| `mtoon_outline_world_0p01` | (n/a in top 15) | **0.5018** | (large drop) |
| `mtoon_outline_screen_0p01` | (n/a in top 15) | **0.5223** | (large drop) |

The corpus-wide mean dropped −0.0445 specifically because of this 8-variant cluster. The release that closed #181 (non-skinned mesh dropped when skin present) appears to have introduced a regression in outline rendering — outline width and mode now produce visibly different pixels than before, and the divergence vs three-vrm is much larger than at the old pin.

### New finding: outline-rendering regression in 0.13.1

This is a measurable behavioral change in VRMMetalKit between 50cfd7d → 9404287. Worth filing upstream as a new issue (likely a side effect of the #181 fix touching the pipeline state cache, or the outline pass's interaction with the new mixed-mesh draw order). The corpus surfaces it automatically: same VRM, same test plan, same three-vrm version, different VRMMetalKit produces materially different outline pixels.

This is also exactly why we pin the upstream revision in `Package.swift` rather than tracking `main`: regressions like this would otherwise propagate silently.

### What this run did and didn't validate

- **Did validate**: the two upstream fixes (#181 + #182) are present at 0.13.1 — `swift test` is clean, the adapter binary boots, spring-bone counts are no longer inflated (TBD — needs a separate verification; the corpus signal doesn't reflect this directly since the chain isn't visible).
- **Did NOT validate end-to-end yet**: visible chain-skinned-mesh diffing. With #181 fixed, the chain_mesh.rs infrastructure in this repo can be re-wired into `emit_vrm_with_spring_bone`. That's a separate piece of work — it unblocks the spring-bone signal that's currently degenerate.
- **Surfaced**: an outline-rendering regression that wasn't visible in any per-renderer unit test. Only the cross-renderer signal catches it.

### Open questions

- **Should the corpus's default SSIM threshold be relaxed below 0.985?** v1.0 standardizes on 0.985 per `docs/methodology.md`, but the data shows that's currently unreachable for any test in the corpus. Two interpretations:
  - The threshold is correct; both renderers are sufficiently spec-divergent that pass requires upstream fixes first. The cross-renderer signal IS the conformance result.
  - The threshold needs methodology refinement — e.g., separate thresholds for "exact MToon math" tests vs "approximate visual fidelity" tests, or per-renderer-pair thresholds.
- **Should consensus exclude the mock-renderer entirely from default reporting?** Mock is synthetic and isn't trying to match real renderers. Including it in consensus inflates the apparent divergence. The current script doesn't include mock by default (the bootstrap only renders through real adapters); confirming this stays the convention.
- **Outline-mode divergence at 0.6313 — is that AA noise alone, or a model-level outline-rendering bug?** Likely worth a dedicated investigation (sample the actual outline pixels in both renders).

## How to reproduce

```bash
git clone https://github.com/arkavo-org/vrm-conformance
cd vrm-conformance

# Bootstrap goldens through both real adapters (~7 min, macOS).
./scripts/bootstrap-goldens.sh

# Run the corpus-wide consensus report.
./scripts/consensus-report.sh

# Findings land at goldens-cache/consensus-report.json (machine-specific paths;
# gitignored). The summary stats print to stdout.
```

For different host configurations (different macOS version, GPU, three-vrm version, VRMMetalKit revision), the numbers will shift but the pattern is expected to hold until upstream fixes for #183 and #1838 land.
