# VMK — `normalTexture.scale` ignored on MToon materials

**Status**: filed 2026-05-23 as [VMK#290](https://github.com/arkavo-org/VRMMetalKit/issues/290).

---

**Title:** MToon: glTF-core `normalTexture` is read but the `scale` field on the textureInfo is silently ignored

**Labels:** bug, mtoon, materials, gltf-core, spec-compliance

**Body:**

When an MToon material's `pbrMetallicRoughness.baseColorTexture`'s sibling `normalTexture` carries a non-default `scale`, VRMMetalKit reads the normal map (different from a no-normal render) but doesn't honor the `scale` field. Two conformance variants with `scale=1.0` and `scale=2.0` render to a byte-identical PNG on VMK; UniVRM (the consortium reference) and three-vrm both produce distinct outputs.

Narrower scope than [VMK#289](https://github.com/arkavo-org/VRMMetalKit/issues/289): there the entire texture-binding's downstream parameters were ignored (factor, mode, modulation). Here only the per-textureInfo `scale` field is ignored — the texture itself is correctly applied at scale=1.0 equivalent perturbation, just not amplified by the field.

## Spec reference

glTF 2.0 `material.normalTextureInfo.schema.json`:

```json
{
    "$id": "material.normalTextureInfo.schema.json",
    "title": "Material Normal Texture Info",
    "allOf": [ { "$ref": "textureInfo.schema.json" } ],
    "properties": {
        "scale": {
            "type": "number",
            "description": "The scalar parameter applied to each normal vector of the normal texture.",
            "default": 1.0,
            "gltf_detailedDescription": "The scalar parameter applied to each normal vector of the texture. This value scales the normal vector in X and Y directions using the formula: `scaledNormal = normalize((<sampled normal texture value> * 2.0 - 1.0) * vec3(<normal scale>, <normal scale>, 1.0))`."
        }
    }
}
```

The spec is unambiguous: `scale` multiplies the X and Y components of the unpacked tangent-space normal before renormalisation. `scale=0` should disable perturbation entirely; `scale=1` is the default; `scale > 1` amplifies surface roughness.

## Reproducer

Conformance corpus at `crates/vrm-asset-generator/src/sweep.rs::mtoon_pbr_textures_sweep` (6 variants; two are normal-map specific). Synthetic VRM 1.0 avatar with a procedural 16×16 tangent-space normal map attached to `pbrMetallicRoughness.normalTexture`. Per-quadrant normal directions: TL (-0.5, +0.5, 0.707), TR (+0.5, +0.5, 0.707), BL (-0.5, -0.5, 0.707), BR (+0.5, -0.5, 0.707), encoded as RGB byte values per the glTF spec.

Rendered through `vrm-runner execute-test-plan --adapter-bin vrm-metal-kit-adapter` on VMK 0.16.0-rc.2 (commit `7f7d39b`) + Apple M4 Max + macOS 26.5, plus the same plans through three-vrm 3.5.0 and UniVRM v0.131.0:

| test_id | declared `scale` | vrm-metal-kit | three-vrm | UniVRM (consortium reference) |
|---|---|---|---|---|
| `mtoon_pbrtex_baseline` (no normalTexture) | — | `5d8cf17... 50K` | `6ff1f56... 58K` | `9ed71e6... 53K` |
| `mtoon_pbrtex_normal_default` | `1.0` | `a599ae8... 69K` | `cb5eec9... 71K` | `e985087... 57K` |
| `mtoon_pbrtex_normal_scale_2x` | `2.0` | **`a599ae8... 69K`** | `d81b15e... 83K` | `308879e... 81K` |

Both UniVRM and three-vrm produce distinct hashes between `scale=1.0` and `scale=2.0` (file size jump in both cases — UniVRM 57K → 81K, three-vrm 71K → 83K — consistent with amplified normal perturbation producing more pixel variation). VMK produces a byte-identical PNG for both scale values.

## What's working

VMK *does* read the normal map. The `normal_default` hash (`a599ae8...`) differs from the no-normal baseline (`5d8cf17...`), so the per-vertex perturbation reaches the shader. Only the `scale` field is silently dropped.

## What's broken

The per-textureInfo `scale` field on `normalTexture` is not consulted by VMK's MToon material parser. The shader receives the unpacked tangent-space normal but the `vec3(scale, scale, 1.0)` amplification factor from the spec's formula is missing — VMK effectively renders every `normalTexture` as `scale=1.0`.

## Suggested fix

Before binding the normalTexture to the shader, read the optional `scale` field and thread it into the shader uniform alongside the texture sampler:

```swift
let normalScale: Float = (material.normalTexture?["scale"] as? Double)
    .map(Float.init)
    ?? 1.0
shaderNormalScale = normalScale
```

Then in the fragment shader:

```metal
float3 sampledNormal = normalTexture.sample(s, in.uv).xyz * 2.0 - 1.0;
sampledNormal.xy *= normalScale;
float3 tangentNormal = normalize(sampledNormal);
// ... continue with TBN transform ...
```

## Filer context

Surfaced by [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) commit `554cb49` (gap #5 PBR-textures sweep). Details:

- Sweep code: `crates/vrm-asset-generator/src/sweep.rs::mtoon_pbr_textures_sweep`
- Texture generator: `crates/vrm-asset-generator/src/texture.rs::quadrant_normal_map_16`
- Findings entry: `docs/findings.md` "glTF-core PBR textures on MToon"
- UniVRM reference run captured 2026-05-23 against Unity 6 + UniVRM v0.131.0

After the fix lands, the conformance suite picks it up automatically — `mtoon_pbrtex_normal_default` and `mtoon_pbrtex_normal_scale_2x` will produce distinct hashes on VMK matching the UniVRM + three-vrm spec behaviour.

## Related VMK issues filed by this suite

- [VMK#287](https://github.com/arkavo-org/VRMMetalKit/issues/287) — `VRMC_materials_hdr_emissiveMultiplier` ignored (pure no-op, extension dropped)
- [VMK#288](https://github.com/arkavo-org/VRMMetalKit/issues/288) — `KHR_texture_transform` on textureInfo ignored (pure no-op, extension dropped)
- [VMK#289](https://github.com/arkavo-org/VRMMetalKit/issues/289) — `outlineWidthMultiplyTexture` triggers degraded outline pipeline (codepath runs, broken on three axes)
- **This issue** — `normalTexture` read correctly, only the `scale` field is ignored (narrowest scope so far)

## Note on occlusionTexture

The same PBR-textures sweep showed that **UniVRM, three-vrm, AND VMK all ignore `occlusionTexture` on MToon materials**. That's documented in the conformance suite's `docs/methodology.md` as a non-applicable conformance axis (MToon spec is explicitly non-PBR; ecosystem-wide omission is intentional). This issue is scoped to `normalTexture.scale` only — UniVRM's behaviour proves `scale` is on the conformance hook even though `strength` isn't.
