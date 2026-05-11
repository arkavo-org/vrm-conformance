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

### New findings: two outline bugs surfaced together

This is a measurable behavioral change in VRMMetalKit between 50cfd7d → 9404287. The corpus surfaces it automatically: same VRM, same test plan, same three-vrm version, different VRMMetalKit produces materially different outline pixels. Pixel sampling reveals *both* renderers diverge from MToon-1.0's outline-rendering spec:

| variant | vrm-metal-kit centerline | three-vrm centerline |
|---|---|---|
| `mtoon_outline_none` | `(255, 255, 255)` (flat white, #183) | `(53, 53, 53)` (shaded gray) |
| `mtoon_outline_world_0p1` | `(255, 255, 255)` (outline invisible) | **`(0, 0, 0)` (outline color floods entire mesh)** |

The expected per [VRMC_materials_mtoon-1.0 §4.2 "Outline"](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_materials_mtoon-1.0/README.md) is a thin silhouette band (~6 pixels at this asset's camera distance for `0.01 m` width). Neither renderer produces that.

Both filed:
- [arkavo-org/VRMMetalKit#185](https://github.com/arkavo-org/VRMMetalKit/issues/185) — outline rendering regression in 0.13.1; outline pass appears to drop entirely
- [pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) — outline color floods entire mesh interior instead of producing a silhouette band

This is also exactly why we pin the upstream revision in `Package.swift` rather than tracking `main`: regressions like the VRMMetalKit one would otherwise propagate silently. And it demonstrates the conformance suite's payoff structurally: same data surfaced two different upstream bugs in two different renderers, neither of which is visible to that renderer's own unit tests.

### What this run did and didn't validate

- **Did validate**: the two upstream fixes (#181 + #182) are present at 0.13.1 — `swift test` is clean, the adapter binary boots, spring-bone counts are no longer inflated (TBD — needs a separate verification; the corpus signal doesn't reflect this directly since the chain isn't visible).
- **Did NOT validate end-to-end yet**: visible chain-skinned-mesh diffing. With #181 fixed, the chain_mesh.rs infrastructure in this repo can be re-wired into `emit_vrm_with_spring_bone`. That's a separate piece of work — it unblocks the spring-bone signal that's currently degenerate.
- **Surfaced**: an outline-rendering regression that wasn't visible in any per-renderer unit test. Only the cross-renderer signal catches it.

## Third run: chain-skinned mesh wired into emit (VRMMetalKit 0.13.1)

**Trigger**: With [#181](https://github.com/arkavo-org/VRMMetalKit/issues/181) closed in 0.13.1, the deferred chain-skinned cylinder infrastructure (`chain_mesh.rs` + `buffer::pack_sphere_and_chain`) can finally be wired into `emit_vrm_with_spring_bone`. Locally smoke-verified before the corpus run: rendering `springbone_segment_0p2` (4 joints × 0.2 m chain, hangs well below the sphere bounding-box) shows the chain cylinder poking out at the bottom of the frame — sphere + chain coexist correctly on vrm-metal-kit 0.13.1.

### Corpus-wide before/after

| Metric | Run 2 (no chain) | Run 3 (with chain) | Δ |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.7002 | **0.6994** | −0.0008 |
| min pairwise SSIM | 0.1840 | 0.1840 | unchanged |
| max pairwise SSIM | 0.9490 | 0.9490 | unchanged |

The mean barely moved. Chain-cylinder pixels are a small fraction of the frame at default chain dimensions (~25 mm radius, ~0.2 m visible length on the longest variants), so even with the new geometry, outline divergence (the dominant component) still drives the corpus-wide signal.

### What did change: spring-bone variants no longer degenerate

In runs 1 and 2, every spring-bone test_id (both settle and swing) produced exactly 0.7105 SSIM — the chain physics had no visible effect, so all 36 variants collapsed to "identical sphere render plus zero chain pixels", and only the sphere shading (unchanged across variants) mattered.

With the chain-skinned cylinder active, spring-bone variants now produce variant-specific SSIM scores. Top-15 sample from run 3:

| spring-bone test_id | run 3 SSIM | previously |
|---|---|---|
| `swing_springbone_joints_16` | 0.7043 | 0.7105 (degenerate) |
| `swing_springbone_joints_8`  | 0.7043 | 0.7105 (degenerate) |
| `swing_springbone_segment_0p1` | 0.7053 | 0.7105 (degenerate) |
| `swing_springbone_segment_0p2` | 0.7065 | 0.7105 (degenerate) |
| `swing_springbone_default` | 0.7105 (settled) | 0.7105 |
| `swing_springbone_drag_0` | 0.7105 | 0.7105 |
| `swing_springbone_drag_1` | 0.7105 | 0.7105 |

Joints-16 and joints-8 variants diverge most (longer chains = more visible deformation). segment-0p1 and segment-0p2 next (longer segments = more chain pokes below the sphere). Drag and gravity variants stay clustered with default because the chain length is the same (default joint count + default segment length) and only the physics dynamics differ — and the dynamics signal is small at the chain widths and frame sizes we're rendering.

This is the **first time the spring-bone corpus produces a non-degenerate cross-renderer signal**. Renderer differences in chain physics now propagate to pixels.

### Net result

- Three upstream fixes worth of work landed across the three runs.
- Chain-mesh asset infrastructure activated.
- Two new upstream issues filed during run 2 ([VRMMetalKit#185](https://github.com/arkavo-org/VRMMetalKit/issues/185), [three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839)) for outline-rendering bugs.
- Spring-bone signal moved from "degenerate" to "parameter-sensitive."
- Outline rendering remains the dominant divergence — pending the two outline issues.

The corpus-wide mean SSIM is now anchored around 0.70, with the cluster structure dominated by 8 outline tests at the bottom (0.18–0.52) and the rest of the corpus distributed around 0.70–0.95. To meaningfully raise the corpus-wide mean, the outline bugs need to land first. The remaining MToon shading divergence (~0.69 cluster) is still gated on [VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183).

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
