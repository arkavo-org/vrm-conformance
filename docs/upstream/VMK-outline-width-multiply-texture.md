# VMK — `outlineWidthMultiplyTexture` puts the outline pipeline in a degraded state

**Status**: filed 2026-05-23 as [VMK#289](https://github.com/arkavo-org/VRMMetalKit/issues/289).

---

**Title:** MToon: `outlineWidthMultiplyTexture` triggers a degraded outline pipeline that ignores per-vertex G-channel modulation, `outlineWidthFactor`, AND `outlineWidthMode`

**Labels:** bug, mtoon, materials, outline, spec-compliance

**Body:**

When an MToon material's `outlineWidthMultiplyTexture` is set, VRMMetalKit renders *some* outline (different from a no-texture render) but the outline geometry doesn't respect any of the three axes that should determine its appearance: per-vertex G-channel modulation from the texture itself, the `outlineWidthFactor` scalar, or the `outlineWidthMode` (world-coord vs screen-coord). Three conformance test variants with materially different settings all render to a byte-identical PNG.

Different failure pattern from VMK#287 (`emissiveMultiplier` ignored, no effect) and VMK#288 (`KHR_texture_transform` ignored, no effect). Those two are pure no-ops: the extension is silently dropped. **This one is worse: the texture *is* read, but the codepath that runs is broken in three independent ways.**

## Spec reference

VRMC_materials_mtoon-1.0 README, "outlineWidthMultiplyTexture" section (README.md:710-715):

> The texture to set multiplication factor of outline width.
>
> The components of the texture are stored in linear colorspace.
> **The G component of the texture is referred to.**

(Schema: `VRMC_materials_mtoon.schema.json`'s `outlineWidthMultiplyTexture` is a plain `textureInfo` — no `scale` field, just `index` and optional `texCoord`.)

The composite outline-width formula derived from the spec:

```
per_vertex_outline_width =
    outlineWidthFactor * texture(outlineWidthMultiplyTexture, uv).g
```

And the rendering mode is controlled separately by `outlineWidthMode` (`none` / `worldCoordinates` / `screenCoordinates`) — the same gating that applies to outlines without the multiply texture.

## Reproducer

Conformance corpus at `crates/vrm-asset-generator/src/sweep.rs::mtoon_outline_width_multiply_texture_sweep` (5 variants). Each uses the procedural 16×16 quadrant checkerboard texture (R top-left red, G top-right green, B bottom-left blue, Y bottom-right yellow) — G-channel values per quadrant: red=30/255, green=200/255, blue=30/255, yellow=220/255. So a conformant render should display **thick outlines** at the green and yellow portions of the sphere and **thin outlines** at the red and blue portions.

Rendered through `vrm-runner execute-test-plan --adapter-bin vrm-metal-kit-adapter` on VMK 0.16.0-rc.2 (commit `7f7d39b`) + Apple M4 Max + macOS 26.5:

| test_id | outline_width_mode | outline_width_factor | outlineWidthMultiplyTexture | VMK sha256[:12] | three-vrm sha256[:12] |
|---|---|---|---|---|---|
| `mtoon_outlinewidthtex_baseline` | `worldCoordinates` | `0.05` | absent | `d3cd8b8733f5` | `1626b6a23782` |
| `mtoon_outlinewidthtex_mode_none` | `none` | `0.05` | present | `5d8cf1789282` | `6ff1f5687375` |
| `mtoon_outlinewidthtex_world` | `worldCoordinates` | `0.05` | present | **`acc8b45afb2b`** | `76ae39623713` |
| `mtoon_outlinewidthtex_screen` | `screenCoordinates` | `0.05` | present | **`acc8b45afb2b`** | `e8db0ad6c981` |
| `mtoon_outlinewidthtex_width_2x` | `worldCoordinates` | `0.10` | present | **`acc8b45afb2b`** | `45aacfe46425` |

The three highlighted VMK hashes are byte-identical, even though they vary `outlineWidthMode` (world vs screen) **and** `outlineWidthFactor` (0.05 vs 0.1). three-vrm produces 5 distinct outputs across the same 5 variants.

## What's working (regression guard passes)

`mtoon_outlinewidthtex_mode_none` (texture present + `outlineWidthMode: none`) renders to `5d8cf1789282` — **byte-identical to a no-texture `mtoon_default` render**. So VMK correctly gates the binding on `outlineWidthMode != none`. The codepath isn't running when it shouldn't.

## What's broken

When `outlineWidthMultiplyTexture` is present AND `outlineWidthMode != none`, VMK enters a degraded outline pipeline that:

1. **Ignores per-vertex G-channel modulation.** A conformant render would show variable-width outlines following the checkerboard's quadrant boundaries. VMK's output is uniform-width (per the byte-identical hashes between variants that should differ along the modulation axis).
2. **Ignores `outlineWidthFactor`.** `width_2x` (factor 0.10) should produce visibly thicker outlines than `world` (factor 0.05). Same hash.
3. **Ignores `outlineWidthMode`.** `world` (world-coord, perspective-scaled outlines) and `screen` (screen-coord, post-projection constant-pixel-width outlines) should produce visibly different outline geometry. Same hash.

The texture-present rendering does *something* different from the no-texture baseline (`acc8b45...` ≠ `d3cd8b8...`), so the codepath that handles the multiply-texture case exists. It's just not implementing any of the three axes the spec says it should.

## Where the gap likely is

Speculative without VMK source access, but the failure pattern suggests:
- VMK's MToon material parser **does** read `outlineWidthMultiplyTexture` (otherwise the no-texture/texture hash would match, like VMK#287/#288).
- A separate codepath fires when this texture is present that uses a hardcoded outline width and ignores both factor and mode.
- Possibly an early-out or "if texture present, use texture-only path" branch that was intended to be additive but became substitutive.

## Suggested fix

The composite outline width per vertex should be:

```swift
let multiplier: Float = outlineWidthMultiplyTexture
    .map { sampleG(it, at: vertex.uv) }
    ?? 1.0
let width = outlineWidthFactor * multiplier
// then dispatch width through the existing world vs screen pipeline
// per outlineWidthMode (don't replace those pipelines — multiply only
// modulates the width input).
```

Threading the multiply factor into the existing outline-dispatching code path (rather than replacing it) should restore conformance on all three axes simultaneously.

## Filer context

Surfaced by [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) commit `136e7df` (the new outlineWidthMultiplyTexture sweep) on its first cross-renderer run. Details:

- Sweep code: `crates/vrm-asset-generator/src/sweep.rs::mtoon_outline_width_multiply_texture_sweep`
- Findings entry: `docs/findings.md` "MToon outlineWidthMultiplyTexture — VMK partial-broken (new gap); three-vrm conformant"

After the fix, the conformance suite picks it up automatically — `mtoon_outlinewidthtex_*` test_ids will produce 5 distinct hashes cross-validating against three-vrm.

## Related VMK issues filed by this suite

- [VMK#287](https://github.com/arkavo-org/VRMMetalKit/issues/287) — `VRMC_materials_hdr_emissiveMultiplier` ignored (pure no-op)
- [VMK#288](https://github.com/arkavo-org/VRMMetalKit/issues/288) — `KHR_texture_transform` on textureInfo ignored (pure no-op)
- **This issue** — different shape: extension read, codepath broken
