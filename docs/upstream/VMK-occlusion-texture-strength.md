# VMK — `occlusionTexture` silently dropped on MToon materials (sibling to VMK#290)

**Status**: filed 2026-05-23 as [VMK#293](https://github.com/arkavo-org/VRMMetalKit/issues/293); **closed 2026-05-23 in 0.16.0-rc.4** (commit `81ebce6`, PR #296). Verified on rc.4: `mtoon_pbrtex_occlusion_default` (`e24bff37139b`) and `mtoon_pbrtex_occlusion_strength_half` (`17a817fdda2b`) now both distinct from the no-occlusion baseline (`5d8cf1789282`); `mtoon_pbrtex_combined` also picks up the occlusion contribution (`6a6c35376509` vs rc.3's identity-with-normal `a599ae818d08`).

---

**Title:** MToon: glTF-core `occlusionTexture` is silently dropped — strength field AND texture binding both ignored (sibling to VMK#290)

**Labels:** bug, mtoon, materials, gltf-core, spec-compliance

**Body:**

Surfaced while verifying [VMK#290](https://github.com/arkavo-org/VRMMetalKit/issues/290)'s closure on 0.16.0-rc.3 — the same `mtoon_pbr_textures_sweep` corpus exercises `occlusionTexture.strength` as an independent axis from `normalTexture.scale`. On rc.3, both occlusion variants (`strength=1.0` default and `strength=0.5`) render to a **byte-identical PNG to the no-occlusion baseline**, indicating the entire `occlusionTexture` binding is silently dropped on the MToon path — not just the `strength` field.

Different shape from VMK#290 (where the normal texture *was* applied at `scale=1.0`-equivalent perturbation but the `scale` multiplier was lost). Here the texture itself appears absent: there is no per-quadrant darkening at all from the procedural occlusion map.

## Spec reference

glTF 2.0 [`material.occlusionTextureInfo.schema.json`](https://github.com/KhronosGroup/glTF/blob/main/specification/2.0/schema/material.occlusionTextureInfo.schema.json):

```json
{
    "$id": "material.occlusionTextureInfo.schema.json",
    "title": "Material Occlusion Texture Info",
    "allOf": [ { "$ref": "textureInfo.schema.json" } ],
    "properties": {
        "strength": {
            "type": "number",
            "description": "A scalar parameter controlling the amount of occlusion applied.",
            "default": 1.0,
            "minimum": 0.0,
            "maximum": 1.0,
            "gltf_detailedDescription": "A scalar multiplier controlling the amount of occlusion applied. A value of `0.0` means no occlusion. A value of `1.0` means full occlusion. This value affects the final occlusion value as: `1.0 + strength * (<sampled occlusion texture value> - 1.0)`."
        }
    }
}
```

VRMC_materials_mtoon-1.0 inherits `occlusionTexture` from glTF-core (MToon does not redefine it). The R channel of the texture provides per-fragment ambient occlusion, modulated by `strength`. The spec is unambiguous: `strength=0` should produce no occlusion (identical to no texture); `strength=1` should produce full occlusion (darkest); intermediate values should produce intermediate darkening. The texture itself should always be visible at `strength=1.0` (the default).

## Reproducer

Conformance corpus at [`crates/vrm-asset-generator/src/sweep.rs::mtoon_pbr_textures_sweep`](https://github.com/arkavo-org/vrm-conformance/blob/main/crates/vrm-asset-generator/src/sweep.rs) — same 6-variant sweep that exposed VMK#290. Two variants are occlusion-specific. Synthetic VRM 1.0 avatar with a procedural 16×16 occlusion map attached to `pbrMetallicRoughness.occlusionTexture`. Per-quadrant occlusion values: TL=0.1 (heavy occlusion), TR=0.3, BL=0.7, BR=1.0 (no occlusion).

Rendered through `vrm-runner execute-test-plan --adapter-bin vrm-metal-kit-adapter` on VMK 0.16.0-rc.3 (commit `8cd3bc9`) + Apple M4 Max + macOS 26.5:

| test_id | declared `strength` | vrm-metal-kit rc.3 sha256[:12] | size |
|---|---|---|---|
| `mtoon_pbrtex_baseline` (no `occlusionTexture`) | — | `5d8cf1789282` | 49634 B |
| `mtoon_pbrtex_occlusion_default` | `1.0` | `5d8cf1789282` | 49634 B |
| `mtoon_pbrtex_occlusion_strength_half` | `0.5` | `5d8cf1789282` | 49634 B |

All three are byte-identical, including the no-texture baseline. The occlusion texture has no observable effect on the rendered output.

For contrast, the same sweep's normal-map axis on rc.3 produces:

| test_id | rc.3 sha256[:12] | size | result |
|---|---|---|---|
| `mtoon_pbrtex_baseline` (no normal) | `5d8cf1789282` | 49634 B | baseline |
| `mtoon_pbrtex_normal_default` (scale=1) | `a599ae818d08` | 68883 B | distinct from baseline ✓ (texture applied) |
| `mtoon_pbrtex_normal_scale_2x` (scale=2) | `8c30c0e22cdf` | 74009 B | distinct from default ✓ (VMK#290 closure) |

So the rc.3 MToon pipeline correctly threads the per-textureInfo `scale` field for normal maps after PR #291, but the occlusionTexture binding doesn't reach the shader at all — neither the texture nor its `strength` field.

## What's broken

`occlusionTexture` (a glTF-core textureInfo on the material root, not under `pbrMetallicRoughness`) appears to be silently dropped in the MToon material parser. The R-channel ambient-occlusion contribution and the per-textureInfo `strength` field are both missing from the rendered output.

## Suggested fix

Mirror the normal-texture wiring from PR #291's VMK#290 closure, but for occlusion. In the material parser:

```swift
let occlusionTexture: TextureBinding? = parseOcclusionTexture(from: material)
let occlusionStrength: Float = (material.occlusionTexture?["strength"] as? Double)
    .map(Float.init)
    ?? 1.0
```

Then in the MToon fragment shader, after sampling base color:

```metal
if (hasOcclusionTexture) {
    float ao = occlusionTexture.sample(s, in.uv).r;
    float occlusionFactor = 1.0 + occlusionStrength * (ao - 1.0);
    finalColor.rgb *= occlusionFactor;
}
```

## Crossref

- [VMK#290](https://github.com/arkavo-org/VRMMetalKit/issues/290) — sibling glTF-core textureInfo gap (normal.scale); closed in 0.16.0-rc.3 PR #291. Same wiring pattern.
- [VMK#287](https://github.com/arkavo-org/VRMMetalKit/issues/287) — sibling MToon emissive multiplier; closed in 0.16.0-rc.3 PR #291 for LDR range.
