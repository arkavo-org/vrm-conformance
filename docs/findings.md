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
