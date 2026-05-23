# VMK — `VRMC_materials_hdr_emissiveMultiplier` silently ignored

**Status**: filed 2026-05-23 as [VMK#287](https://github.com/arkavo-org/VRMMetalKit/issues/287).

---

**Title:** MToon: `VRMC_materials_hdr_emissiveMultiplier-1.0` is ignored — `extensions.VRMC_materials_hdr_emissiveMultiplier.emissiveMultiplier` never applied to rendered emission

**Labels:** bug, mtoon, materials, spec-compliance

**Body:**

VRM 1.0 assets carrying the `VRMC_materials_hdr_emissiveMultiplier-1.0` extension render to byte-identical PNGs across every multiplier value when loaded through VRMMetalKit. The extension's documented behaviour — "Overwrite material.emissiveFactor of the target material with the value multiplied by emissiveMultiplier" — is not applied. `emissiveFactor` itself is respected (different RGB channels produce different output), but the multiplier never reaches the shader.

## Spec reference

VRMC_materials_hdr_emissiveMultiplier-1.0 README (`Defined Properties` section, `vrm-c/vrm-specification` master):

> Overwrite material.emissiveFactor of the target material with the value multiplied by emissiveMultiplier.
> This value is linear.

Schema (`VRMC_materials_hdr_emissiveMultiplier.json`):

```json
{
    "title": "VRMC_materials_hdr_emissiveMultiplier",
    "type": "object",
    "properties": {
        "emissiveMultiplier": {
            "type": "number",
            "description": "A multiplier for emissiveFactor",
            "default": 1.0,
            "minimum": 0.0
        }
    },
    "required": ["emissiveMultiplier"]
}
```

Status field is "Archived — Superseded by [KHR_materials_emissive_strength](https://github.com/KhronosGroup/glTF/blob/main/extensions/2.0/Khronos/KHR_materials_emissive_strength/README.md)", but the extension remains in the VRM 1.0 spec tree and is present in real-world VRM 1.0 assets, so VMK should support it for spec-conformance on legacy content. Implementations may treat it as a direct alias of `KHR_materials_emissive_strength.emissiveStrength` if that's simpler — the math is identical.

## Reproducer

Conformance corpus assets at `crates/vrm-asset-generator/src/sweep.rs::mtoon_emissive_sweep` (14 variants total). Each is a synthetic VRM 1.0 avatar with `base_color = [0.3, 0.3, 0.3, 1.0]`, `emissive_factor = [1.0, 1.0, 1.0]` (or per-channel for the r/g/b variants), and varying `emissiveMultiplier`. Rendered through `vrm-runner execute-test-plan --adapter-bin vrm-metal-kit-adapter` on VMK 0.16.0-rc.2 (commit `7f7d39b`) + Apple M4 Max + macOS 26.5:

| test_id | declared effective emission | rendered PNG sha256[:12] |
|---|---|---|
| `mtoon_emissive_multiplier_0` | `[1,1,1] × 0 = [0,0,0]` | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_0p25` | `[1,1,1] × 0.25 = [0.25, …]` | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_0p5` | `[1,1,1] × 0.5` | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_0p75` | `[1,1,1] × 0.75` | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_1` | `[1,1,1] × 1 = [1,1,1]` | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_2` | `[1,1,1] × 2 = [2,2,2]` (HDR) | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_4` | `[1,1,1] × 4 = [4,4,4]` (HDR) | `9d5a8a62ccb8` |

**Seven distinct multipliers, one rendered output.** The multiplier value never reaches the shader.

Per-channel variants confirm `emissiveFactor` itself IS being read:

| test_id | rendered sha256[:12] |
|---|---|
| `mtoon_emissive_r_x1` (factor `[1,0,0]`) | `c8e62ed8cb7a` |
| `mtoon_emissive_g_x1` (factor `[0,1,0]`) | `770f3e900379` |
| `mtoon_emissive_b_x1` (factor `[0,0,1]`) | `2f554fa91511` |

Three distinct outputs for three distinct colors. So VMK reads `material.emissiveFactor` but doesn't consult `extensions.VRMC_materials_hdr_emissiveMultiplier.emissiveMultiplier`.

## Cross-renderer comparison

Same assets through `three-vrm` 3.5.0 (Playwright + headless Chromium) on the same machine:

| test_id | vrm-metal-kit | three-vrm |
|---|---|---|
| `mtoon_emissive_multiplier_0` | `9d5a8a62ccb8` | `adc93c4ebafb` |
| `mtoon_emissive_multiplier_0p25` | `9d5a8a62ccb8` | `56d40fc9d08d` |
| `mtoon_emissive_multiplier_0p5` | `9d5a8a62ccb8` | `720eabd652fc` |
| `mtoon_emissive_multiplier_0p75…4` | `9d5a8a62ccb8` | `86eb695a20fb` |

`three-vrm` produces three distinct outputs across the [0, 0.5] multiplier range — proving the multiplier reaches the shader and is applied linearly. Values ≥ 0.75 all converge because `base_color + emissive × multiplier ≥ 1.0` saturates the 8-bit framebuffer (expected UNORM clamp at the output stage — sweep can be re-tuned with a darker base color to expose the HDR axis further). VMK's failure is not a framebuffer issue: it's at the shader-input stage.

## Where the gap likely is

Educated guess from reading `Sources/VRMMetalKit/Renderer/VRMRenderer.swift` + `Sources/VRMMetalKit/Shaders/MToonShader.metal` patterns:
- the material parser reads `material.emissiveFactor` from the glTF JSON correctly,
- but doesn't read `material.extensions["VRMC_materials_hdr_emissiveMultiplier"]["emissiveMultiplier"]` during material construction,
- so the shader's `emissive` uniform receives the raw factor with implicit multiplier 1.0.

`KHR_materials_emissive_strength` (the spec's named replacement) takes the same shape — `extensions.KHR_materials_emissive_strength.emissiveStrength: number` — so a single material-loader hook can handle both: pick whichever extension is present (or take the product if both are, per the glTF convention of "later wins") and apply it to `emissiveFactor` at construction time.

## Suggested fix

In the MToon material parser, before handing `emissiveFactor` to the shader:

```swift
// VRMC_materials_hdr_emissiveMultiplier-1.0 (and KHR_materials_emissive_strength)
let multiplier: Float = {
    if let ext = material.extensions?["VRMC_materials_hdr_emissiveMultiplier"] as? [String: Any],
       let m = ext["emissiveMultiplier"] as? Double {
        return Float(m)
    }
    if let ext = material.extensions?["KHR_materials_emissive_strength"] as? [String: Any],
       let s = ext["emissiveStrength"] as? Double {
        return Float(s)
    }
    return 1.0
}()
shaderEmissive = emissiveFactor * multiplier
```

(The exact integration point depends on where the existing `emissiveFactor` parse happens; the JSON shape and the math are the same.)

## Filer context

Surfaced by [vrm-conformance](https://github.com/arkavo-org/vrm-conformance) commit `6953dab` (the new emissive sweep) on its first cross-renderer run. Details:

- Sweep code: `crates/vrm-asset-generator/src/sweep.rs::mtoon_emissive_sweep`
- Findings entry: `docs/findings.md` "VMK ignores `VRMC_materials_hdr_emissiveMultiplier`"
- Adapter wiring (no change needed — the existing `load_vrm` path doesn't filter the JSON; the gap is on the VMK material parser side)

After the fix lands, the conformance suite will pick it up automatically — `mtoon_emissive_multiplier_*` test_ids will produce non-degenerate hashes that cross-validate against three-vrm.
