# VMK — MToon function-constant specialization (#193) is pixel-faithful: sub-perceptual LSB drift on feature paths only, no action needed

**Status**: verified 2026-06-13 against [VRMMetalKit 0.21.0-rc.1](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.21.0-rc.1) (`1ebe2ab`) vs the 0.20.1 pin (`39e65f0`). **Positive confirmation, not a defect** — the specialization changes no shader math; the only observable effect is last-bit floating-point drift on MToon feature paths, all far below the perceptual gate. The suite will re-baseline the affected golden cells when the conformance pin lands on 0.21.x; no VMK change requested.

---

**Title:** MToon function-constant specialization (#193) verified pixel-faithful on the conformance corpus — 397/428 byte-identical, the 31 feature-path cells all SSIM ≥ 0.9974

**Labels:** mtoon, perf, conformance, verification, no-action

**Body:**

Regression-checked the [#193 MToon function-constant specialization](https://github.com/arkavo-org/VRMMetalKit/pull/) (commit `40c5fe9`, shader hash refreshed in `ac1e6b5`) as part of the 0.21.0-rc.1 pin verification. Built the `vrm-metal-kit` adapter from each pin on the same machine (Apple M4 Max, macOS 26.5 / Xcode 26.5 / Swift 6.3.2), rendered the full 428-plan local corpus through each, BLAKE3 byte-compared every output A/B, and ran SSIM on every byte-divergent cell.

## Result

- **397 / 428 outputs byte-identical**; 0 missing/failed on either side.
- **31 cells differ at the byte level, every one sub-perceptual** — SSIM(base, rc) ∈ **[0.99739, 1.000000]** against the suite's 0.85 gate (all `ssim_passed: true`). The `rimLightingMix` family is SSIM = 1.000000 (byte-differs, perceptually identical).
- Every byte-divergent cell is **MToon-shading-feature-bearing**; nothing else drifts. By family: shadingShift / shadeShift scalar (6), rimLightingMix scalar (5), shadeMultiplyTexture (5), rimMultiplyTexture (4), glTF-core normal+occlusion PBR textures (3), KHR_texture_transform / uvxform (8). Plain/default MToon, all spring-bone families, all VRMA, first-person, and the `outlineWidthMultiplyTexture` cells are bit-for-bit unchanged.
- Worst three: `mtoon_shadetex_default` 0.99739, `mtoon_shadingShift_1` 0.99810, `mtoon_shadetex_shift_pos0p5` 0.99848. SSIM's locality rules out a concentrated artifact behind a high global score.
- **Determinism intact**: `swing_springbone_joints_16` (the historical VMK#283 reproducer) rendered 5× byte-identical (`9e9a6c3d8ba6…`) on the RC binary. Adapter `swift test` 36 tests / 0 failures.

## Why this is expected, and not a regression

The `.metal` change is purely structural. Each edit is `if (material.hasX > 0)` → `if (effectiveHasX)`, where

```metal
bool effectiveHasShadeMultiplyTexture =
    fc_useMaterialFlags ? (material.hasShadeMultiplyTexture > 0)   // dynamic fallback — identical to before
                        : fc_hasShadeMultiplyTexture;              // specialized pipeline
```

No sampling, multiply, shading-shift, or occlusion math is altered. `MToonFunctionConstantKey.init(material:)` keys each specialized variant off the material's real texture flags, and `VRMRenderer+Pipeline.swift` falls back to the dynamic path on pipeline-creation failure — so variant selection is correct (a mis-keyed variant would be gross, not sub-LSB).

The drift is the **Metal compiler scheduling/contracting FP ops differently for the constant-specialized variants**: once a feature branch constant-folds to "always taken," the surrounding instruction selection and FMA contraction change. On macOS the MToon fragment runs in FP16 (`mtoon_float = half`, from the #279/0.18 FP16 path), which makes that reordering visible at the last bit — but only on the feature branches, which is exactly the 397-identical / 31-feature-cells pattern observed. Both pins are equally-valid roundings of identical math; neither is "more correct," and the shift is far below anything that moves conformance vs. the UniVRM reference.

## Recommendation

No VMK action. The specialization is correct and the perf win (dead-stripping unused texture samples) is worth the ≤1-LSB drift on toon-shaded feature paths — forcing bit-stability would require pinning FP contraction or reverting the specialization, an over-constraint for non-PBR output. The conformance suite owns the close-out: re-baseline the 31 MToon-feature golden cells at the 0.21.x pin (tracked in `docs/findings.md`, 2026-06-13 entry, and the `adapters/vrm-metal-kit/Package.swift` pin comment when it bumps).
