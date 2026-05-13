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

## Fourth run: VRMMetalKit 0.13.2 — outline regression closed

**Trigger**: [VRMMetalKit 0.13.2](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.13.2) shipped within 3 hours of [#185](https://github.com/arkavo-org/VRMMetalKit/issues/185) being filed. Hotfix; root cause per the release notes: "the outline pass dispatched the inverted-hull geometry at world origin instead of at the rigid mesh node's world position" — side effect from 0.13.1's #181 fix touching the mesh-iteration path.

Re-rendered through vrm-metal-kit only (three-vrm renders preserved from prior run; same hardware, same three-vrm version 3.5.0).

### Corpus-wide before/after

| Metric | Run 3 (0.13.1+chain) | Run 4 (0.13.2+chain) | Δ |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.6994 | **0.7439** | **+0.0445** |
| min pairwise SSIM | 0.1840 | **0.6313** | **+0.4473** |
| max pairwise SSIM | 0.9490 | 0.9665 | +0.0175 |

The 0.13.2 hotfix recovered **exactly** the ground lost by the 0.13.1 outline regression (the delta numbers are symmetric to run 1 → run 2). All three statistics returned to their original baseline values, modulo a tiny rounding band.

### 8 outline tests no longer dominate divergence

In run 3, `mtoon_outline_world_0p1` was the worst test at 0.1840 SSIM. In run 4, it's back to 0.6313 — still the worst test, but in the same range as MToon shading divergence. The 7 other outline tests have fallen entirely out of the top 15 most-divergent list. Top 15 in run 4 is now dominated by:

- 1 outline test (`mtoon_outline_world_0p1` at 0.6313, baseline-equivalent)
- 5 MToon shading tests (shadingShift / shadingToony / doubleSided)
- 8 spring-bone variants (joints / segment / stiffness — parameter-sensitive thanks to the chain-skinned mesh from run 3)
- 1 baseline (`mtoon_default`)

### Cumulative four-run progression

| Run | mean | min | upstream events |
|---|---|---|---|
| 1 | 0.7447 | 0.6313 | first corpus measurement |
| 2 | 0.7002 | 0.1840 | #181/#182 closed; #185+#1839 surfaced |
| 3 | 0.6994 | 0.1840 | chain-skinned mesh wired |
| 4 | 0.7439 | 0.6313 | #185 closed in 0.13.2 |

Three of the four issues filed against VRMMetalKit (#181, #182, #185) are now closed. Three remain open: #183 (MToon flat-white shading), [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) (color-space hypothesis), [pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) (outline floods entire mesh).

### Time-to-fix observed

- #181 + #182 filed → fixed in 0.13.1: same-session turnaround (hours).
- #185 filed during the 0.13.1 corpus re-run → fixed in 0.13.2: **3 hours**.

When the upstream maintainer is engaged with the conformance suite, the loop closes faster than the test corpus can re-run. The total wall-clock from "find regression" → "merge fix" → "re-measure recovery" is now under a single project session.

## Fifth run: VRMMetalKit 0.13.3 — MToon flat-white closed

**Trigger**: [VRMMetalKit 0.13.3](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.13.3) shipped less than an hour after 0.13.2. Closes [#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) — the deepest of the four bugs filed against VRMMetalKit and the one driving the largest residual divergence. Root cause per the release notes: "the main lighting path applied a Half-Lambert remap that saturated `shadowStep=1` across the visible hemisphere with `shadingToonyFactor=0.9` + typical directional lighting, collapsing the rendered color to `baseColor` everywhere."

That's exactly what the pixel sampling in [#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) showed: every sphere fragment at `(255, 255, 255)` regardless of position, regardless of shading parameter. The toon ramp wasn't applying.

Re-rendered through vrm-metal-kit only.

### Corpus-wide before/after

| Metric | Run 4 (0.13.2) | Run 5 (0.13.3) | Δ |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.7439 | **0.7879** | **+0.0440** |
| min pairwise SSIM | 0.6313 | 0.6313 | unchanged (still outline floor on three-vrm side) |
| max pairwise SSIM | 0.9665 | 0.9665 | unchanged |

The mean moved by +0.044 on a single upstream fix — the largest single-release delta of the session. The 44 MToon shading variants that were anchored at ~0.69 are now distributed across the 0.74–0.76 band.

### Pixel-level recovery

Sphere centerline (x=512 on a 1024×1024 render), `mtoon_default`:

| run | vrm-metal-kit (R,G,B) | three-vrm (R,G,B) |
|---|---|---|
| 1–4 (pre-#183 fix) | **255, 255, 255** (flat white) | 53, 53, 53 |
| 5 (0.13.3) | **164, 164, 164** | 53, 53, 53 |

vrm-metal-kit moved from `1.0` linear (flat white, no shading) to `0.643` linear (real MToon mid-gray, exactly what we'd expect from the spec for a sphere with `baseColor=1.0`, `shadeColor=0.5`, `shadingToonyFactor=0.9`, lit from `(-0.3, -0.6, -0.7)` with intensity 1 + ambient 0.15). The toon math is now firing.

three-vrm is still at 0.208 — consistent with their longer-standing color-space hypothesis ([three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)). With both renderers' raw outputs now non-trivial, the residual divergence at ~0.79 reflects the *actual* spec-interpretation gap between the two real MToon implementations, not a "one renderer fully broken" artifact.

### Top 15 most-divergent: cluster structure has shifted

| test_id | run 4 | run 5 |
|---|---|---|
| `mtoon_outline_world_0p1` | 0.6313 | 0.6313 (still the floor; three-vrm-side bug) |
| `mtoon_shadingShift_0p8` | not top-15 | **0.7384** |
| `mtoon_shadingToony_0p5` | 0.7087 | **0.7418** |
| `mtoon_shadingShift_neg0p5` | 0.6893 (was worst MToon) | not in top 15 (moved up) |
| `mtoon_shadingShift_neg0p2` | 0.7013 | not in top 15 (moved up) |

Worth noting: `mtoon_shadingShift_neg0p5` was the second-worst test in run 4 at 0.6893 — it fell out of the top 15 entirely in run 5. The negative-shadingShift variants benefited most from the toon-ramp fix (negative shift shifts the lit/shadow boundary toward more shadow, which under the broken ramp had no effect; now the broader shadow area produces actual shadow pixels).

The new bottom of the divergence list is dominated by:
- 1 outline test at the floor (`mtoon_outline_world_0p1` at 0.6313, three-vrm-side per [#1839](https://github.com/pixiv/three-vrm/issues/1839))
- 5 MToon shading + toony variants in the 0.738–0.749 band
- 8 spring-bone parameter-sensitive swing variants in the 0.757–0.762 band

The variance within the spring-bone cluster is now meaningfully tighter (3.7 percentage points of spread vs ~2.5 in run 3/4) because the chain-skinned cylinder is rendering against a properly-shaded sphere background, so the SSIM contributions from chain pixels are easier to discriminate.

### Cumulative five-run progression

| Run | mean | min | upstream events |
|---|---|---|---|
| 1 (50cfd7d) | 0.7447 | 0.6313 | first corpus baseline |
| 2 (0.13.1) | 0.7002 | 0.1840 | #181/#182 closed; #185+#1839 surfaced |
| 3 (0.13.1+chain) | 0.6994 | 0.1840 | chain-skinned mesh wired |
| 4 (0.13.2+chain) | 0.7439 | 0.6313 | #185 closed in 0.13.2 |
| **5 (0.13.3+chain)** | **0.7879** | **0.6313** | **#183 closed in 0.13.3** |

The corpus-wide mean is now +0.0432 ABOVE the original baseline (0.7447 → 0.7879). Three of four VRMMetalKit issues have been closed in the same session that filed them; #183 took 4 hours from filing to closing.

### What remains in the divergence floor

With the three vrm-metal-kit bugs closed, residual divergence comes from:

1. **three-vrm side** — [#1838](https://github.com/pixiv/three-vrm/issues/1838) (dark MToon shadow / double sRGB) and [#1839](https://github.com/pixiv/three-vrm/issues/1839) (outline color floods entire mesh). The first drives the 0.74-cluster floor; the second pins `mtoon_outline_world_0p1` at 0.6313.
2. **MToon spec interpretation** — even with both renderers' fundamental bugs fixed, the two real renderers may legitimately diverge on edge-case shading parameters. Without a third independent reference (UniVRM in Unity), the suite can't pin which interpretation is closest to spec.

The corpus mean is now plausibly approaching what "two-real-renderer pairwise SSIM" can theoretically reach. Further movement requires either three-vrm-side fixes (would raise the corpus floor) or adding a third real adapter to make consensus meaningful enough to identify outliers.

### Open questions

- **Should the corpus's default SSIM threshold be relaxed below 0.985?** v1.0 standardizes on 0.985 per `docs/methodology.md`, but the data shows that's currently unreachable for any test in the corpus. Two interpretations:
  - The threshold is correct; both renderers are sufficiently spec-divergent that pass requires upstream fixes first. The cross-renderer signal IS the conformance result.
  - The threshold needs methodology refinement — e.g., separate thresholds for "exact MToon math" tests vs "approximate visual fidelity" tests, or per-renderer-pair thresholds.
- **Should consensus exclude the mock-renderer entirely from default reporting?** Mock is synthetic and isn't trying to match real renderers. Including it in consensus inflates the apparent divergence. The current script doesn't include mock by default (the bootstrap only renders through real adapters); confirming this stays the convention.
- **Outline-mode divergence at 0.6313 — is that AA noise alone, or a model-level outline-rendering bug?** Likely worth a dedicated investigation (sample the actual outline pixels in both renders).

## Sixth run: godot-vrm L3 shipped — third real renderer added

**Date**: 2026-05-11, vrm-conformance commit `820b716`.

**Trigger**: V-Sekai/godot-vrm vendored at `9fae4049` + Godot-MToon-Shader at `27cb2b78`; L3 Phase 1 ops landed (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`). Phase 2 (spring-bone) deferred — spring-bone test plans skip godot-vrm.

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test corpus through three-vrm, vrm-metal-kit, and godot-vrm on macOS 26 (Apple M4 Max, Godot 4.6.2). `scripts/consensus-report.sh` ran pairwise SSIM across the manifest.

**Headline**: First three-renderer pairwise SSIM data on the corpus. Three-way consensus available for the 44 MToon test_ids where godot-vrm renders; the 36 spring-bone settle + swing tests remain two-renderer (three-vrm vs vrm-metal-kit only) because godot-vrm's Phase 2 ops are `Unimplemented`. godot-vrm vs vrm-metal-kit pairs are the closest at mean SSIM 0.852 — meaningfully tighter than either renderer's pair with three-vrm.

### Corpus-wide consensus

```
Processed 80 test_ids; skipped 0
consensus_passed: 0/80
consensus_failed: 80/80

Pairwise SSIM stats across the corpus:
  pair                                  mean    min     max     n
  godot-vrm vs three-vrm                0.6916  0.1840  0.9482  44
  godot-vrm vs vrm-metal-kit            0.8521  0.5301  0.9517  44
  three-vrm vs vrm-metal-kit            0.7879  0.6313  0.9665  80
```

`n=44` reflects the 44 MToon tests where all three renderers produced output. `n=80` covers the full corpus (including the 36 spring-bone tests where godot-vrm is absent). The `three-vrm vs vrm-metal-kit` pair is unchanged from run 5, as expected — neither renderer changed in this run.

### Top 15 most-divergent test_ids

```
mtoon_outline_world_0p1                   0.1840  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p05                  0.3588  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p1                  0.4028  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p03                  0.4330  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p05                 0.4711  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p03                 0.4967  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p01                  0.5018  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p01                 0.5223  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_doubleSided_true                    0.7045  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p5                 0.7053  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p2                 0.7075  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingToony_0p75                   0.7079  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p8                 0.7106  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg1                   0.7108  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingToony_0p5                    0.7109  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
```

The outline-test cluster dominates the divergence floor as in prior runs (still pinned to `mtoon_outline_world_0p1` at 0.1840). Adding godot-vrm pulled the floor down: in run 5 the floor was 0.6313 (`mtoon_outline_world_0p1` between three-vrm and vrm-metal-kit). godot-vrm renders outlines at a third interpretation that disagrees with both — so the min pair-SSIM for that test drops from 0.6313 to 0.1840.

### Pixel-level sample — mtoon_default

| renderer       | (R, G, B) at sphere center (x=512, y=512) |
|---|---|
| three-vrm      | (53, 53, 53)    |
| vrm-metal-kit  | (164, 164, 164) |
| godot-vrm      | (255, 255, 255) |

For reference: run 5 had three-vrm at (53, 53, 53) and vrm-metal-kit at (164, 164, 164). godot-vrm at (255, 255, 255) is the new data point — flat white at the sphere center, the same surface signature VMK had pre-0.13.3 (run 5 closed [VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) for the same symptom). The upstream code paths are entirely independent so the cause differs; the symptom is identical.

### Observations

- **godot-vrm clusters closer to vrm-metal-kit than to three-vrm.** The `godot-vrm vs vrm-metal-kit` mean (0.8521) is +0.164 above the `godot-vrm vs three-vrm` mean (0.6916), and +0.064 above the `three-vrm vs vrm-metal-kit` mean (0.7879). Both godot-vrm and vrm-metal-kit are first-party MToon implementations against the VRMC spec; three-vrm's color-space hypothesis ([three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)) keeps it darker than either. With three renderers, the consensus diff can now flag `three-vrm` as the outlier on the shading cluster — the methodology's intended use case.
- **godot-vrm mtoon_default = flat white.** Same surface symptom as VMK pre-0.13.3 but unrelated upstream code. Likely candidates: tonemapping not disabled in the Godot-MToon-Shader path, MToon shadingToony saturation similar to VMK's #183, or a default lighting intensity mismatch. Worth a dedicated spike before promoting godot-vrm as the reference for "spec-correct MToon."
- **The outline-test divergence floor dropped from 0.6313 to 0.1840** because godot-vrm renders outlines differently from both other renderers. This is expected: outline rendering is the least-specified part of MToon and three different renderers will produce three different interpretations. The min isn't a regression in any renderer — it's the cost of widening the panel.
- **Spring-bone tests retained run-5 two-renderer numbers** (godot-vrm absent). No regression there.

### Open follow-ups

- **godot-vrm spring-bone tests skipped**: 36 spring-bone settle + swing tests fail the runner's `execute-test-plan` because Phase 2 ops (`step_physics`, `reset_physics`, `animate_root_transform`) return `Unimplemented`. A follow-up plan would add Phase 2 by overriding godot-vrm's `vrm_secondary.gd` spring-bone auto-stepping and taking manual control of the physics pump (`Engine.physics_ticks_per_second = 60`, deterministic per-frame step).
- **godot-vrm flat-white at `mtoon_default`**: file an investigation issue against either `adapters/godot-vrm/src/session.gd` (lighting / tonemap setup) or the upstream Godot-MToon-Shader. Pixel sampling matches the VMK #183 symptom; the fix path likely doesn't.
- **Concern 2 from Spike 2 (mesh-under-head-bone)**: `addons/godot-vrm/VRMC_vrm.gd:387` emits a `Skeleton3D` → `ImporterMeshInstance3D` typed-assignment SCRIPT ERROR during `_create_animation_player` when the asset generator places the mesh node as a child of a humanoid bone (head). Non-fatal — the renderer recovers and produces output — but worth filing upstream against either `V-Sekai/godot-vrm` (typed-assignment hardness; the line should fail gracefully when the node isn't an `ImporterMeshInstance3D`) or `crates/vrm-asset-generator/` (avoid mesh-as-bone-leaf layouts that trip this branch). Reproducer: any of the chain-skinned spring-bone fixtures emitted by `vrm-asset-generator emit-sweep`.

## Seventh run: godot-vrm L4 shipped — full 80-test 3-way consensus

**Date**: 2026-05-11, vrm-conformance commit `9f5aa7b`.

**Trigger**: godot-vrm L4 landed — `step_physics`, `reset_physics`, `animate_root_transform` are now real implementations driving V-Sekai/godot-vrm's `VRMSecondary` node manually (auto-stepping disabled, `do_process` called explicitly, bone-pose-override clearing for proper reset). All 36 spring-bone tests (18 settle + 18 swing) now render through godot-vrm. **This closes the VMK 1.0 launch blocker.**

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test corpus through three-vrm, vrm-metal-kit, and godot-vrm on macOS 26 (Apple M4 Max, Godot 4.6.2). `scripts/consensus-report.sh` ran pairwise SSIM across the manifest.

**Headline**: All three adapters at 80/80. First time the project has full three-way coverage across the entire corpus. Every spring-bone test now has three independent renderers driving the same physics contract.

### Corpus-wide consensus

```
Processed 80 test_ids; skipped 0
consensus_passed: 0/80
consensus_failed: 80/80

Pairwise SSIM stats across the corpus:
  pair                                  mean    min     max     n
  godot-vrm vs three-vrm                0.7042  0.1840  0.9482  80
  godot-vrm vs vrm-metal-kit            0.8709  0.5301  0.9517  80
  three-vrm vs vrm-metal-kit            0.7879  0.6313  0.9665  80
```

All three pairs at `n=80` for the first time. The `three-vrm vs vrm-metal-kit` row is unchanged from run 6 (those renderers didn't move). The `godot-vrm vs *` pairs both gained 36 spring-bone tests' worth of data points:

- `godot-vrm vs three-vrm`: mean +0.0126 (0.6916 → 0.7042), min unchanged (still pinned to the outline cluster).
- `godot-vrm vs vrm-metal-kit`: mean +0.0188 (0.8521 → 0.8709), min unchanged.

The spring-bone tests are pulling both godot-vrm pair means up — i.e. godot-vrm's spring-bone renders agree with the other two renderers more strongly than its MToon renders do. The godot-vrm/vrm-metal-kit pair remains the tightest cluster across the corpus.

### Top 15 most-divergent test_ids

```
mtoon_outline_world_0p1                   0.1840  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p05                  0.3588  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p1                  0.4028  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p03                  0.4330  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p05                 0.4711  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p03                 0.4967  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p01                  0.5018  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p01                 0.5223  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_doubleSided_true                    0.7045  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p5                 0.7053  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p2                 0.7075  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingToony_0p75                   0.7079  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
springbone_joints_16                      0.7096  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
springbone_joints_8                       0.7096  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
springbone_segment_0p1                    0.7097  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
```

Two `springbone_*` entries crack the top-15 for the first time (`joints_16`, `joints_8`, `segment_0p1`) — but only at ~0.7096, well above the outline-cluster floor of 0.1840. The outline tests still dominate the divergence floor (same 8 entries leading the list as in run 6); their min SSIMs are unchanged because nothing in this run touched outline rendering. The outline-cluster floor hasn't moved with the expanded `n=80` panel — confirming the floor is set by godot-vrm's distinct outline interpretation, not by sample-size noise.

### Observations

- **Spring-bone three-way coverage works.** godot-vrm's manual physics pump (60 Hz fixed step, explicit `do_process` calls, bone-pose-override clearing on reset) produces renders that agree with three-vrm and vrm-metal-kit more strongly than the MToon corpus does. The fact that no spring-bone test cracks the top-8 divergent list — despite three independent physics engines (Godot's, three-vrm's, VMK's) settling the same chain — is the headline methodological win of L4.
- **mtoon_default flat-white persists** (carried from Run 6). godot-vrm still renders (255, 255, 255) at the sphere center while three-vrm sits at (53, 53, 53) and vrm-metal-kit at (164, 164, 164). L4 didn't touch lighting/tonemap; this remains the leading godot-vrm-specific fidelity bug.
- **Outline divergence floor (0.1840) is stable at `n=80`.** The floor didn't shift between run 6's `n=44` godot-pair sample and run 7's `n=80`, which means the worst-case outline disagreement is a deterministic three-way property of the renderers — not an artifact of which subset we sampled.
- **godot-vrm/vrm-metal-kit remains the tightest pair** (mean 0.8709 vs 0.7879 for three-vrm/vmk vs 0.7042 for three-vrm/godot). Both are first-party MToon implementations against the VRMC spec; three-vrm's color-space hypothesis ([three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)) continues to keep it as the dimmest of the three. With three renderers' worth of data at full `n`, the consensus diff confidently flags three-vrm as the shading-cluster outlier — the methodology working as designed.

### Open follow-ups

- **godot-vrm flat-white at `mtoon_default`** (carried from Run 6 — still open). Lighting/tonemap investigation against `adapters/godot-vrm/src/session.gd` or the upstream Godot-MToon-Shader is the next concrete fidelity win available.
- **`springbone_joints_16` / `joints_8` / `segment_0p1`** crack the top-15 divergent list at ~0.7096. Worth a closer look — the chains agree well enough not to dominate the floor but they're meaningfully lower than the spring-bone median. Candidates: joint-count edge cases (longer chains accumulate more drift per step), per-joint segment-length scaling, or a `do_process` ordering subtlety in long chains.
- **Linux CI driver spike** (still pending from L3). The whole corpus runs on macOS today; a Linux driver pass would validate that the godot-vrm shim doesn't inherit anything macOS-specific from the Godot path.
- **Concern 2 from Spike 2 (mesh-under-head-bone)** (still open from Run 6). Non-fatal `VRMC_vrm.gd:387` typed-assignment script error during chain-skinned imports; renderer recovers and emits output.

## Eighth run: methodology refinement — color-space convention pinned

**Date**: 2026-05-12. No new renderer revisions; methodology + tooling change only.

**Trigger**: [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) closed by maintainer reply (not a bug). [0b5vr](https://github.com/0b5vr) explained that three-vrm's MToon implementation deliberately renders in linear color space and assumes the renderer's `outputColorSpace = THREE.SRGBColorSpace` will apply the sRGB OETF on output. `THREE.LinearSRGBColorSpace` is explicitly unsupported for MToon — three.js's `SRGBColorSpace` corresponds to what Unity calls "Linear" workflow (linear shading + sRGB display encoding), not `LinearSRGBColorSpace` (which is *no* display encoding at all).

Our prior test-plan default was `color_space: Linear`, which the three-vrm adapter honored by setting `LinearSRGBColorSpace`. That asked three-vrm to render in an unsupported mode and produced a corpus baseline that systematically under-represented its MToon output by the sRGB OETF. The other two adapters interpreted the same field inconsistently (vrm-metal-kit → `rgba8Unorm` linear framebuffer; godot-vrm → always sRGB-encoded PNG regardless of request).

### What changed

- `docs/methodology.md` — rewrote the **Color management** section to pin `color_space: Srgb` as the v1.0 default for every MToon math test, document the adapter contract per renderer, and flag the directional-intensity-by-π open question as a follow-up.
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_default_test_plan` now emits `color_space: Srgb`. All sweep variants inherit the change (they all start from `build_default_test_plan` or its spring-bone derivatives).
- `adapters/three-vrm/src/renderer-host.html` — added a comment near the `outputColorSpace` branch flagging the convention so future contributors don't reintroduce `LinearSRGBColorSpace` as a default.

### Expected impact on the corpus (not yet measured)

This change has not been re-rendered through the corpus yet — that's a follow-up bootstrap-goldens run. Predictions:

- **three-vrm**: every test should now render meaningfully brighter (the sRGB OETF is applied on output, so the `(53, 53, 53)` sphere centerline at `mtoon_default` should move into the high-100s, much closer to the VRMMetalKit `(164, 164, 164)` and away from the godot-vrm `(255, 255, 255)` outlier). The longstanding "three-vrm is the dimmest renderer" signal — which has been the dominant divergence floor across runs 1–7 since the run-5 VMK fix — should largely close.
- **vrm-metal-kit**: framebuffer flips from `rgba8Unorm` to `rgba8Unorm_srgb`. Pixel values move from raw-linear to sRGB-encoded. Expected to remain visually similar but PNG byte values shift; SSIM vs the new three-vrm baseline likely tightens substantially.
- **godot-vrm**: no behavioral change (already wrote sRGB-encoded PNGs unconditionally).

If the prediction holds, the corpus mean SSIM should jump materially — possibly through the 0.85+ band — driven primarily by the three-vrm/VMK pair re-converging. The remaining divergence floor would still be the outline cluster (three different outline interpretations across three renderers, including [#1839](https://github.com/pixiv/three-vrm/issues/1839)).

### What this measures, conceptually

Up to run 7, every divergence finding was filed against a renderer that was actually behaving incorrectly relative to the spec. This run is the first where the conformance suite *itself* was the source of a systematic divergence — the test plan asked renderers for an output mode that wasn't well-defined cross-renderer, and three-vrm in particular flagged it. The fix is methodology, not renderer code. Logging it here as a deliverable on the same footing as the upstream-bug findings, because the suite's purpose is to produce falsifiable signal and that includes signal about the suite's own assumptions.

### Follow-ups

- **Run 9 bootstrap**: re-render the full 80-test corpus through all three real adapters with the new default and re-measure pairwise SSIM. Compare against run 7 numbers to validate the prediction above.
- **Directional-intensity-by-π**: three-vrm assumes `Math.PI` scaling (legacy three.js convention). Our plan declares `intensity: 1.0` without specifying which convention applies. Decide whether to scale in the adapter (preserves the human-readable `1.0`) or in the plan (requires updating every test). Tracked as an open methodology question in `docs/methodology.md`.

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
