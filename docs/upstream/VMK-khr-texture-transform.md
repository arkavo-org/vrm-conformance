# VMK — `KHR_texture_transform` ignored on MToon `baseColorTexture`

**Status**: filed 2026-05-23 as [VMK#288](https://github.com/arkavo-org/VRMMetalKit/issues/288).

---

**Title:** MToon: `KHR_texture_transform` on `baseColorTexture` is ignored — `extensions.KHR_texture_transform.{offset,rotation,scale}` never applied to the sampled UVs

**Labels:** bug, mtoon, materials, gltf-extension, spec-compliance

**Body:**

When an MToon material's `baseColorTexture` carries `extensions.KHR_texture_transform`, VRMMetalKit reads the texture but does not apply the transform to the sampled UV coordinates. Eight conformance test variants exercising the spec's three transform axes (offset / rotation / scale) plus a combined case all render to a byte-identical PNG. By contrast, three-vrm produces 8 distinct outputs.

## Spec reference

[KHR_texture_transform-1.0 README](https://github.com/KhronosGroup/glTF/blob/main/extensions/2.0/Khronos/KHR_texture_transform/README.md), "Overview" section:

> This extension adds `offset`, `rotation`, and `scale` properties to textureInfo structures. These properties would typically be implemented as an affine transform on the UV coordinates. In GLSL:
>
> ```glsl
> mat3 translation = mat3(1,0,0, 0,1,0, Offset.x, Offset.y, 1);
> mat3 rotation = mat3(
>     cos(Rotation), sin(Rotation), 0,
>    -sin(Rotation), cos(Rotation), 0,
>                 0,             0, 1
> );
> mat3 scale = mat3(Scale.x,0,0, 0,Scale.y,0, 0,0,1);
> mat3 matrix = translation * rotation * scale;
> vec2 uvTransformed = ( matrix * vec3(Uv.xy, 1) ).xy;
> ```
>
> This is equivalent to Unity's `Material#SetTextureOffset` and `Material#SetTextureScale`, or Three.js's `Texture#offset` and `Texture#repeat`.

Schema (`textureInfo.KHR_texture_transform.schema.json`): defaults are `offset=[0,0]`, `rotation=0`, `scale=[1,1]`. Status is "Complete, Ratified by the Khronos Group" — this is a stable Khronos extension, not a vendor proposal.

## Reproducer

Conformance corpus assets at `crates/vrm-asset-generator/src/sweep.rs::mtoon_texture_transform_sweep` (8 variants). Each is a synthetic VRM 1.0 avatar with a 16×16 quadrant checkerboard texture (red top-left, green top-right, blue bottom-left, yellow bottom-right) attached to the head sphere's `pbrMetallicRoughness.baseColorTexture`, with one of eight different `KHR_texture_transform` extension payloads. Rendered through `vrm-runner execute-test-plan --adapter-bin vrm-metal-kit-adapter` on VMK 0.16.0-rc.2 (commit `7f7d39b`) + Apple M4 Max + macOS 26.5:

| test_id | declared transform | rendered sha256[:12] (VMK) | rendered sha256[:12] (three-vrm) |
|---|---|---|---|
| `mtoon_uvxform_identity` | (extension absent) | `5b8077fbe8a4` | `fcd41570e763` |
| `mtoon_uvxform_offset_x_0p5` | offset `[0.5, 0]` | `5b8077fbe8a4` | `d8aed98253e2` |
| `mtoon_uvxform_offset_y_0p5` | offset `[0, 0.5]` | `5b8077fbe8a4` | `147fd12b206b` |
| `mtoon_uvxform_rotation_eighth` | rotation `π/4` rad | `5b8077fbe8a4` | `c416ef51b768` |
| `mtoon_uvxform_rotation_quarter` | rotation `π/2` rad | `5b8077fbe8a4` | `6a6992a5755a` |
| `mtoon_uvxform_scale_2x` | scale `[2, 2]` | `5b8077fbe8a4` | `33ac4596423c` |
| `mtoon_uvxform_scale_half` | scale `[0.5, 0.5]` | `5b8077fbe8a4` | `2ec50ff90eb9` |
| `mtoon_uvxform_combined` | offset `[0.25, 0.25]` + rotation `π/4` + scale `[2, 2]` | `5b8077fbe8a4` | `0d7e9f3ccbf2` |

**VMK: one rendered output across all 8 distinct transforms.** three-vrm: eight unique outputs, one per variant — proving the extension is well-formed in the asset and just needs a renderer to honor it.

VMK **does** read the texture itself (the `5b8077fbe8a4` hash differs from a no-texture `mtoon_default` render at `5d8cf1789282`) — it just renders every variant with the raw mesh UVs, unaltered by the extension.

## Where the gap likely is

Educated guess from reading existing VMK MToon material handling: the `pbrMetallicRoughness.baseColorTexture` parse picks up the `index` (and any `texCoord` override) but doesn't read `material.pbrMetallicRoughness.baseColorTexture.extensions["KHR_texture_transform"]` during material construction. The shader's UV input is then `mesh_uv` (post-glTF-Y-flip but otherwise raw), not `transform(mesh_uv)`.

`extensions.KHR_texture_transform` is a glTF-core extension and applies to **every** `textureInfo` location (baseColorTexture, normalTexture, occlusionTexture, emissiveTexture, and the MToon-specific texture bindings like `shadeMultiplyTexture`, `matcapTexture`, `outlineWidthMultiplyTexture`, etc.). A single material-loader helper that builds a `mat3` from the optional extension payload and threads it through the shader's UV computation would cover every textureInfo at once — same payload shape regardless of which texture binding it's attached to.

## Suggested fix

Before binding the textureInfo to the shader, parse the extension if present and compose the `mat3` from the spec's example shader:

```swift
// glTF core extension — applies to every textureInfo, not just baseColorTexture.
func uvTransform(for textureInfo: [String: Any]) -> simd_float3x3 {
    guard let ext = textureInfo["extensions"] as? [String: Any],
          let kt = ext["KHR_texture_transform"] as? [String: Any] else {
        return matrix_identity_float3x3
    }
    let offset = (kt["offset"] as? [Double]).flatMap { o in
        o.count >= 2 ? SIMD2<Float>(Float(o[0]), Float(o[1])) : nil
    } ?? .zero
    let rotation = Float(kt["rotation"] as? Double ?? 0.0)
    let scale = (kt["scale"] as? [Double]).flatMap { s in
        s.count >= 2 ? SIMD2<Float>(Float(s[0]), Float(s[1])) : nil
    } ?? SIMD2<Float>(1, 1)
    let cosR = cos(rotation), sinR = sin(rotation)
    let translation = simd_float3x3(rows: [
        SIMD3<Float>(1, 0, offset.x),
        SIMD3<Float>(0, 1, offset.y),
        SIMD3<Float>(0, 0, 1),
    ])
    let rotMat = simd_float3x3(rows: [
        SIMD3<Float>(cosR, sinR, 0),
        SIMD3<Float>(-sinR, cosR, 0),
        SIMD3<Float>(0, 0, 1),
    ])
    let scaleMat = simd_float3x3(rows: [
        SIMD3<Float>(scale.x, 0, 0),
        SIMD3<Float>(0, scale.y, 0),
        SIMD3<Float>(0, 0, 1),
    ])
    return translation * rotMat * scaleMat
}
```

Then pass that `mat3` into the MToon fragment shader and apply `transformed_uv = (uv_xform * vec3(uv, 1)).xy` before sampling. Same fix shape as the suggested fix for the emissive-multiplier extension (VMK#287) — a per-textureInfo extension that needs threading into the shader from material construction.

## Filer context

Surfaced by [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) commit `866162d` (the new texture-transform sweep) on its first cross-renderer run. Details:

- Sweep code: `crates/vrm-asset-generator/src/sweep.rs::mtoon_texture_transform_sweep`
- Texture infra: `crates/vrm-asset-generator/src/texture.rs` (procedural 16×16 quadrant checkerboard, base64 data-URI embedded in glTF JSON)
- Findings entry: `docs/findings.md` "KHR_texture_transform — three distinct conformance patterns"

After the fix lands, the conformance suite will pick it up automatically — `mtoon_uvxform_*` test_ids will produce non-degenerate hashes that cross-validate against three-vrm.
