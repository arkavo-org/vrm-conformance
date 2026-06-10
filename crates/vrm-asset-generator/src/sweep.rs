//! MToon basic parameter sweep: ~50 assets, one per axis-value pair, all
//! other parameters held at `MToonParams::defaults()`.

use crate::params::{AlphaMode, FirstPersonType, MToonParams, OutlineWidthMode, TextureTransform};
use crate::spring_bone::{
    ColliderAttach, ColliderGroupParams, ColliderParams, ColliderShape, SpringBoneParams,
    SpringBoneSceneParams,
};

pub fn mtoon_basic_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Baseline.
    out.push(MToonParams::defaults("mtoon_default"));

    // shadingShiftFactor: -1.0 .. 1.0
    for v in [-1.0, -0.8, -0.5, -0.2, 0.0, 0.2, 0.5, 0.8, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_shadingShift_{}", fmt_num(v)));
        p.shading_shift_factor = v;
        out.push(p);
    }

    // shadingToonyFactor: 0.0 .. 1.0
    for v in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_shadingToony_{}", fmt_num(v)));
        p.shading_toony_factor = v;
        out.push(p);
    }

    // giEqualizationFactor
    for v in [0.0, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_giEqualization_{}", fmt_num(v)));
        p.gi_equalization_factor = v;
        out.push(p);
    }

    // rimLightingMixFactor (the three-vrm v3.5.0 regression source)
    for v in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
        let mut p = MToonParams::defaults(format!("mtoon_rimLightingMix_{}", fmt_num(v)));
        p.rim_lighting_mix_factor = v;
        // Pair with a non-zero rim color so the parameter actually matters
        // visually.
        p.parametric_rim_color_factor = [1.0, 0.5, 0.0];
        p.parametric_rim_fresnel_power_factor = 5.0;
        out.push(p);
    }

    // Outline mode × width
    for &mode in &[
        OutlineWidthMode::None,
        OutlineWidthMode::WorldCoordinates,
        OutlineWidthMode::ScreenCoordinates,
    ] {
        let mode_str = match mode {
            OutlineWidthMode::None => "none",
            OutlineWidthMode::WorldCoordinates => "world",
            OutlineWidthMode::ScreenCoordinates => "screen",
        };
        if matches!(mode, OutlineWidthMode::None) {
            // Single None baseline; width is meaningless when outlines are off.
            let mut p = MToonParams::defaults("mtoon_outline_none");
            p.outline_width_mode = mode;
            out.push(p);
            continue;
        }
        for &w in &[0.01_f32, 0.03, 0.05, 0.10] {
            let mut p =
                MToonParams::defaults(format!("mtoon_outline_{mode_str}_{w}", w = fmt_num(w)));
            p.outline_width_mode = mode;
            p.outline_width_factor = w;
            p.outline_color_factor = [0.0, 0.0, 0.0];
            out.push(p);
        }
    }

    // renderQueueOffsetNumber
    for v in [-9_i32, 0, 9] {
        let mut p = MToonParams::defaults(format!("mtoon_renderQueueOffset_{v}"));
        p.render_queue_offset_number = v;
        out.push(p);
    }

    // doubleSided
    for v in [false, true] {
        let mut p = MToonParams::defaults(format!("mtoon_doubleSided_{v}"));
        p.double_sided = v;
        out.push(p);
    }

    // VRM 0.x slice-1 v1 counterparts — named to pair with mtoon_basic_v0_sweep()
    // Applicable entries. The symmetry assertion in registry_symmetry_tests
    // requires these IDs to exist.
    out.push(MToonParams::defaults("mtoon_basic_lit_001"));
    {
        let mut p = MToonParams::defaults("mtoon_basic_shadeShift_neg05");
        p.shading_shift_factor = -0.5;
        out.push(p);
    }

    // alphaMode × alphaCutoff (MASK) and transparentWithZWrite (BLEND).
    // Exercises VMK#264 (MToon shader discard_fragment defeats A2C on MASK)
    // by routing the renderer through its MASK pipeline; cross-renderer SSIM
    // against UniVRM/three-vrm/godot-vrm surfaces any divergence. The default
    // `mtoon_default` already covers the OPAQUE baseline.
    for cutoff in [0.25_f32, 0.5, 0.75] {
        let mut p = MToonParams::defaults(format!("mtoon_alpha_mask_cutoff_{}", fmt_num(cutoff)));
        p.alpha_mode = AlphaMode::Mask;
        p.alpha_cutoff = cutoff;
        // A semi-transparent base color so MASK has something to clip.
        p.base_color_factor = [1.0, 1.0, 1.0, cutoff];
        out.push(p);
    }
    for z in [false, true] {
        let mut p = MToonParams::defaults(format!("mtoon_alpha_blend_zwrite_{z}"));
        p.alpha_mode = AlphaMode::Blend;
        p.transparent_with_z_write = z;
        // Half-transparent base color for BLEND so the path actually composites.
        p.base_color_factor = [1.0, 1.0, 1.0, 0.5];
        out.push(p);
    }

    out
}

/// glTF-core PBR textures sweep: 6 variants covering `normalTexture`
/// (tangent-space normal map) and `occlusionTexture` (R-channel AO
/// modulation, reuses checkerboard). These bindings are NOT in MToon —
/// they're glTF-core PBR. The question this surfaces: do MToon renderers
/// integrate with the glTF-core PBR pipeline?
///
/// Variants:
/// - `mtoon_pbrtex_baseline` — no PBR textures.
/// - `mtoon_pbrtex_normal_default` — normal map scale=1.0.
/// - `mtoon_pbrtex_normal_scale_2x` — scale=2.0.
/// - `mtoon_pbrtex_occlusion_default` — occlusion strength=1.0.
/// - `mtoon_pbrtex_occlusion_strength_half` — strength=0.5.
/// - `mtoon_pbrtex_combined` — both.
pub fn mtoon_pbr_textures_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    let mut baseline = MToonParams::defaults("mtoon_pbrtex_baseline");
    baseline.normal_texture_scale = None;
    baseline.occlusion_texture_strength = None;
    out.push(baseline);

    let mut normal_default = MToonParams::defaults("mtoon_pbrtex_normal_default");
    normal_default.normal_texture_scale = Some(1.0);
    out.push(normal_default);

    let mut normal_scale_2x = MToonParams::defaults("mtoon_pbrtex_normal_scale_2x");
    normal_scale_2x.normal_texture_scale = Some(2.0);
    out.push(normal_scale_2x);

    let mut occlusion_default = MToonParams::defaults("mtoon_pbrtex_occlusion_default");
    occlusion_default.occlusion_texture_strength = Some(1.0);
    out.push(occlusion_default);

    let mut occlusion_strength_half = MToonParams::defaults("mtoon_pbrtex_occlusion_strength_half");
    occlusion_strength_half.occlusion_texture_strength = Some(0.5);
    out.push(occlusion_strength_half);

    let mut combined = MToonParams::defaults("mtoon_pbrtex_combined");
    combined.normal_texture_scale = Some(1.0);
    combined.occlusion_texture_strength = Some(1.0);
    out.push(combined);

    out
}

/// MToon `outlineWidthMultiplyTexture` sweep: 5 variants exercising
/// per-vertex outline-width modulation. Per the MToon spec (README.md
/// :710-715): the texture's **G-channel** (not R) is read and
/// multiplied into the outline width.
///
/// Our procedural checkerboard's G values give a clear per-quadrant
/// G signal: red=30, green=200, blue=30, yellow=220. Conformant
/// renderers display thicker outlines on the green+yellow portions
/// of the sphere and thinner outlines on the red+blue portions.
///
/// Variants:
/// - `mtoon_outlinewidthtex_baseline` — outline_width_mode=world
///   + width=0.05, no texture. Diff target.
/// - `mtoon_outlinewidthtex_world` — texture + world-coord outline.
/// - `mtoon_outlinewidthtex_screen` — texture + screen-coord outline.
/// - `mtoon_outlinewidthtex_width_2x` — wider base outline (0.1).
/// - `mtoon_outlinewidthtex_mode_none` — texture present but
///   `outline_width_mode=none`. Regression guard: must NOT produce
///   visible outlines (gating must check mode, not just texture).
pub fn mtoon_outline_width_multiply_texture_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    let mut baseline = MToonParams::defaults("mtoon_outlinewidthtex_baseline");
    baseline.outline_width_mode = OutlineWidthMode::WorldCoordinates;
    baseline.outline_width_factor = 0.05;
    baseline.outline_color_factor = [0.0, 0.0, 0.0];
    baseline.outline_width_multiply_texture = false;
    out.push(baseline);

    let mut world_textured = MToonParams::defaults("mtoon_outlinewidthtex_world");
    world_textured.outline_width_mode = OutlineWidthMode::WorldCoordinates;
    world_textured.outline_width_factor = 0.05;
    world_textured.outline_color_factor = [0.0, 0.0, 0.0];
    world_textured.outline_width_multiply_texture = true;
    out.push(world_textured);

    let mut screen_textured = MToonParams::defaults("mtoon_outlinewidthtex_screen");
    screen_textured.outline_width_mode = OutlineWidthMode::ScreenCoordinates;
    screen_textured.outline_width_factor = 0.05;
    screen_textured.outline_color_factor = [0.0, 0.0, 0.0];
    screen_textured.outline_width_multiply_texture = true;
    out.push(screen_textured);

    let mut wider = MToonParams::defaults("mtoon_outlinewidthtex_width_2x");
    wider.outline_width_mode = OutlineWidthMode::WorldCoordinates;
    wider.outline_width_factor = 0.10;
    wider.outline_color_factor = [0.0, 0.0, 0.0];
    wider.outline_width_multiply_texture = true;
    out.push(wider);

    let mut mode_none = MToonParams::defaults("mtoon_outlinewidthtex_mode_none");
    mode_none.outline_width_mode = OutlineWidthMode::None;
    mode_none.outline_width_factor = 0.05;
    mode_none.outline_width_multiply_texture = true;
    out.push(mode_none);

    out
}

/// MToon emissive sweep covering `material.emissiveFactor` +
/// `VRMC_materials_hdr_emissiveMultiplier-1.0`. Held-constant axes:
/// base color is mid-gray ([0.3,0.3,0.3,1]) so the emissive contribution
/// is unambiguously the dominant pixel-write at the rendered head. All
/// other MToon parameters are at defaults.
///
/// 14 variants:
/// - 7 multiplier values × white emissive [1,1,1] (the canonical signal:
///   verifies the spec's "overwrite material.emissiveFactor with
///   factor * multiplier" rule across the [0, 4] HDR range).
/// - 3 color variants at multiplier=1 (red / green / blue) to verify
///   per-channel independence.
/// - 3 color variants at multiplier=2 to verify the multiplier scales
///   the whole 3-vector rather than re-mapping one channel.
/// - 1 baseline: factor=[0,0,0], multiplier=anything — emits no
///   extension and should render identically to MToon default
///   (regression guard: extension-emit must be conditional).
///
/// Methodology: `tone_mapping: none` (per `docs/methodology.md`) is
/// required so HDR multipliers > 1 saturate the UNORM PNG channel
/// rather than getting reshaped by ACES/Filmic. The asset's test plan
/// inherits the suite default of `tone_mapping: None`.
pub fn mtoon_emissive_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Sub-1.0 sweep where renderer differences should be linearly
    // distinguishable in the output PNG. 1.5 / 2.0 / 4.0 verify HDR
    // saturation behaviour (everything above 1.0 clamps in UNORM output
    // but should still pass through the spec's multiply step before
    // clamping — so a renderer that applied the multiplier as `min(1,…)`
    // before the multiply would diverge from spec).
    for m in [0.0_f32, 0.25, 0.5, 0.75, 1.0, 2.0, 4.0] {
        let mut p = MToonParams::defaults(format!("mtoon_emissive_multiplier_{}", fmt_num(m)));
        p.base_color_factor = [0.3, 0.3, 0.3, 1.0];
        p.emissive_factor = [1.0, 1.0, 1.0];
        p.emissive_multiplier = m;
        out.push(p);
    }

    // Per-channel independence at multiplier=1 (no extension emitted —
    // exercises plain glTF emissiveFactor).
    for (axis, factor) in [
        ("r", [1.0_f32, 0.0, 0.0]),
        ("g", [0.0, 1.0, 0.0]),
        ("b", [0.0, 0.0, 1.0]),
    ] {
        let mut p = MToonParams::defaults(format!("mtoon_emissive_{axis}_x1"));
        p.base_color_factor = [0.3, 0.3, 0.3, 1.0];
        p.emissive_factor = factor;
        p.emissive_multiplier = 1.0;
        out.push(p);
    }

    // Per-channel × multiplier=2: emissive_multiplier scales the
    // 3-vector uniformly per spec ("overwrite material.emissiveFactor
    // of the target material with the value multiplied by
    // emissiveMultiplier"), so red×2 should produce the same colour
    // hue as red×1 just brighter, not e.g. a chroma shift.
    for (axis, factor) in [
        ("r", [1.0_f32, 0.0, 0.0]),
        ("g", [0.0, 1.0, 0.0]),
        ("b", [0.0, 0.0, 1.0]),
    ] {
        let mut p = MToonParams::defaults(format!("mtoon_emissive_{axis}_x2"));
        p.base_color_factor = [0.3, 0.3, 0.3, 1.0];
        p.emissive_factor = factor;
        p.emissive_multiplier = 2.0;
        out.push(p);
    }

    // Zero-emissive baseline: even though `emissive_multiplier=4.0` is
    // set, the extension must NOT be emitted because emissive_factor is
    // [0,0,0] (zero × anything = zero; emitting the extension would be
    // misleading and validators flag unused `extensionsUsed` entries).
    let mut zero = MToonParams::defaults("mtoon_emissive_zero_factor");
    zero.emissive_factor = [0.0, 0.0, 0.0];
    zero.emissive_multiplier = 4.0;
    out.push(zero);

    out
}

/// MToon `shadingShiftTexture` sweep: 5 variants exercising the spec's
/// per-pixel shading-boundary modulation. Per the MToon spec
/// (`shadingShiftTexture` section): the texture's R-channel value,
/// multiplied by `scale`, is ADDED to `shadingShiftFactor` to shift
/// the lit/shaded boundary per-pixel. The checkerboard's RGB values
/// give a clear per-quadrant R signal (red=220, green=30, blue=30,
/// yellow=220) — boundary should land at four distinct positions on
/// the sphere depending on which quadrant the surface point UV-samples.
///
/// Variants:
/// - `mtoon_shadingshifttex_baseline` — no texture; clean
///   `shadingShiftFactor` baseline. Diff target.
/// - `mtoon_shadingshifttex_default` — texture, `scale=1.0` (spec
///   default). R-channel directly added to factor.
/// - `mtoon_shadingshifttex_scale_half` — `scale=0.5`. Half-intensity
///   per-pixel shift; verifies multiplicative scale axis.
/// - `mtoon_shadingshifttex_scale_2x` — `scale=2.0`. Double-intensity.
/// - `mtoon_shadingshifttex_with_factor` — `scale=1.0` plus a
///   non-zero `shadingShiftFactor=-0.3`. Tests the spec's additive
///   composition (`final_shift = factor + texture.r * scale`).
pub fn mtoon_shading_shift_texture_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    let mut baseline = MToonParams::defaults("mtoon_shadingshifttex_baseline");
    baseline.shading_shift_texture_scale = None;
    out.push(baseline);

    let mut default_texture = MToonParams::defaults("mtoon_shadingshifttex_default");
    default_texture.shading_shift_texture_scale = Some(1.0);
    out.push(default_texture);

    let mut scale_half = MToonParams::defaults("mtoon_shadingshifttex_scale_half");
    scale_half.shading_shift_texture_scale = Some(0.5);
    out.push(scale_half);

    let mut scale_2x = MToonParams::defaults("mtoon_shadingshifttex_scale_2x");
    scale_2x.shading_shift_texture_scale = Some(2.0);
    out.push(scale_2x);

    let mut with_factor = MToonParams::defaults("mtoon_shadingshifttex_with_factor");
    with_factor.shading_shift_texture_scale = Some(1.0);
    with_factor.shading_shift_factor = -0.3;
    out.push(with_factor);

    out
}

/// MToon `rimMultiplyTexture` sweep: 4 variants exercising the spec's
/// per-pixel parametric-rim modulation. Per the MToon spec
/// (`rimMultiplyTexture` section): the texture's RGB multiplies into
/// the parametric rim contribution. For the multiplication to produce
/// visible signal the asset must set non-zero `parametricRimColorFactor`
/// and a non-zero `rimLightingMixFactor`; every variant here does so.
///
/// Variants:
/// - `mtoon_rimtex_baseline` — texture absent, parametric rim active
///   (white rim color, lift=0). Diff target proves "renderer reads
///   rimMultiplyTexture" by showing colour difference when present.
/// - `mtoon_rimtex_default` — texture present, same rim params as
///   baseline. Conformant render: rim colour modulated per-pixel by
///   the checkerboard RGB.
/// - `mtoon_rimtex_red_rim` — `parametricRimColorFactor=[1,0,0]`
///   (red rim), textured. Tests the multiplicative blending of the
///   texture RGB against a non-white rim colour.
/// - `mtoon_rimtex_half_mix` — `rimLightingMixFactor=0.5`. Halves
///   the rim blend strength to verify the mix factor is applied
///   after the texture multiplication.
pub fn mtoon_rim_multiply_texture_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Rim contribution depends on parametricRimColor + lift +
    // fresnelPower; defaults to all-zero, which would make rim
    // invisible. Helper closure to set the rim baseline for every
    // variant.
    let with_rim = |id: &str| -> MToonParams {
        let mut p = MToonParams::defaults(id);
        p.parametric_rim_color_factor = [1.0, 1.0, 1.0];
        p.parametric_rim_fresnel_power_factor = 3.0;
        p.parametric_rim_lift_factor = 0.0;
        p.rim_lighting_mix_factor = 1.0;
        p
    };

    let mut baseline = with_rim("mtoon_rimtex_baseline");
    baseline.rim_multiply_texture = false;
    out.push(baseline);

    let mut default_texture = with_rim("mtoon_rimtex_default");
    default_texture.rim_multiply_texture = true;
    out.push(default_texture);

    let mut red_rim = with_rim("mtoon_rimtex_red_rim");
    red_rim.rim_multiply_texture = true;
    red_rim.parametric_rim_color_factor = [1.0, 0.0, 0.0];
    out.push(red_rim);

    let mut half_mix = with_rim("mtoon_rimtex_half_mix");
    half_mix.rim_multiply_texture = true;
    half_mix.rim_lighting_mix_factor = 0.5;
    out.push(half_mix);

    out
}

/// MToon `matcapTexture` sweep: 5 variants exercising the spec's rim-
/// lighting matcap contribution
/// (`rim += matcapFactor.rgb * texture(matcapTexture, matcapUv).rgb`,
/// where matcapUv is derived from the view-space surface normal — not
/// mesh UVs). To isolate the matcap signal, every variant sets
/// `baseColorFactor` and `shadeColorFactor` to near-black so the only
/// visible contribution to the rendered sphere is the matcap RGB
/// (plus any parametric rim, which we leave at zero defaults).
/// Renders should show the procedural checkerboard mapped onto the
/// sphere by view orientation — distinctive sphere-mapping signature
/// that's visually unambiguous when present.
///
/// Variants:
/// - `mtoon_matcap_baseline` — matcapTexture absent, near-black base
///   and shade colors. Sphere renders near-black with no matcap
///   contribution. The diff target for "renderer reads matcapTexture".
/// - `mtoon_matcap_default` — matcapTexture set, matcapFactor=[1,1,1]
///   (the spec default). Sphere displays the raw checkerboard via
///   matcap sphere-mapping.
/// - `mtoon_matcap_red_tint` — matcapFactor=[1,0,0]. Only red channel
///   of the matcap reaches the rendered sphere; green and blue
///   quadrants of the texture darken to black, red and yellow
///   (red+green) keep their red component.
/// - `mtoon_matcap_blue_tint` — matcapFactor=[0,0,1]. Symmetric blue
///   test; only blue and yellow (red+green has no blue) quadrants
///   contribute visibly.
/// - `mtoon_matcap_dim` — matcapFactor=[0.5,0.5,0.5]. Half-intensity
///   matcap. Exercises whether renderers apply the factor as a
///   linear multiplier (50% brightness reduction) vs ignoring or
///   clamping.
pub fn mtoon_matcap_texture_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Baseline: no matcap, near-black sphere. Render should show
    // a barely-visible black-on-magenta sphere with no matcap RGB.
    let mut baseline = MToonParams::defaults("mtoon_matcap_baseline");
    baseline.matcap_texture = false;
    baseline.base_color_factor = [0.05, 0.05, 0.05, 1.0];
    baseline.shade_color_factor = [0.0, 0.0, 0.0];
    out.push(baseline);

    // Textured, matcapFactor=[1,1,1] default — full matcap signal.
    let mut default_matcap = MToonParams::defaults("mtoon_matcap_default");
    default_matcap.matcap_texture = true;
    default_matcap.base_color_factor = [0.05, 0.05, 0.05, 1.0];
    default_matcap.shade_color_factor = [0.0, 0.0, 0.0];
    // matcap_factor stays at the spec default [1.0, 1.0, 1.0] from MToonParams::defaults
    out.push(default_matcap);

    // Red tint: matcapFactor=[1,0,0]. Per spec multiplicative blend,
    // only red components of the checkerboard survive.
    let mut red_tint = MToonParams::defaults("mtoon_matcap_red_tint");
    red_tint.matcap_texture = true;
    red_tint.base_color_factor = [0.05, 0.05, 0.05, 1.0];
    red_tint.shade_color_factor = [0.0, 0.0, 0.0];
    red_tint.matcap_factor = [1.0, 0.0, 0.0];
    out.push(red_tint);

    // Blue tint: symmetric test on the other primary axis.
    let mut blue_tint = MToonParams::defaults("mtoon_matcap_blue_tint");
    blue_tint.matcap_texture = true;
    blue_tint.base_color_factor = [0.05, 0.05, 0.05, 1.0];
    blue_tint.shade_color_factor = [0.0, 0.0, 0.0];
    blue_tint.matcap_factor = [0.0, 0.0, 1.0];
    out.push(blue_tint);

    // Dim: linear half-intensity. Verifies that matcapFactor is
    // applied as a multiplier rather than ignored or clamped.
    let mut dim = MToonParams::defaults("mtoon_matcap_dim");
    dim.matcap_texture = true;
    dim.base_color_factor = [0.05, 0.05, 0.05, 1.0];
    dim.shade_color_factor = [0.0, 0.0, 0.0];
    dim.matcap_factor = [0.5, 0.5, 0.5];
    out.push(dim);

    out
}

/// MToon `shadeMultiplyTexture` sweep: 6 variants exercising the spec's
/// shaded-color path (`shadeColorTerm = shadeColorFactor * texture(shadeMultiplyTexture, uv)`).
/// All variants use the procedural quadrant checkerboard texture; the
/// sweep axis is the surrounding shading state that determines how
/// much of the rendered sphere is in the shadowed (texture-visible)
/// region.
///
/// Variants:
/// - `mtoon_shadetex_baseline` — `shadeMultiplyTexture = None`,
///   `shadeColorFactor = [0.5, 0.5, 0.5]` (the suite's MToon default).
///   The visual baseline: shaded portion of the sphere renders as
///   plain mid-gray. Difference between baseline and any textured
///   variant proves the renderer reads `shadeMultiplyTexture`.
/// - `mtoon_shadetex_default` — same default shading, but with the
///   checkerboard texture on `shadeMultiplyTexture`. Lit half stays
///   white from `baseColorFactor=[1,1,1]`; shaded half gets the
///   texture × `shadeColorFactor=[0.5,0.5,0.5]` blend.
/// - `mtoon_shadetex_white_tint` — `shadeColorFactor = [1, 1, 1]`,
///   textured. With no tint, the shaded half should display the
///   raw checkerboard colors directly. Maximally visible signal.
/// - `mtoon_shadetex_red_tint` — `shadeColorFactor = [1, 0, 0]`,
///   textured. Tests the multiplicative blending: red × {red, green,
///   blue, yellow} should produce {red, black, black, red} in the
///   shaded region.
/// - `mtoon_shadetex_shift_neg0p5` — `shadingShiftFactor = -0.5`,
///   pushing more of the sphere into shadow. More texture visible.
/// - `mtoon_shadetex_shift_pos0p5` — `shadingShiftFactor = 0.5`,
///   pulling the sphere out of shadow. Less texture visible (lit
///   region dominates).
pub fn mtoon_shade_multiply_texture_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Baseline: no shade texture, default shadeColorFactor mid-gray.
    let mut baseline = MToonParams::defaults("mtoon_shadetex_baseline");
    // Explicitly false so the conditional-emit guard in emit_vrm
    // produces a no-texture render (validator + visual-diff baseline).
    baseline.shade_multiply_texture = false;
    out.push(baseline);

    // Textured + default shading: visible checkerboard in shaded half,
    // tinted by shadeColorFactor=[0.5,0.5,0.5].
    let mut default_textured = MToonParams::defaults("mtoon_shadetex_default");
    default_textured.shade_multiply_texture = true;
    out.push(default_textured);

    // Textured + white shadeColorFactor: raw checkerboard colors in
    // shaded region. Maximum visible signal — easiest to inspect.
    let mut white_tint = MToonParams::defaults("mtoon_shadetex_white_tint");
    white_tint.shade_multiply_texture = true;
    white_tint.shade_color_factor = [1.0, 1.0, 1.0];
    out.push(white_tint);

    // Textured + red shadeColorFactor: only red channel survives in
    // shaded region. Per spec multiplicative blending, the green and
    // blue quadrants should darken to near-black, while red and yellow
    // (red+green) should keep their red component.
    let mut red_tint = MToonParams::defaults("mtoon_shadetex_red_tint");
    red_tint.shade_multiply_texture = true;
    red_tint.shade_color_factor = [1.0, 0.0, 0.0];
    out.push(red_tint);

    // Textured + shadingShift -0.5: more of the sphere in shadow,
    // larger area covered by texture.
    let mut shifted_more_shadow = MToonParams::defaults("mtoon_shadetex_shift_neg0p5");
    shifted_more_shadow.shade_multiply_texture = true;
    shifted_more_shadow.shade_color_factor = [1.0, 1.0, 1.0];
    shifted_more_shadow.shading_shift_factor = -0.5;
    out.push(shifted_more_shadow);

    // Textured + shadingShift +0.5: most of the sphere out of shadow,
    // texture barely visible.
    let mut shifted_less_shadow = MToonParams::defaults("mtoon_shadetex_shift_pos0p5");
    shifted_less_shadow.shade_multiply_texture = true;
    shifted_less_shadow.shade_color_factor = [1.0, 1.0, 1.0];
    shifted_less_shadow.shading_shift_factor = 0.5;
    out.push(shifted_less_shadow);

    out
}

/// KHR_texture_transform sweep: 8 variants exercising the spec's three
/// transform axes (offset, rotation, scale) on a textured MToon
/// material. The texture itself is a 16×16 quadrant checkerboard
/// (red TL, green TR, blue BL, yellow BR) added by `emit_vrm` when
/// `texture_transform` is `Some` — the four distinct colors make
/// transform effects visually unambiguous (e.g., a rotation that
/// swaps red→bottom would have been silently identical with a uniform
/// texture).
///
/// Variant ID conventions match other sweeps:
/// - `mtoon_uvxform_identity` — textured material, no transform (the
///   "extension absent" baseline; verifies textures-themselves work
///   independently from KHR_texture_transform).
/// - `mtoon_uvxform_offset_x_0p5` — half-tile UV shift along X.
/// - `mtoon_uvxform_offset_y_0p5` — half-tile UV shift along Y.
/// - `mtoon_uvxform_rotation_quarter` — π/2 rad rotation (90° ccw).
/// - `mtoon_uvxform_rotation_eighth` — π/4 rad rotation (45° ccw).
/// - `mtoon_uvxform_scale_2x` — 2× tile (each quadrant appears 4×).
/// - `mtoon_uvxform_scale_half` — 0.5× tile (one quadrant fills the UV).
/// - `mtoon_uvxform_combined` — offset + rotation + scale stacked,
///   exercises the multiplication order in the spec's example shader.
pub fn mtoon_texture_transform_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();
    let variants: &[(&str, TextureTransform)] = &[
        ("identity", TextureTransform::identity()),
        (
            "offset_x_0p5",
            TextureTransform {
                offset: [0.5, 0.0],
                rotation: 0.0,
                scale: [1.0, 1.0],
            },
        ),
        (
            "offset_y_0p5",
            TextureTransform {
                offset: [0.0, 0.5],
                rotation: 0.0,
                scale: [1.0, 1.0],
            },
        ),
        (
            "rotation_quarter",
            TextureTransform {
                offset: [0.0, 0.0],
                rotation: std::f32::consts::FRAC_PI_2,
                scale: [1.0, 1.0],
            },
        ),
        (
            "rotation_eighth",
            TextureTransform {
                offset: [0.0, 0.0],
                rotation: std::f32::consts::FRAC_PI_4,
                scale: [1.0, 1.0],
            },
        ),
        (
            "scale_2x",
            TextureTransform {
                offset: [0.0, 0.0],
                rotation: 0.0,
                scale: [2.0, 2.0],
            },
        ),
        (
            "scale_half",
            TextureTransform {
                offset: [0.0, 0.0],
                rotation: 0.0,
                scale: [0.5, 0.5],
            },
        ),
        (
            "combined",
            TextureTransform {
                offset: [0.25, 0.25],
                rotation: std::f32::consts::FRAC_PI_4,
                scale: [2.0, 2.0],
            },
        ),
    ];
    for (suffix, tt) in variants {
        let mut p = MToonParams::defaults(format!("mtoon_uvxform_{suffix}"));
        p.texture_transform = Some(*tt);
        out.push(p);
    }
    out
}

/// VRMC_vrm.firstPerson sweep: 4 variants, one per spec-defined
/// `meshAnnotations[*].type` enum value (`auto`, `both`,
/// `thirdPersonOnly`, `firstPersonOnly`). All other parameters at
/// MToon defaults so the only axis under test is the firstPerson
/// annotation on the avatar's single mesh node.
///
/// Expected third-person rendering signal (the suite's standard camera
/// is third-person at `[0, 1.4, 1.5]` looking at `[0, 1.4, 0]`):
///
/// | firstPerson.type   | head mesh in third-person render | rationale                                      |
/// |--------------------|----------------------------------|------------------------------------------------|
/// | `auto`             | visible                          | sphere isn't bone-weighted to head → defaults to renderable (most adapters) |
/// | `both`             | visible                          | "visible from every camera"                    |
/// | `thirdPersonOnly`  | visible                          | exactly what third-person camera should show   |
/// | `firstPersonOnly`  | culled                           | hidden from non-first-person cameras per spec  |
///
/// Conformance signal: renderers that correctly implement firstPerson
/// culling produce a distinct (mostly-empty) `firstPersonOnly` PNG and
/// roughly-identical PNGs for the other three. Renderers that ignore
/// firstPerson annotations produce four identical PNGs — a clear
/// non-conformance.
///
/// Note: this sweep exercises the third-person rendering path only.
/// First-person culling (the reverse case, where `thirdPersonOnly`
/// should be culled and `firstPersonOnly` should be visible) requires
/// a camera-mode field on `set_camera` that we don't have yet — that's
/// a follow-up RFC. For now the third-person path alone is the
/// minimum-viable signal.
pub fn mtoon_first_person_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();
    for t in [
        FirstPersonType::Auto,
        FirstPersonType::Both,
        FirstPersonType::ThirdPersonOnly,
        FirstPersonType::FirstPersonOnly,
    ] {
        let id = format!("mtoon_firstperson_{}", t.as_spec_str());
        let mut p = MToonParams::defaults(id);
        p.first_person_type = Some(t);
        out.push(p);
    }
    out
}

/// VRM 0.x MToon basic sweep — slice 1 of the conformance corpus.
///
/// 3 variants:
/// - `mtoon_basic_v0_lit_001` — neutral lit baseline; Applicable.
/// - `mtoon_basic_v0_shadeShift_neg05` — shadingShift -0.5; Applicable.
/// - `mtoon_basic_v0_outline_lighting_mix` — registered as
///   `NotApplicable { reason: OutlineLightingMixV1Only }` because 0.x has no
///   `_OutlineLightingMix` Unity-shader key (v1-only axis).
///
/// Each Applicable variant has a counterpart in `mtoon_basic_sweep` (1.0).
pub fn mtoon_basic_v0_sweep() -> Vec<(MToonParams, crate::SweepApplicability)> {
    let mut out = Vec::new();

    let lit = MToonParams::defaults("mtoon_basic_v0_lit_001");
    out.push((lit, crate::SweepApplicability::Applicable));

    let mut shade_shift = MToonParams::defaults("mtoon_basic_v0_shadeShift_neg05");
    shade_shift.shading_shift_factor = -0.5;
    out.push((shade_shift, crate::SweepApplicability::Applicable));

    let mut lighting_mix = MToonParams::defaults("mtoon_basic_v0_outline_lighting_mix");
    // outlineLightingMix is only meaningful in VRM 1.0; in 0.x there is no
    // corresponding Unity-shader key. We set the field to a non-default
    // value to make the intent explicit; the runner uses the .skipped.json
    // marker produced for this variant and never reads the MToonParams.
    lighting_mix.outline_lighting_mix_factor = 0.5;
    out.push((
        lighting_mix,
        crate::SweepApplicability::NotApplicable {
            reason: crate::NotApplicableReason::OutlineLightingMixV1Only,
        },
    ));

    out
}

/// Material-name render-classification reproducer (the VMK `Vita_clothing`
/// z-fighting quirk). Emits ONE MToon material under heuristic-tripping names
/// (`*clothing*`, `*skirt*`) vs control names (`*plain*`, `*body*`) crossed
/// with the glTF `doubleSided` flag. Every variant is byte-identical except
/// `id` (which becomes `material.name`, see mtoon_v0.rs:26 / vrm_ext.rs:324)
/// and `double_sided`. A conformant renderer's output is invariant to the
/// material name at a fixed `doubleSided`; a name-heuristic renderer (VMK:
/// `cloth` → forced double-sided + overlay depth bias) diverges on the trip
/// tokens. See docs/superpowers/specs/2026-05-28-material-name-classification-reproducer.md.
pub fn material_name_classification_sweep() -> Vec<MToonParams> {
    let mut out = Vec::new();

    // Single-sided group: all should render identically; VMK diverges on the
    // names that contain a clothing token.
    out.push(MToonParams::defaults("matname_plain_singlesided"));
    out.push(MToonParams::defaults("matname_clothing_singlesided"));
    out.push(MToonParams::defaults("matname_skirt_singlesided"));

    // Double-sided group: the plain/clothing pair should match each other.
    let mut plain_ds = MToonParams::defaults("matname_plain_doublesided");
    plain_ds.double_sided = true;
    out.push(plain_ds);
    let mut cloth_ds = MToonParams::defaults("matname_clothing_doublesided");
    cloth_ds.double_sided = true;
    out.push(cloth_ds);

    // Control for a different name-category path (`body` → not "clothing").
    out.push(MToonParams::defaults("matname_body_singlesided"));

    out
}

fn fmt_num<T: std::fmt::Display + Copy + PartialOrd + Default>(v: T) -> String
where
    f64: From<T>,
{
    let f = f64::from(v);
    let s = format!("{f:.3}").replace('.', "p").replace('-', "neg");
    s.trim_end_matches('0').trim_end_matches('p').to_string()
}

fn fmt_signed(v: f32) -> String {
    if v < 0.0 {
        format!("neg{}", fmt_num(-v))
    } else {
        fmt_num(v)
    }
}

/// 7-variant per-joint taper sweep: 4 stiffness shapes + 3 drag shapes.
/// All variants use joint_count=4 and set exactly one per-joint vector so
/// regression regressions are unconfounded on the taper axis.
pub fn spring_bone_taper_sweep() -> Vec<SpringBoneParams> {
    let mut out = Vec::with_capacity(7);
    let n: u32 = 4; // joint_count used throughout this sweep

    // Stiffness tapers (4 shapes):
    let stiffness_shapes: [(&str, Vec<f32>); 4] = [
        ("stiffness_flat", vec![0.5, 0.5, 0.5, 0.5]),
        ("stiffness_high_to_low", vec![1.0, 0.7, 0.4, 0.1]),
        ("stiffness_low_to_high", vec![0.1, 0.4, 0.7, 1.0]),
        ("stiffness_expdecay", vec![1.0, 0.5, 0.25, 0.125]),
    ];
    for (suffix, vec) in stiffness_shapes {
        let mut p = SpringBoneParams::defaults(format!("springbone_taper_{suffix}"));
        p.joint_count = n;
        p.stiffness_per_joint = Some(vec);
        out.push(p);
    }

    // Drag tapers (3 shapes — flat omitted since it would duplicate the stiffness_flat baseline):
    let drag_shapes: [(&str, Vec<f32>); 3] = [
        ("drag_flat", vec![0.5, 0.5, 0.5, 0.5]),
        ("drag_high_to_low", vec![1.0, 0.7, 0.4, 0.1]),
        ("drag_expdecay", vec![1.0, 0.5, 0.25, 0.125]),
    ];
    for (suffix, vec) in drag_shapes {
        let mut p = SpringBoneParams::defaults(format!("springbone_taper_{suffix}"));
        p.joint_count = n;
        p.drag_force_per_joint = Some(vec);
        out.push(p);
    }

    out
}

/// 4-variant one-axis sweep over `gravity_dir`, holding all other
/// `SpringBoneParams` at defaults. Directions: -Y (default), +Y (anti-gravity),
/// +X (sideways), and a 45° oblique (+0.7, -0.7, 0). Each variant changes only
/// the gravity direction so adapter regressions on this axis are unconfounded.
pub fn spring_bone_gravity_dir_sweep() -> Vec<SpringBoneParams> {
    let directions = [
        ("default", [0.0_f32, -1.0, 0.0]),
        ("anti", [0.0, 1.0, 0.0]),
        ("sideways", [1.0, 0.0, 0.0]),
        ("oblique", [0.7, -0.7, 0.0]),
    ];

    directions
        .iter()
        .map(|(name, dir)| {
            let mut p = SpringBoneParams::defaults(format!("springbone_gravity_dir_{name}"));
            p.gravity_dir = *dir;
            p
        })
        .collect()
}

/// 24-variant Cartesian sweep: 2 shapes × 4 offset_x values × 3 radii.
///
/// **Why lateral X offsets, not Y offsets.** An earlier draft of this sweep
/// varied `offset: [0, y, 0]` along the chain axis. With the chain anchored
/// at the head and hanging straight down through (x=0, z=0), a collider
/// placed on that same vertical axis sits at distance zero from every chain
/// joint center — there is no off-axis component, so the spring system has
/// no lateral force to apply during settle. Every variant produced
/// byte-identical PNGs (chain falls straight through collider center).
///
/// Lateral X offsets break the symmetry: the chain's straight-down pose
/// either contacts the collider sphere (small `|x|`, large radius) and
/// must bend around it, or misses it entirely (large `|x|`, small radius).
/// The 4 offsets here span both regimes so the sweep covers contact and
/// no-contact cases per radius.
///
/// `offset_y` is held at -0.10 m (middle of a 0.20 m chain's vertical span)
/// so the collider intersects the chain's vertical reach.
pub fn spring_bone_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(24);

    let offsets_x = [-0.05_f32, -0.02, 0.02, 0.05];
    let radii = [0.03_f32, 0.05, 0.10];
    let fixed_y = -0.10_f32;

    for shape_kind in ["sphere", "capsule"].iter() {
        for &off_x in offsets_x.iter() {
            for &radius in radii.iter() {
                let id = format!(
                    "springbone_collider_{}_x{}_r{}",
                    shape_kind,
                    fmt_signed(off_x),
                    fmt_num(radius),
                );
                let shape = match *shape_kind {
                    "sphere" => ColliderShape::Sphere { radius },
                    "capsule" => ColliderShape::Capsule {
                        radius,
                        tail_offset: [0.0, -0.05, 0.0],
                    },
                    _ => unreachable!(),
                };
                let collider = ColliderParams {
                    shape,
                    offset: [off_x, fixed_y, 0.0],
                    attach: ColliderAttach::Head,
                };
                let scene = SpringBoneSceneParams {
                    springs: vec![SpringBoneParams::defaults(&id)],
                    colliders: vec![collider],
                    collider_groups: vec![ColliderGroupParams {
                        name: "head_g".into(),
                        collider_indices: vec![0],
                    }],
                    spring_collider_groups: vec![vec![0]],
                };
                let mtoon = MToonParams::defaults(&id);
                out.push((mtoon, scene));
            }
        }
    }

    out
}

/// Build a (ColliderShape, offset) pair for the extended collider at the
/// given shape name and placement index (0=tight, 1=medium, 2=loose).
fn make_extended_shape_with_placement(shape_name: &str, p_idx: usize) -> (ColliderShape, [f32; 3]) {
    match shape_name {
        "plane" => {
            // Plane normal stays +Y; vary the Y offset to place at different depths.
            let offsets = [-0.04_f32, -0.08, -0.15];
            (
                ColliderShape::Plane {
                    normal: [0.0, 1.0, 0.0],
                },
                [0.0, offsets[p_idx], 0.0],
            )
        }
        "isphere" => {
            // Inside-sphere: vary radius (tight=small, loose=large). All three
            // must be smaller than the chain's deepest-joint distance from the
            // collider node (≈0.10 m for the default 4-joint × 0.05 m chain) so
            // the containment constraint actually engages — VMK#237 diagnostic
            // (2026-05-24) showed earlier radii [0.10, 0.20, 0.40] only
            // engaged at ptight=0.10, leaving the other 5 variants as physical
            // no-ops that collapsed to a single SHA bucket. {0.04, 0.06, 0.08}
            // produces three distinct engagement depths.
            let radii = [0.04_f32, 0.06, 0.08];
            (
                ColliderShape::InsideSphere {
                    radius: radii[p_idx],
                },
                [0.0, -0.10, 0.0],
            )
        }
        "icaps" => {
            // Inside-capsule: same radii rationale as isphere above.
            let radii = [0.04_f32, 0.06, 0.08];
            (
                ColliderShape::InsideCapsule {
                    radius: radii[p_idx],
                    tail_offset: [0.0, 0.30, 0.0],
                },
                [0.0, -0.10, 0.0],
            )
        }
        _ => unreachable!(),
    }
}

fn build_extended_scene(
    id: &str,
    shape: ColliderShape,
    offset: [f32; 3],
    angle_limit: Option<f32>,
) -> SpringBoneSceneParams {
    let mut spring = SpringBoneParams::defaults(id);
    spring.joint_angle_limit_deg = angle_limit;
    let collider = ColliderParams {
        shape,
        offset,
        attach: ColliderAttach::Head,
    };
    SpringBoneSceneParams {
        springs: vec![spring],
        colliders: vec![collider],
        collider_groups: vec![ColliderGroupParams {
            name: "ext_g".into(),
            collider_indices: vec![0],
        }],
        spring_collider_groups: vec![vec![0]],
    }
}

/// 18-variant extended collider sweep:
/// - First 9: 3 shapes (plane, isphere, icaps) × 3 placements (tight, med, loose), no angle limit.
/// - Second 9: 3 shapes × 3 angle limits (30°, 60°, 90°) at medium placement.
pub fn spring_bone_extended_collider_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(18);
    let shape_names = ["plane", "isphere", "icaps"];
    let placement_keys = ["tight", "med", "loose"];

    // First 9: shape × placement, no angle limit.
    for shape_name in shape_names.iter() {
        for (p_idx, p_key) in placement_keys.iter().enumerate() {
            let id = format!("springbone_extended_{shape_name}_p{p_key}");
            let (shape, offset) = make_extended_shape_with_placement(shape_name, p_idx);
            let scene = build_extended_scene(&id, shape, offset, None);
            out.push((MToonParams::defaults(&id), scene));
        }
    }

    // Second 9: shape × angle limit (30, 60, 90), medium placement.
    for shape_name in shape_names.iter() {
        for &deg in [30.0_f32, 60.0, 90.0].iter() {
            let id = format!("springbone_extended_{shape_name}_anglelimit_{}", deg as i32);
            let (shape, offset) = make_extended_shape_with_placement(shape_name, 1);
            let scene = build_extended_scene(&id, shape, offset, Some(deg));
            out.push((MToonParams::defaults(&id), scene));
        }
    }

    out
}

/// 18-variant multi-chain sweep: 3 chain counts × 2 spacings × 3 sharing modes.
///
/// - Chain counts: 2, 3, 5
/// - Spacing: 0.02 m, 0.05 m (encoded in the ID; emit.rs uses a fixed 0.05 m radial
///   spacing at emit time — see docs/findings.md phase-6 limitation note)
/// - Sharing modes: share_all (single collider group referenced by all chains),
///   share_none (per-chain group, each pointing at the same collider), share_alt
///   (two groups, chains alternate between them)
///
/// Each variant carries one trivial sphere collider so the sharing semantics are
/// non-vacuous (the groups reference a real collider even if it is minimal).
pub fn spring_bone_multichain_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(18);

    let chain_counts: [u32; 3] = [2, 3, 5];
    let spacings_m: [f32; 2] = [0.02, 0.05];
    let sharing_modes = ["share_all", "share_none", "share_alt"];

    for &cc in chain_counts.iter() {
        for &sp in spacings_m.iter() {
            for &mode in sharing_modes.iter() {
                let sp_str = fmt_num(sp as f64);
                let id = format!("springbone_multichain_n{cc}_sp{sp_str}_{mode}");
                let scene = build_multichain_scene(&id, cc, mode);
                out.push((MToonParams::defaults(&id), scene));
            }
        }
    }
    out
}

fn build_multichain_scene(id: &str, chain_count: u32, sharing_mode: &str) -> SpringBoneSceneParams {
    let springs: Vec<SpringBoneParams> = (0..chain_count)
        .map(|i| SpringBoneParams::defaults(format!("{id}_chain_{i}")))
        .collect();

    // One sphere collider lateral to the chain axis so the group-sharing axis
    // is non-vacuous AND produces visual signal (chain bends around it). An
    // earlier draft used offset=[0,0,0] which placed the collider on every
    // chain's vertical axis; with no off-axis component, settle pose was
    // identical regardless of sharing mode. Lateral offset at y=-0.10 puts the
    // collider in the chain's lateral path.
    let trivial_collider = ColliderParams {
        shape: ColliderShape::Sphere { radius: 0.04 },
        offset: [0.03, -0.10, 0.0],
        attach: ColliderAttach::Head,
    };

    let (collider_groups, spring_collider_groups) = match sharing_mode {
        "share_all" => {
            // Single group; every chain references it.
            let groups = vec![ColliderGroupParams {
                name: "shared".into(),
                collider_indices: vec![0],
            }];
            let sgs = (0..chain_count).map(|_| vec![0_usize]).collect();
            (groups, sgs)
        }
        "share_none" => {
            // N groups, each pointed at the same single collider; one group per chain.
            let groups = (0..chain_count)
                .map(|i| ColliderGroupParams {
                    name: format!("g{i}"),
                    collider_indices: vec![0],
                })
                .collect();
            let sgs = (0..chain_count).map(|i| vec![i as usize]).collect();
            (groups, sgs)
        }
        "share_alt" => {
            // 2 groups; chains alternate which group they reference.
            let groups = vec![
                ColliderGroupParams {
                    name: "even".into(),
                    collider_indices: vec![0],
                },
                ColliderGroupParams {
                    name: "odd".into(),
                    collider_indices: vec![0],
                },
            ];
            let sgs = (0..chain_count).map(|i| vec![(i as usize) % 2]).collect();
            (groups, sgs)
        }
        _ => unreachable!("unknown sharing mode {sharing_mode}"),
    };

    SpringBoneSceneParams {
        springs,
        colliders: vec![trivial_collider],
        collider_groups,
        spring_collider_groups,
    }
}

// ── CCD / tunneling collider sweep ──────────────────────────────────────────

/// World-fixed collider specification returned by [`ccd_collider_world_spec`].
///
/// Contains enough information for Task 7's sidecar to emit a matching
/// `ccd_colliders` entry on the test plan, and for Task 8's mock E2E to
/// validate placement.
///
/// For spheres, `center` is the world position of the sphere center and
/// `capsule_head`/`capsule_tail` are `None`.
///
/// For capsules, `center` is the node world position (the
/// `ColliderAttach::WorldCoordinates` value), `capsule_head` is the world
/// position of the capsule's head endpoint (`node + offset`), and
/// `capsule_tail` is the world position of the tail endpoint
/// (`node + tail_offset`). These match the values a renderer computes from
/// the `.vrm`'s `VRMC_springBone` collider entry:
/// `transformedOffset = offset * nodeWorldMatrix` and
/// `transformedTail = tail * nodeWorldMatrix` (VRM spec pseudocode).
#[derive(Debug, Clone)]
pub struct CcdColliderWorldSpec {
    /// `"sphere"` or `"capsule"`.
    pub kind: &'static str,
    /// World center: sphere center for spheres; the collider node's world
    /// translation for capsules.
    pub center: [f32; 3],
    pub radius: f32,
    /// Capsule head endpoint in world space (`node_translation + offset`).
    /// `None` for spheres.
    pub capsule_head: Option<[f32; 3]>,
    /// Capsule tail endpoint in world space (`node_translation + tail_offset`).
    /// `None` for spheres.
    pub capsule_tail: Option<[f32; 3]>,
}

/// Return the world-space collider geometry for a CCD sweep cell. Task 7 calls
/// this to emit the matching `ccd_colliders` entry on the test plan; Task 8
/// uses it to validate placement.
///
/// The center is the same value used for `ColliderAttach::WorldCoordinates` —
/// it is a pure function of the scene params (no mutable state), so both the
/// asset generator and the sidecar generator agree without a separate lookup.
///
/// For capsules, `capsule_head` and `capsule_tail` are the true world
/// endpoints derived from the VRM file's node position and shape offsets:
/// - head = node_translation + collider.offset
/// - tail = node_translation + tail_offset   (both in node-local → world space)
///
/// With `offset = [0,0,0]` (always in CCD sweep), head = node_translation
/// and tail = node_translation + tail_offset.
pub fn ccd_collider_world_spec(scene: &SpringBoneSceneParams) -> CcdColliderWorldSpec {
    assert_eq!(
        scene.colliders.len(),
        1,
        "ccd sweep cells have exactly one collider"
    );
    let c = &scene.colliders[0];
    let center = match &c.attach {
        ColliderAttach::WorldCoordinates { center } => *center,
        _ => panic!("ccd sweep collider must use WorldCoordinates attach"),
    };
    match &c.shape {
        ColliderShape::Sphere { radius } => CcdColliderWorldSpec {
            kind: "sphere",
            center,
            radius: *radius,
            capsule_head: None,
            capsule_tail: None,
        },
        ColliderShape::Capsule {
            radius,
            tail_offset,
        } => {
            // head = node_translation + offset (offset is [0,0,0] in CCD sweep)
            let head = [
                center[0] + c.offset[0],
                center[1] + c.offset[1],
                center[2] + c.offset[2],
            ];
            // tail = node_translation + tail_offset
            let tail = [
                center[0] + tail_offset[0],
                center[1] + tail_offset[1],
                center[2] + tail_offset[2],
            ];
            CcdColliderWorldSpec {
                kind: "capsule",
                center,
                radius: *radius,
                capsule_head: Some(head),
                capsule_tail: Some(tail),
            }
        }
        _ => panic!("ccd sweep collider must be sphere or capsule"),
    }
}

/// 12-variant CCD / tunneling sweep.
///
/// ## Geometry rationale
///
/// The default spring-bone chain (`SpringBoneParams::defaults`) hangs from the
/// head bone at world position (0, 1.36, 0) straight down along −Y:
/// - Joint 0: (0, 1.31, 0)  …  Joint 3: (0, 1.16, 0)
/// - Chain midpoint: (0, 1.26, 0)
///
/// The world-fixed collider is placed at **(0.10, 1.26, 0.0)** — 10 cm lateral
/// of the chain's rest column at mid-chain height. When Task 7's sidecar sweeps
/// the avatar root +X by ≥ 0.10 m in one frame (fast) or incrementally in
/// sub-frame steps (slow), the chain crosses the collider. Sphere / capsule ×
/// radius × speed tag = 12 cells.
///
/// ## Speed tag
///
/// Speed is encoded in the ID (`…_fast` / `…_slow`) so Task 7 can key off
/// the asset ID and set matching root velocity. There is no speed field in
/// `SpringBoneSceneParams` (avoids schema churn); Task 7 parses the suffix.
///
/// - `fast`: large step per frame (~0.083 m = 1.0 m / 12 frames), above the
///   CCD tunneling threshold for small radii — a non-CCD integrator will
///   tunnel through thin colliders.
/// - `slow`: small step per frame (~0.0083 m = 1.0 m / 120 frames), well
///   below the threshold — all compliant integrators should detect the
///   collision regardless of CCD.
///
/// ## Threshold straddle
///
/// Radii `{0.005, 0.02, 0.05}` straddle the practical CCD threshold:
/// - 0.005 m (5 mm): smaller than the hit_radius (0.02 m) on "fast" cells →
///   a non-CCD integrator may tunnel through.
/// - 0.02 m: marginal — matches the joint hit_radius exactly.
/// - 0.05 m: large enough that even fast displacement is caught without CCD.
///
/// ## Cell count: 2 shapes × 3 radii × 2 speeds = 12 cells
pub fn spring_bone_ccd_sweep() -> Vec<(MToonParams, SpringBoneSceneParams)> {
    let mut out = Vec::with_capacity(12);

    // World-fixed collider center: 10 cm lateral of the chain's rest column,
    // at chain mid-height. See geometry rationale in the doc comment above.
    let world_center: [f32; 3] = [0.10, 1.26, 0.0];

    let radii = [0.005_f32, 0.02, 0.05];
    let speeds = ["fast", "slow"];
    let shape_kinds = ["sphere", "capsule"];

    for &shape_kind in shape_kinds.iter() {
        for &radius in radii.iter() {
            for &speed in speeds.iter() {
                let radius_tag = fmt_num(radius as f64);
                let id = format!("ccd_{shape_kind}_r{radius_tag}_{speed}");
                let shape = match shape_kind {
                    "sphere" => ColliderShape::Sphere { radius },
                    "capsule" => ColliderShape::Capsule {
                        radius,
                        // Capsule axis along Y, half-length 0.05 m each side.
                        tail_offset: [0.0, -0.10, 0.0],
                    },
                    _ => unreachable!(),
                };
                let collider = ColliderParams {
                    shape,
                    // With WorldCoordinates, offset is [0,0,0]: the node
                    // translation IS the world position.
                    offset: [0.0, 0.0, 0.0],
                    attach: ColliderAttach::WorldCoordinates {
                        center: world_center,
                    },
                };
                let scene = SpringBoneSceneParams {
                    springs: vec![SpringBoneParams::defaults(&id)],
                    colliders: vec![collider],
                    collider_groups: vec![ColliderGroupParams {
                        name: "ccd_g".into(),
                        collider_indices: vec![0],
                    }],
                    spring_collider_groups: vec![vec![0]],
                };
                let mtoon = MToonParams::defaults(&id);
                out.push((mtoon, scene));
            }
        }
    }

    out
}

pub fn vrma_lookat_sweep() -> Vec<crate::vrma_params::VrmaLookAtParams> {
    use crate::vrma_params::{AvatarLookAtType, RotationAxis, VrmaLookAtParams};

    let directions: [(&str, RotationAxis, f32); 5] = [
        ("yaw_neg60", RotationAxis::Y, -60.0),
        ("yaw_pos60", RotationAxis::Y, 60.0),
        ("pitch_neg30", RotationAxis::X, -30.0),
        ("pitch_pos30", RotationAxis::X, 30.0),
        ("neutral", RotationAxis::Y, 0.0),
    ];
    let configs = [AvatarLookAtType::Bone, AvatarLookAtType::Expression];

    let mut out = Vec::new();
    for (dir_name, axis, angle) in directions {
        for config in configs {
            let config_str = match config {
                AvatarLookAtType::Bone => "bone",
                AvatarLookAtType::Expression => "expr",
            };
            out.push(VrmaLookAtParams {
                id: format!("vrma_lookat_{dir_name}_{config_str}"),
                axis,
                angle_deg: angle,
                avatar_lookat_type: config,
                duration_s: 1.0,
            });
        }
    }
    out
}

/// Real-avatar gaze sweep (8 clips) covering VMK 0.17.1 #332. Five neutral-body
/// gaze directions exercise the eye-rest-composition bug (wall-eye at center,
/// inverted side gaze); three turned-head variants exercise head-local gaze
/// resolution. Sign convention follows `vrma_lookat_sweep` (gaze rotation vs
/// world frame). Each entry emits one `.vrma` (the avatar is the real VRoid
/// fixture, paired via a committed manual plan).
pub fn gaze_sweep() -> Vec<crate::vrma_params::GazeParams> {
    use crate::vrma_params::{GazeParams, RotationAxis};
    let g = |id: &str, axis: RotationAxis, angle: f32, body: f32| GazeParams {
        id: id.to_string(),
        gaze_axis: axis,
        gaze_angle_deg: angle,
        body_yaw_deg: body,
        duration_s: 1.0,
    };
    vec![
        // Bug B: eye-rest composition (neutral body).
        g("gaze_center", RotationAxis::Y, 0.0, 0.0),
        g("gaze_left", RotationAxis::Y, 30.0, 0.0),
        g("gaze_right", RotationAxis::Y, -30.0, 0.0),
        g("gaze_up", RotationAxis::X, 20.0, 0.0),
        g("gaze_down", RotationAxis::X, -20.0, 0.0),
        // Bug A: head-local resolution (turned head).
        g("gaze_center_bodyL", RotationAxis::Y, 0.0, 35.0),
        g("gaze_center_bodyR", RotationAxis::Y, 0.0, -35.0),
        g("gaze_right_bodyL", RotationAxis::Y, -30.0, 35.0),
    ]
}

pub fn vrma_expression_sweep() -> Vec<crate::vrma_params::VrmaExpressionParams> {
    use crate::vrma_params::VrmaExpressionParams;

    let presets = [
        "happy",
        "angry",
        "sad",
        "relaxed",
        "surprised",
        "aa",
        "ih",
        "ou",
        "ee",
        "oh",
        "blink",
    ];
    let custom = ["smug", "drowsy"];

    presets
        .iter()
        .map(|n| VrmaExpressionParams {
            id: format!("vrma_expression_preset_{n}"),
            expression_name: (*n).into(),
            is_preset: true,
            duration_s: 1.0,
        })
        .chain(custom.iter().map(|n| VrmaExpressionParams {
            id: format!("vrma_expression_custom_{n}"),
            expression_name: (*n).into(),
            is_preset: false,
            duration_s: 1.0,
        }))
        .collect()
}

/// Real-avatar expression clip sweep (11 preset clips) covering VMK 0.17.2 #333.
/// Reuses `VrmaExpressionParams`; ids are `expr_<name>` so manual plans pair the
/// real VRoid avatar with `expr_<name>.vrma`. Presets only — #333 is about preset
/// morph binds (blink, the five visemes, and the emotion presets); custom
/// expressions are out of scope.
pub fn expression_clip_sweep() -> Vec<crate::vrma_params::VrmaExpressionParams> {
    use crate::vrma_params::VrmaExpressionParams;
    [
        "blink",
        "happy",
        "angry",
        "sad",
        "relaxed",
        "surprised",
        "aa",
        "ih",
        "ou",
        "ee",
        "oh",
    ]
    .iter()
    .map(|n| VrmaExpressionParams {
        id: format!("expr_{n}"),
        expression_name: (*n).into(),
        is_preset: true,
        duration_s: 1.0,
    })
    .collect()
}

/// The 52 ARKit `ARFaceAnchor.BlendShapeLocation` raw names in canonical
/// lexicographic order. Mirrors Spatial's
/// `MotionStreamSchema.canonicalBlendShapeOrder` — producers exporting
/// VRMA from ARKit face capture (Spatial iOS) emit each of these as an
/// `expressions.custom` track keyed by this exact camelCase name.
pub fn arkit_blendshape_names() -> [&'static str; 52] {
    [
        "browDownLeft",
        "browDownRight",
        "browInnerUp",
        "browOuterUpLeft",
        "browOuterUpRight",
        "cheekPuff",
        "cheekSquintLeft",
        "cheekSquintRight",
        "eyeBlinkLeft",
        "eyeBlinkRight",
        "eyeLookDownLeft",
        "eyeLookDownRight",
        "eyeLookInLeft",
        "eyeLookInRight",
        "eyeLookOutLeft",
        "eyeLookOutRight",
        "eyeLookUpLeft",
        "eyeLookUpRight",
        "eyeSquintLeft",
        "eyeSquintRight",
        "eyeWideLeft",
        "eyeWideRight",
        "jawForward",
        "jawLeft",
        "jawOpen",
        "jawRight",
        "mouthClose",
        "mouthDimpleLeft",
        "mouthDimpleRight",
        "mouthFrownLeft",
        "mouthFrownRight",
        "mouthFunnel",
        "mouthLeft",
        "mouthLowerDownLeft",
        "mouthLowerDownRight",
        "mouthPressLeft",
        "mouthPressRight",
        "mouthPucker",
        "mouthRight",
        "mouthRollLower",
        "mouthRollUpper",
        "mouthShrugLower",
        "mouthShrugUpper",
        "mouthSmileLeft",
        "mouthSmileRight",
        "mouthStretchLeft",
        "mouthStretchRight",
        "mouthUpperUpLeft",
        "mouthUpperUpRight",
        "noseSneerLeft",
        "noseSneerRight",
        "tongueOut",
    ]
}

/// ARKit custom-expression sweep (52 variants — one per blendshape).
/// Each variant is a 0 → 1 → 0 weight ramp on one `expressions.custom`
/// track named with the raw ARKit camelCase name, paired with an avatar
/// that pre-registers that custom expression. Covers the producer surface
/// Spatial iOS exports (all 52 blendshapes, lossless, custom-classified).
pub fn vrma_arkit_expression_sweep() -> Vec<crate::vrma_params::VrmaExpressionParams> {
    use crate::vrma_params::VrmaExpressionParams;
    arkit_blendshape_names()
        .into_iter()
        .map(|name| VrmaExpressionParams {
            id: format!("vrma_arkit_{name}"),
            expression_name: name.into(),
            is_preset: false,
            duration_s: 1.0,
        })
        .collect()
}

pub fn vrma_humanoid_sweep() -> Vec<crate::vrma_params::VrmaHumanoidParams> {
    use crate::vrma_params::{RotationAxis, VrmaHumanoidParams};

    let entries: Vec<(&str, &str, RotationAxis, f32)> = vec![
        ("vrma_humanoid_hips_yaw_15", "hips", RotationAxis::Y, 15.0),
        ("vrma_humanoid_spine_yaw_30", "spine", RotationAxis::Y, 30.0),
        ("vrma_humanoid_head_yaw_45", "head", RotationAxis::Y, 45.0),
        ("vrma_humanoid_head_pitch_20", "head", RotationAxis::X, 20.0),
        ("vrma_humanoid_head_roll_15", "head", RotationAxis::Z, 15.0),
        (
            "vrma_humanoid_l_upperarm_yaw",
            "leftUpperArm",
            RotationAxis::Y,
            60.0,
        ),
        (
            "vrma_humanoid_l_upperarm_pitch",
            "leftUpperArm",
            RotationAxis::X,
            60.0,
        ),
        (
            "vrma_humanoid_l_upperarm_roll",
            "leftUpperArm",
            RotationAxis::Z,
            30.0,
        ),
        (
            "vrma_humanoid_r_upperarm_yaw",
            "rightUpperArm",
            RotationAxis::Y,
            -60.0,
        ),
        (
            "vrma_humanoid_r_upperarm_pitch",
            "rightUpperArm",
            RotationAxis::X,
            60.0,
        ),
        (
            "vrma_humanoid_r_upperarm_roll",
            "rightUpperArm",
            RotationAxis::Z,
            30.0,
        ),
        (
            "vrma_humanoid_l_upperleg_pitch",
            "leftUpperLeg",
            RotationAxis::X,
            45.0,
        ),
        (
            "vrma_humanoid_l_upperleg_yaw",
            "leftUpperLeg",
            RotationAxis::Y,
            15.0,
        ),
        (
            "vrma_humanoid_l_lowerleg_pitch",
            "leftLowerLeg",
            RotationAxis::X,
            60.0,
        ),
        ("vrma_humanoid_neck_yaw_30", "neck", RotationAxis::Y, 30.0),
    ];

    entries
        .into_iter()
        .map(|(id, bone, axis, angle)| VrmaHumanoidParams {
            id: id.into(),
            bone_name: bone.into(),
            axis,
            angle_deg: angle,
            duration_s: 1.0,
        })
        .collect()
}

/// Hips root-motion sweep (5 variants). One-axis-at-a-time: each variant
/// translates hips along a single world axis. Covers the VRMA spec's only
/// humanoid translation channel (root motion), which the rotation-only
/// `vrma_humanoid_sweep` never exercises.
pub fn vrma_hips_translation_sweep() -> Vec<crate::vrma_params::VrmaHipsTranslationParams> {
    use crate::vrma_params::VrmaHipsTranslationParams;
    // Rise kept smaller than crouch so the avatar stays in-frame and
    // crouch stays above the floor plane.
    let variants: [(&str, [f32; 3]); 5] = [
        ("vrma_hips_trans_forward", [0.0, 0.0, 0.3]),
        ("vrma_hips_trans_backward", [0.0, 0.0, -0.3]),
        ("vrma_hips_trans_lateral", [0.2, 0.0, 0.0]),
        ("vrma_hips_trans_crouch", [0.0, -0.15, 0.0]),
        ("vrma_hips_trans_rise", [0.0, 0.10, 0.0]),
    ];
    variants
        .into_iter()
        .map(|(id, offset)| VrmaHipsTranslationParams {
            id: id.into(),
            offset_m: offset,
            duration_s: 1.0,
        })
        .collect()
}

/// Multi-channel sweep (6 variants). Deliberate one-axis exception —
/// documented in each plan's spec_section. Variant 3 is the "double-drive"
/// surface (preset + ARKit-named custom coexisting, per Spatial's exporter
/// and PerfectSync-style avatars); variant 6 has all four channel kinds.
pub fn vrma_multichannel_sweep() -> Vec<crate::vrma_params::VrmaMultiChannelParams> {
    use crate::vrma_params::{
        BoneRotationSpec, ExpressionWeightSpec, GazeDirectionSpec, RotationAxis,
        VrmaMultiChannelParams,
    };

    fn bone(name: &str, axis: RotationAxis, angle: f32) -> BoneRotationSpec {
        BoneRotationSpec {
            bone_name: name.into(),
            axis,
            angle_deg: angle,
        }
    }
    fn expr(name: &str, is_preset: bool, peak: f32) -> ExpressionWeightSpec {
        ExpressionWeightSpec {
            name: name.into(),
            is_preset,
            peak_weight: peak,
        }
    }

    vec![
        VrmaMultiChannelParams {
            id: "vrma_multi_bone_hips".into(),
            bones: vec![bone("head", RotationAxis::Y, 30.0)],
            hips_offset_m: Some([0.0, 0.0, 0.2]),
            expressions: vec![],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_bone_expr".into(),
            bones: vec![bone("head", RotationAxis::Y, 30.0)],
            hips_offset_m: None,
            expressions: vec![expr("happy", true, 1.0)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_double_drive".into(),
            bones: vec![],
            hips_offset_m: Some([0.0, 0.0, 0.1]),
            expressions: vec![expr("happy", true, 1.0), expr("mouthSmileLeft", false, 1.0)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_body_face".into(),
            bones: vec![
                bone("spine", RotationAxis::Y, 20.0),
                bone("leftUpperArm", RotationAxis::X, 40.0),
            ],
            hips_offset_m: Some([0.0, 0.0, 0.2]),
            expressions: vec![expr("aa", true, 1.0)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_two_bones_two_exprs".into(),
            bones: vec![
                bone("head", RotationAxis::X, 20.0),
                bone("rightUpperArm", RotationAxis::Y, 30.0),
            ],
            hips_offset_m: None,
            expressions: vec![expr("blink", true, 1.0), expr("ou", true, 0.6)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_all_channels".into(),
            bones: vec![bone("head", RotationAxis::Y, 20.0)],
            hips_offset_m: Some([0.1, 0.0, 0.0]),
            expressions: vec![expr("happy", true, 0.8)],
            look_at: Some(GazeDirectionSpec {
                axis: RotationAxis::Y,
                angle_deg: 30.0,
            }),
            duration_s: 1.0,
        },
    ]
}

#[cfg(test)]
mod vrma_expression_sweep_tests {
    use super::*;

    #[test]
    fn expression_sweep_produces_13_variants() {
        let variants = vrma_expression_sweep();
        assert_eq!(variants.len(), 13, "11 presets + 2 custom = 13");
    }

    #[test]
    fn expression_sweep_unique_ids() {
        let variants = vrma_expression_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 13);
    }

    #[test]
    fn expression_sweep_includes_all_five_visemes() {
        let variants = vrma_expression_sweep();
        let names: std::collections::HashSet<_> =
            variants.iter().map(|p| p.expression_name.clone()).collect();
        for v in ["aa", "ih", "ou", "ee", "oh"] {
            assert!(
                names.contains(v),
                "viseme preset {v} must appear in vrma_expression_sweep()"
            );
        }
    }

    #[test]
    fn expression_sweep_preset_flag_correct() {
        let variants = vrma_expression_sweep();
        for v in &variants {
            if v.id.contains("_preset_") {
                assert!(v.is_preset, "{} should be preset", v.id);
            } else {
                assert!(!v.is_preset, "{} should be custom", v.id);
            }
        }
    }

    #[test]
    fn expression_sweep_all_duration_1s() {
        let variants = vrma_expression_sweep();
        for v in &variants {
            assert!(
                (v.duration_s - 1.0).abs() < 1e-6,
                "{}: duration should be 1.0",
                v.id
            );
        }
    }
}

#[cfg(test)]
mod vrma_hips_translation_sweep_tests {
    use super::*;

    #[test]
    fn five_single_direction_variants_with_unique_ids() {
        let variants = vrma_hips_translation_sweep();
        assert_eq!(variants.len(), 5);
        let mut ids: Vec<&str> = variants.iter().map(|v| v.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 5, "duplicate sweep ids");
        for v in &variants {
            let nonzero = v.offset_m.iter().filter(|c| c.abs() > f32::EPSILON).count();
            assert_eq!(nonzero, 1, "{}: one-axis-at-a-time violated", v.id);
            assert!(v.duration_s > 0.0);
        }
    }
}

#[cfg(test)]
mod taper_sweep_tests {
    use super::*;

    #[test]
    fn taper_sweep_produces_7_variants() {
        // 4 stiffness shapes + 3 drag shapes = 7.
        let variants = spring_bone_taper_sweep();
        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn taper_sweep_each_variant_has_a_per_joint_vector_set() {
        let variants = spring_bone_taper_sweep();
        for p in &variants {
            let has_per_joint = p.stiffness_per_joint.is_some()
                || p.drag_force_per_joint.is_some()
                || p.gravity_power_per_joint.is_some()
                || p.hit_radius_per_joint.is_some();
            assert!(
                has_per_joint,
                "{}: must have at least one per-joint vector",
                p.id
            );
        }
    }

    #[test]
    fn taper_sweep_vector_lengths_match_joint_count() {
        let variants = spring_bone_taper_sweep();
        for p in &variants {
            if let Some(v) = &p.stiffness_per_joint {
                assert_eq!(
                    v.len() as u32,
                    p.joint_count,
                    "{}: stiffness vector len mismatch",
                    p.id
                );
            }
            if let Some(v) = &p.drag_force_per_joint {
                assert_eq!(
                    v.len() as u32,
                    p.joint_count,
                    "{}: drag vector len mismatch",
                    p.id
                );
            }
        }
    }

    #[test]
    fn taper_sweep_unique_ids() {
        let variants = spring_bone_taper_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 7);
    }
}

#[cfg(test)]
mod extended_sweep_tests {
    use super::*;

    #[test]
    fn extended_sweep_produces_18_variants() {
        // 3 shapes × 3 placements = 9 (default angle, no limit)
        // 3 shapes × 3 angle_limits (30, 60, 90) at default placement = 9
        // Total: 18 base variants.
        let variants = spring_bone_extended_collider_sweep();
        assert_eq!(variants.len(), 18);
    }

    #[test]
    fn extended_sweep_unique_names() {
        let variants = spring_bone_extended_collider_sweep();
        let names: std::collections::HashSet<_> =
            variants.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(names.len(), 18);
    }

    #[test]
    fn extended_sweep_angle_limit_variants_actually_set_the_limit() {
        let variants = spring_bone_extended_collider_sweep();
        let limited: Vec<_> = variants
            .iter()
            .filter(|(_, s)| s.springs[0].joint_angle_limit_deg.is_some())
            .collect();
        assert_eq!(limited.len(), 9, "9 variants should carry angle limits");
    }
}

#[cfg(test)]
mod gravity_dir_sweep_tests {
    use super::*;

    #[test]
    fn gravity_dir_sweep_produces_4_variants() {
        let variants = spring_bone_gravity_dir_sweep();
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn gravity_dir_sweep_covers_4_distinct_directions() {
        let variants = spring_bone_gravity_dir_sweep();
        let dirs: std::collections::HashSet<[i32; 3]> = variants
            .iter()
            .map(|p| {
                let g = p.gravity_dir;
                // Multiply by 10 to compare with tolerance via integer hashing.
                [
                    (g[0] * 10.0) as i32,
                    (g[1] * 10.0) as i32,
                    (g[2] * 10.0) as i32,
                ]
            })
            .collect();
        assert_eq!(dirs.len(), 4, "all four directions must be distinct");
    }

    #[test]
    fn gravity_dir_sweep_baseline_first() {
        let variants = spring_bone_gravity_dir_sweep();
        assert_eq!(
            variants[0].gravity_dir,
            [0.0, -1.0, 0.0],
            "first variant should be the baseline -Y direction"
        );
    }

    #[test]
    fn gravity_dir_sweep_includes_anti_sideways_oblique() {
        let variants = spring_bone_gravity_dir_sweep();
        let has_antigravity = variants.iter().any(|p| p.gravity_dir == [0.0, 1.0, 0.0]);
        let has_sideways_x = variants.iter().any(|p| p.gravity_dir == [1.0, 0.0, 0.0]);
        let has_oblique = variants.iter().any(|p| {
            (p.gravity_dir[0] - 0.7).abs() < 1e-6 && (p.gravity_dir[1] - (-0.7)).abs() < 1e-6
        });
        assert!(
            has_antigravity && has_sideways_x && has_oblique,
            "must include anti, sideways, and oblique"
        );
    }

    #[test]
    fn gravity_dir_sweep_uses_unique_ids() {
        let variants = spring_bone_gravity_dir_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 4);
    }
}

#[cfg(test)]
mod collider_sweep_tests {
    use super::*;

    #[test]
    fn collider_sweep_produces_24_variants() {
        let variants = spring_bone_collider_sweep();
        assert_eq!(
            variants.len(),
            24,
            "Cartesian: 2 shapes × 4 offsets × 3 radii = 24"
        );
    }

    #[test]
    fn collider_sweep_variants_are_uniquely_named() {
        let variants = spring_bone_collider_sweep();
        let names: std::collections::HashSet<_> = variants
            .iter()
            .map(|(mtoon, _scene)| mtoon.id.clone())
            .collect();
        assert_eq!(names.len(), 24, "all variant IDs must be unique");
    }

    #[test]
    fn collider_sweep_uses_default_mtoon_constant_across_variants() {
        let variants = spring_bone_collider_sweep();
        let baseline_color = variants[0].0.base_color_factor;
        for (m, _) in &variants {
            assert_eq!(
                m.base_color_factor, baseline_color,
                "MToon must be held constant across collider sweep"
            );
        }
    }

    #[test]
    fn collider_sweep_each_variant_has_exactly_one_collider_group_per_spring() {
        let variants = spring_bone_collider_sweep();
        for (id, scene) in variants.iter().map(|(m, s)| (m.id.clone(), s)) {
            assert_eq!(scene.colliders.len(), 1, "{id}: expected 1 collider");
            assert_eq!(scene.collider_groups.len(), 1, "{id}: expected 1 group");
            assert_eq!(
                scene.spring_collider_groups[0],
                vec![0],
                "{id}: spring must reference group 0"
            );
        }
    }
}

#[cfg(test)]
mod multichain_sweep_tests {
    use super::*;

    #[test]
    fn multichain_sweep_produces_18_variants() {
        // 3 chain_count × 2 spacing × 3 sharing = 18.
        let variants = spring_bone_multichain_sweep();
        assert_eq!(variants.len(), 18);
    }

    #[test]
    fn multichain_sweep_each_variant_has_multiple_chains() {
        let variants = spring_bone_multichain_sweep();
        for (_, scene) in &variants {
            assert!(scene.springs.len() >= 2);
            assert!(scene.springs.len() <= 5);
        }
    }

    #[test]
    fn multichain_sweep_unique_ids() {
        let variants = spring_bone_multichain_sweep();
        let ids: std::collections::HashSet<_> =
            variants.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(ids.len(), 18);
    }

    #[test]
    fn multichain_sweep_sharing_modes_actually_share() {
        let variants = spring_bone_multichain_sweep();
        // For sharing="all", every chain references the same collider_group index.
        let all_share: Vec<_> = variants
            .iter()
            .filter(|(m, _)| m.id.contains("share_all"))
            .collect();
        for (_, scene) in all_share {
            let first = &scene.spring_collider_groups[0];
            for sg in &scene.spring_collider_groups {
                assert_eq!(
                    sg, first,
                    "share_all variants must point all chains at the same group"
                );
            }
        }
    }
}

#[cfg(test)]
mod shading_shift_texture_sweep_tests {
    use super::*;

    #[test]
    fn shading_shift_sweep_has_5_variants_with_unique_ids() {
        let s = mtoon_shading_shift_texture_sweep();
        assert_eq!(s.len(), 5);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len());
    }

    #[test]
    fn baseline_omits_shading_shift_texture() {
        let s = mtoon_shading_shift_texture_sweep();
        let b = s
            .iter()
            .find(|p| p.id == "mtoon_shadingshifttex_baseline")
            .unwrap();
        assert!(b.shading_shift_texture_scale.is_none());
    }

    #[test]
    fn non_baseline_variants_carry_a_scale() {
        let s = mtoon_shading_shift_texture_sweep();
        for p in &s {
            if p.id == "mtoon_shadingshifttex_baseline" {
                continue;
            }
            assert!(
                p.shading_shift_texture_scale.is_some(),
                "{}: must set shading_shift_texture_scale to exercise the binding",
                p.id
            );
        }
    }
}

#[cfg(test)]
mod rim_multiply_texture_sweep_tests {
    use super::*;

    #[test]
    fn rim_multiply_sweep_has_4_variants_with_unique_ids() {
        let s = mtoon_rim_multiply_texture_sweep();
        assert_eq!(s.len(), 4);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len());
    }

    #[test]
    fn every_variant_has_visible_rim_contribution() {
        let s = mtoon_rim_multiply_texture_sweep();
        for p in &s {
            assert_ne!(
                p.parametric_rim_color_factor,
                [0.0, 0.0, 0.0],
                "{}: parametricRimColorFactor must be non-zero or rim contribution is invisible",
                p.id
            );
            assert!(
                p.rim_lighting_mix_factor > 0.0,
                "{}: rimLightingMixFactor must be > 0 or rim doesn't blend in",
                p.id
            );
        }
    }
}

#[cfg(test)]
mod matcap_texture_sweep_tests {
    use super::*;

    #[test]
    fn matcap_sweep_has_5_variants_with_unique_ids() {
        let s = mtoon_matcap_texture_sweep();
        assert_eq!(s.len(), 5);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len(), "matcap sweep IDs must be unique");
    }

    #[test]
    fn baseline_omits_matcap_texture_and_non_baseline_sets_it() {
        let s = mtoon_matcap_texture_sweep();
        let baseline = s
            .iter()
            .find(|p| p.id == "mtoon_matcap_baseline")
            .expect("baseline variant missing");
        assert!(!baseline.matcap_texture);
        for p in &s {
            if p.id == "mtoon_matcap_baseline" {
                continue;
            }
            assert!(
                p.matcap_texture,
                "{}: non-baseline variant must set matcap_texture",
                p.id
            );
        }
    }

    #[test]
    fn every_matcap_variant_isolates_the_matcap_signal() {
        // base+shade colors must be near-black for every variant so the
        // matcap contribution dominates the rendered output. If a future
        // edit forgets one, the diff signal degrades.
        let s = mtoon_matcap_texture_sweep();
        for p in &s {
            let b = p.base_color_factor;
            let sc = p.shade_color_factor;
            assert!(
                b[0] <= 0.1 && b[1] <= 0.1 && b[2] <= 0.1,
                "{}: base_color_factor must be near-black",
                p.id
            );
            assert!(
                sc[0] == 0.0 && sc[1] == 0.0 && sc[2] == 0.0,
                "{}: shade_color_factor must be black",
                p.id
            );
        }
    }
}

#[cfg(test)]
mod shade_multiply_texture_sweep_tests {
    use super::*;

    #[test]
    fn shade_multiply_texture_sweep_has_6_variants_with_unique_ids() {
        let s = mtoon_shade_multiply_texture_sweep();
        assert_eq!(s.len(), 6);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len(), "shadetex sweep IDs must be unique");
    }

    #[test]
    fn baseline_variant_has_no_shade_texture() {
        let s = mtoon_shade_multiply_texture_sweep();
        let baseline = s
            .iter()
            .find(|p| p.id == "mtoon_shadetex_baseline")
            .expect("baseline variant missing");
        assert!(
            !baseline.shade_multiply_texture,
            "baseline must omit shadeMultiplyTexture so it renders as the no-texture diff target"
        );
    }

    #[test]
    fn every_non_baseline_variant_sets_the_texture() {
        let s = mtoon_shade_multiply_texture_sweep();
        for p in &s {
            if p.id == "mtoon_shadetex_baseline" {
                continue;
            }
            assert!(
                p.shade_multiply_texture,
                "{}: non-baseline variant must set shade_multiply_texture",
                p.id
            );
        }
    }
}

#[cfg(test)]
mod texture_transform_sweep_tests {
    use super::*;

    #[test]
    fn texture_transform_sweep_emits_8_variants_with_unique_ids() {
        let s = mtoon_texture_transform_sweep();
        assert_eq!(s.len(), 8);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            s.len(),
            "all texture-transform sweep IDs must be unique"
        );
    }

    #[test]
    fn identity_variant_emits_no_extension_at_emit_time() {
        // Regression guard: the identity TextureTransform should round-
        // trip without producing a KHR_texture_transform extension entry
        // in the textureInfo (base_material's `is_identity()` gate). This
        // is the conformance-test equivalent for "extension absent when
        // it would be a no-op", matching the conditional-emit pattern
        // for the emissive multiplier sweep.
        let s = mtoon_texture_transform_sweep();
        let id_var = s.iter().find(|p| p.id == "mtoon_uvxform_identity").unwrap();
        assert!(id_var.texture_transform.is_some());
        assert!(id_var.texture_transform.unwrap().is_identity());
    }

    #[test]
    fn every_non_identity_variant_has_a_distinct_transform() {
        let s = mtoon_texture_transform_sweep();
        let non_identity: Vec<_> = s
            .iter()
            .filter(|p| !p.texture_transform.unwrap().is_identity())
            .collect();
        // Each non-identity variant must vary at least one axis.
        for p in &non_identity {
            let t = p.texture_transform.unwrap();
            let varies = t.offset != [0.0, 0.0] || t.rotation != 0.0 || t.scale != [1.0, 1.0];
            assert!(varies, "{}: non-identity variant must vary an axis", p.id);
        }
        // And every pair must be distinguishable to keep sweep signal sharp.
        for i in 0..non_identity.len() {
            for j in (i + 1)..non_identity.len() {
                assert_ne!(
                    non_identity[i].texture_transform, non_identity[j].texture_transform,
                    "{} and {} produce identical transforms",
                    non_identity[i].id, non_identity[j].id
                );
            }
        }
    }
}

#[cfg(test)]
mod first_person_sweep_tests {
    use super::*;

    #[test]
    fn first_person_sweep_emits_one_variant_per_spec_enum() {
        let s = mtoon_first_person_sweep();
        assert_eq!(s.len(), 4, "VRMC_vrm.firstPerson has 4 enum values");
        let ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"mtoon_firstperson_auto"));
        assert!(ids.contains(&"mtoon_firstperson_both"));
        assert!(ids.contains(&"mtoon_firstperson_thirdPersonOnly"));
        assert!(ids.contains(&"mtoon_firstperson_firstPersonOnly"));
    }

    #[test]
    fn every_first_person_variant_sets_the_field() {
        let s = mtoon_first_person_sweep();
        for p in &s {
            assert!(
                p.first_person_type.is_some(),
                "{}: first_person_type must be Some so the emitter overrides the default 'auto'",
                p.id
            );
        }
    }
}

#[cfg(test)]
mod emissive_sweep_tests {
    use super::*;

    #[test]
    fn emissive_sweep_emits_expected_variants() {
        let s = mtoon_emissive_sweep();
        // 7 multiplier × white + 3 channel @ x1 + 3 channel @ x2 + 1 zero-baseline = 14
        assert_eq!(
            s.len(),
            14,
            "expected 14 emissive sweep entries, got {}",
            s.len()
        );

        // ID uniqueness — every sweep across the suite assumes unique IDs
        // so the runner's plan→png path doesn't collide.
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len(), "emissive sweep IDs must be unique");

        // Baseline at multiplier=1 must exist (this is the canonical
        // "extension absent, plain emissiveFactor only" case).
        assert!(
            s.iter().any(|p| p.id == "mtoon_emissive_multiplier_1"),
            "missing canonical multiplier=1 variant"
        );

        // Zero-emissive baseline must keep emissive_factor at zero so the
        // material round-trip produces no extension regardless of the
        // (intentionally != 1.0) multiplier value.
        let zero = s
            .iter()
            .find(|p| p.id == "mtoon_emissive_zero_factor")
            .expect("missing zero-factor baseline");
        assert_eq!(zero.emissive_factor, [0.0, 0.0, 0.0]);
        assert_ne!(
            zero.emissive_multiplier, 1.0,
            "zero-factor baseline must set multiplier != 1.0 to exercise the conditional-emit guard"
        );
    }

    #[test]
    fn emissive_sweep_per_channel_variants_are_pure_red_green_blue() {
        let s = mtoon_emissive_sweep();
        for axis in ["r", "g", "b"] {
            for mult_suffix in ["x1", "x2"] {
                let id = format!("mtoon_emissive_{axis}_{mult_suffix}");
                let p = s.iter().find(|p| p.id == id).unwrap_or_else(|| {
                    panic!("missing channel variant {id}");
                });
                let nonzero_count = p.emissive_factor.iter().filter(|&&c| c != 0.0).count();
                assert_eq!(
                    nonzero_count, 1,
                    "{id} must have exactly one non-zero channel"
                );
            }
        }
    }
}

#[cfg(test)]
mod outline_width_multiply_texture_sweep_tests {
    use super::*;

    #[test]
    fn outline_width_sweep_has_5_variants_with_unique_ids() {
        let s = mtoon_outline_width_multiply_texture_sweep();
        assert_eq!(s.len(), 5);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len());
    }

    #[test]
    fn baseline_omits_texture_and_uses_world_outline() {
        let s = mtoon_outline_width_multiply_texture_sweep();
        let b = s
            .iter()
            .find(|p| p.id == "mtoon_outlinewidthtex_baseline")
            .unwrap();
        assert!(!b.outline_width_multiply_texture);
        assert!(matches!(
            b.outline_width_mode,
            OutlineWidthMode::WorldCoordinates
        ));
    }

    #[test]
    fn mode_none_variant_is_regression_guard() {
        let s = mtoon_outline_width_multiply_texture_sweep();
        let n = s
            .iter()
            .find(|p| p.id == "mtoon_outlinewidthtex_mode_none")
            .unwrap();
        assert!(n.outline_width_multiply_texture);
        assert!(matches!(n.outline_width_mode, OutlineWidthMode::None));
    }
}

#[cfg(test)]
mod pbr_textures_sweep_tests {
    use super::*;

    #[test]
    fn pbr_textures_sweep_has_6_variants_with_unique_ids() {
        let s = mtoon_pbr_textures_sweep();
        assert_eq!(s.len(), 6);
        let mut ids: Vec<&str> = s.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.len());
    }

    #[test]
    fn baseline_omits_both_pbr_textures() {
        let s = mtoon_pbr_textures_sweep();
        let b = s.iter().find(|p| p.id == "mtoon_pbrtex_baseline").unwrap();
        assert!(b.normal_texture_scale.is_none());
        assert!(b.occlusion_texture_strength.is_none());
    }

    #[test]
    fn combined_variant_sets_both() {
        let s = mtoon_pbr_textures_sweep();
        let c = s.iter().find(|p| p.id == "mtoon_pbrtex_combined").unwrap();
        assert!(c.normal_texture_scale.is_some());
        assert!(c.occlusion_texture_strength.is_some());
    }
}

#[cfg(test)]
mod sweep_v0_tests {
    use super::*;
    use crate::{NotApplicableReason, SweepApplicability};

    #[test]
    fn mtoon_basic_v0_sweep_has_three_variants() {
        let sweep = mtoon_basic_v0_sweep();
        assert_eq!(sweep.len(), 3);
    }

    #[test]
    fn mtoon_basic_v0_sweep_has_one_not_applicable() {
        let sweep = mtoon_basic_v0_sweep();
        let na_count = sweep
            .iter()
            .filter(|(_, app)| matches!(app, SweepApplicability::NotApplicable { .. }))
            .count();
        assert_eq!(na_count, 1);
    }

    #[test]
    fn not_applicable_reason_is_outline_lighting_mix() {
        let sweep = mtoon_basic_v0_sweep();
        let na = sweep
            .iter()
            .find_map(|(_, app)| match app {
                SweepApplicability::NotApplicable { reason } => Some(*reason),
                _ => None,
            })
            .unwrap();
        assert_eq!(na, NotApplicableReason::OutlineLightingMixV1Only);
    }

    #[test]
    fn mtoon_basic_v0_sweep_variant_ids_match_plan() {
        let sweep = mtoon_basic_v0_sweep();
        let ids: Vec<&str> = sweep.iter().map(|(p, _)| p.id.as_str()).collect();
        assert!(
            ids.contains(&"mtoon_basic_v0_lit_001"),
            "missing mtoon_basic_v0_lit_001"
        );
        assert!(
            ids.contains(&"mtoon_basic_v0_shadeShift_neg05"),
            "missing mtoon_basic_v0_shadeShift_neg05"
        );
        assert!(
            ids.contains(&"mtoon_basic_v0_outline_lighting_mix"),
            "missing mtoon_basic_v0_outline_lighting_mix"
        );
    }

    #[test]
    fn mtoon_basic_v0_sweep_applicable_variants_have_correct_params() {
        let sweep = mtoon_basic_v0_sweep();
        let shade_shift = sweep
            .iter()
            .find(|(p, _)| p.id == "mtoon_basic_v0_shadeShift_neg05")
            .unwrap();
        assert!(
            (shade_shift.0.shading_shift_factor - (-0.5)).abs() < 1e-6,
            "shadeShift variant must set shading_shift_factor = -0.5"
        );
    }
}

// ── expressions_preset_basic sweeps (Task 17) ────────────────────────────────

/// VRM 0.x expressions preset basic — canonical normalization test pair (v0 side).
///
/// 2 variants:
/// - `expressions_preset_basic_v0_joy` — `blendShapeMaster` group with
///   preset=joy, weight=100 (Unity convention) on mesh 0 / morph 0.
/// - `expressions_preset_basic_v0_neutral` — preset=neutral, same binding.
///
/// These are the assets that validate `vrm-normalize::expressions` mapping
/// (joy → happy, neutral → neutral) in Task 32 and drive the cross-renderer
/// round-trip property test in Task 36.
pub fn expressions_preset_basic_v0_sweep(
) -> Vec<(String, crate::expressions_v0::ExpressionsV0Params)> {
    use crate::expressions_v0::{
        BlendShapeBind, BlendShapeGroup, BlendShapePreset, ExpressionsV0Params,
    };

    vec![
        (
            "expressions_preset_basic_v0_joy".to_string(),
            ExpressionsV0Params {
                groups: vec![BlendShapeGroup {
                    name: "Joy".into(),
                    preset: BlendShapePreset::Joy,
                    binds: vec![BlendShapeBind {
                        mesh_index: 0,
                        morph_target_index: 0,
                        weight_0_to_100: 100.0,
                    }],
                }],
            },
        ),
        (
            "expressions_preset_basic_v0_neutral".to_string(),
            ExpressionsV0Params {
                groups: vec![BlendShapeGroup {
                    name: "Neutral".into(),
                    preset: BlendShapePreset::Neutral,
                    binds: vec![BlendShapeBind {
                        mesh_index: 0,
                        morph_target_index: 0,
                        weight_0_to_100: 100.0,
                    }],
                }],
            },
        ),
    ]
}

/// VRM 1.0 expressions preset basic — canonical normalization test pair (v1 side).
///
/// 2 variants:
/// - `expressions_preset_basic_happy` — `VRMC_vrm.expressions.preset.happy`
///   bound to mesh node / morph 0, weight=1.0 (glTF convention).
/// - `expressions_preset_basic_neutral` — preset=neutral, same binding.
///
/// Pairs with [`expressions_preset_basic_v0_sweep`] for the canonical
/// normalization round-trip test.
pub fn expressions_preset_basic_v1_sweep() -> Vec<(String, crate::vrm_ext::ExpressionsV1Params)> {
    use crate::vrm_ext::{ExpressionsV1Params, V1ExpressionPreset, V1MorphTargetBind};

    // Note: the node index for the morph-target bind is set to 0 here as a
    // placeholder. The actual mesh node index is determined at emit time
    // (it depends on the skeleton node count). The dispatch in cli.rs passes
    // the correct node index when calling emit_with_sidecars_v1_with_expressions.
    // For the sweep definition itself we store 0 and the emitter will override.
    vec![
        (
            "expressions_preset_basic_happy".to_string(),
            ExpressionsV1Params {
                preset_binds: vec![(
                    V1ExpressionPreset::Happy,
                    V1MorphTargetBind {
                        node: 0,
                        morph_target_index: 0,
                        weight_0_to_1: 1.0,
                    },
                )],
            },
        ),
        (
            "expressions_preset_basic_neutral".to_string(),
            ExpressionsV1Params {
                preset_binds: vec![(
                    V1ExpressionPreset::Neutral,
                    V1MorphTargetBind {
                        node: 0,
                        morph_target_index: 0,
                        weight_0_to_1: 1.0,
                    },
                )],
            },
        ),
    ]
}

#[cfg(test)]
mod expressions_basic_tests {
    use super::*;

    #[test]
    fn v0_sweep_has_two_variants_joy_and_neutral() {
        let sweep = expressions_preset_basic_v0_sweep();
        assert_eq!(sweep.len(), 2);
        let names: Vec<_> = sweep.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("joy")));
        assert!(names.iter().any(|n| n.contains("neutral")));
    }

    #[test]
    fn v1_sweep_has_two_variants_happy_and_neutral() {
        let sweep = expressions_preset_basic_v1_sweep();
        assert_eq!(sweep.len(), 2);
        let names: Vec<_> = sweep.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("happy")));
        assert!(names.iter().any(|n| n.contains("neutral")));
    }

    #[test]
    fn v0_weight_is_unity_convention_100() {
        let sweep = expressions_preset_basic_v0_sweep();
        let joy = sweep.iter().find(|(name, _)| name.contains("joy")).unwrap();
        assert_eq!(joy.1.groups[0].binds[0].weight_0_to_100, 100.0);
    }

    #[test]
    fn v1_weight_is_gltf_convention_1() {
        let sweep = expressions_preset_basic_v1_sweep();
        let happy = sweep
            .iter()
            .find(|(name, _)| name.contains("happy"))
            .unwrap();
        assert_eq!(happy.1.preset_binds[0].1.weight_0_to_1, 1.0);
    }

    #[test]
    fn v0_sweep_unique_ids() {
        let sweep = expressions_preset_basic_v0_sweep();
        let ids: std::collections::HashSet<_> = sweep.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn v1_sweep_unique_ids() {
        let sweep = expressions_preset_basic_v1_sweep();
        let ids: std::collections::HashSet<_> = sweep.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(ids.len(), 2);
    }
}

#[cfg(test)]
mod registry_symmetry_tests {
    use super::*;
    use crate::SweepApplicability;

    /// Slice 1 success criterion #6: every sweep ID ending in `_v0` (or
    /// containing `_v0_`) must have a 1.0 counterpart registered.
    /// Counterpart = same ID with `_v0` removed (or `_v0_` replaced with `_`).
    /// Either the counterpart exists as Applicable, OR the v0 entry is
    /// registered as NotApplicable.
    #[test]
    fn mtoon_basic_v0_applicable_entries_have_v1_counterparts() {
        let v1_ids: std::collections::HashSet<String> = mtoon_basic_sweep()
            .into_iter()
            .map(|p| p.id.clone())
            .collect();

        let v0_entries = mtoon_basic_v0_sweep();
        let mut missing_counterparts = Vec::new();

        for (params, applicability) in v0_entries {
            let v0_id = &params.id;
            // Map v0 ID → v1 counterpart by stripping the `_v0` infix/suffix.
            let counterpart_id = v0_id
                .replace("_basic_v0_", "_basic_")
                .replace("_v0_", "_")
                .trim_end_matches("_v0")
                .to_string();

            if matches!(applicability, SweepApplicability::Applicable)
                && !v1_ids.contains(&counterpart_id)
            {
                missing_counterparts.push(format!(
                    "v0 sweep entry {v0_id} (Applicable) is missing 1.0 counterpart {counterpart_id}"
                ));
            }
            // NotApplicable entries don't require counterparts.
        }

        assert!(
            missing_counterparts.is_empty(),
            "sweep registry symmetry violations:\n{}",
            missing_counterparts.join("\n")
        );
    }

    /// Same property for the v0+v1 expressions pair (Task 17).
    /// Pairing is by ordering, not by ID-stripping, since the v0/v1 names
    /// use different preset tokens (joy/happy, neutral/neutral).
    #[test]
    fn expressions_preset_basic_v0_and_v1_have_matching_variant_count() {
        let v0 = expressions_preset_basic_v0_sweep();
        let v1 = expressions_preset_basic_v1_sweep();
        assert_eq!(
            v0.len(),
            v1.len(),
            "expressions sweep variant count mismatch: v0={}, v1={}",
            v0.len(),
            v1.len()
        );
    }
}

#[cfg(test)]
mod material_name_classification_tests {
    use super::*;

    // VMK (and similar renderers) classify render behavior by substrings of
    // the *material name* — e.g. a name containing "cloth" is forced
    // double-sided + given an overlay depth bias (VRMRenderItemBuilder.swift:216).
    // glTF/VRM make `material.doubleSided` the sole authority. This sweep emits
    // ONE MToon material under heuristic-tripping vs control names crossed with
    // the glTF doubleSided flag; conformant output is invariant to the name.
    // See docs/superpowers/specs/2026-05-28-material-name-classification-reproducer.md.

    #[test]
    fn sweep_has_six_variants() {
        assert_eq!(material_name_classification_sweep().len(), 6);
    }

    #[test]
    fn variant_ids_are_the_designed_set() {
        let ids: Vec<String> = material_name_classification_sweep()
            .iter()
            .map(|p| p.id.clone())
            .collect();
        assert_eq!(
            ids,
            vec![
                "matname_plain_singlesided",
                "matname_clothing_singlesided",
                "matname_skirt_singlesided",
                "matname_plain_doublesided",
                "matname_clothing_doublesided",
                "matname_body_singlesided",
            ]
        );
    }

    #[test]
    fn trip_token_variants_contain_heuristic_substrings() {
        let by_id = |id: &str| -> MToonParams {
            material_name_classification_sweep()
                .into_iter()
                .find(|p| p.id == id)
                .unwrap_or_else(|| panic!("missing variant {id}"))
        };
        // The material name IS params.id (mtoon_v0.rs:26, vrm_ext.rs:324), so
        // these names are what a name-heuristic renderer matches on.
        assert!(by_id("matname_clothing_singlesided").id.contains("cloth"));
        assert!(by_id("matname_skirt_singlesided").id.contains("skirt"));
        assert!(!by_id("matname_plain_singlesided").id.contains("cloth"));
        assert!(!by_id("matname_plain_singlesided").id.contains("skirt"));
    }

    #[test]
    fn double_sided_flag_matches_name_suffix() {
        for p in material_name_classification_sweep() {
            if p.id.ends_with("_doublesided") {
                assert!(p.double_sided, "{} must set double_sided=true", p.id);
            } else {
                assert!(!p.double_sided, "{} must set double_sided=false", p.id);
            }
        }
    }

    #[test]
    fn variants_differ_only_by_id_and_double_sided() {
        // The whole point: within a doubleSided value, the materials are
        // byte-identical except the name. A conformant renderer must produce
        // identical pixels; a name-heuristic renderer diverges on the trip
        // tokens. We assert the params themselves are equal modulo id +
        // double_sided by normalizing both fields and comparing serialized form.
        let sweep = material_name_classification_sweep();
        let normalize = |p: &MToonParams| -> String {
            let mut q = p.clone();
            q.id = "NORM".to_string();
            q.double_sided = false;
            serde_json::to_string(&q).unwrap()
        };
        let baseline = normalize(&sweep[0]);
        for p in &sweep {
            assert_eq!(
                normalize(p),
                baseline,
                "variant {} differs from baseline in a field other than id/double_sided",
                p.id
            );
        }
    }
}

#[cfg(test)]
mod gaze_sweep_tests {
    use super::*;
    use crate::vrma_params::RotationAxis;

    #[test]
    fn gaze_sweep_covers_both_bugs() {
        let sweep = gaze_sweep();
        let ids: Vec<&str> = sweep.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(sweep.len(), 8);
        // Bug B: gaze directions at neutral body.
        assert!(ids.contains(&"gaze_center"));
        assert!(ids.contains(&"gaze_left"));
        assert!(ids.contains(&"gaze_right"));
        assert!(ids.contains(&"gaze_up"));
        assert!(ids.contains(&"gaze_down"));
        // Bug A: turned-head variants.
        assert!(ids.contains(&"gaze_center_bodyL"));
        assert!(ids.contains(&"gaze_center_bodyR"));
        assert!(ids.contains(&"gaze_right_bodyL"));
        // center has zero gaze; bodyL turns +35.
        let center = sweep.iter().find(|p| p.id == "gaze_center").unwrap();
        assert_eq!(center.gaze_angle_deg, 0.0);
        assert_eq!(center.body_yaw_deg, 0.0);
        let body_l = sweep.iter().find(|p| p.id == "gaze_center_bodyL").unwrap();
        assert_eq!(body_l.body_yaw_deg, 35.0);
        // Axis assertions: up/down use X (pitch); left/right use Y (yaw).
        let gaze_up = sweep.iter().find(|p| p.id == "gaze_up").unwrap();
        assert!(matches!(gaze_up.gaze_axis, RotationAxis::X));
        assert_eq!(gaze_up.gaze_angle_deg, 20.0);
        let gaze_down = sweep.iter().find(|p| p.id == "gaze_down").unwrap();
        assert!(matches!(gaze_down.gaze_axis, RotationAxis::X));
        assert_eq!(gaze_down.gaze_angle_deg, -20.0);
        let gaze_left = sweep.iter().find(|p| p.id == "gaze_left").unwrap();
        assert!(matches!(gaze_left.gaze_axis, RotationAxis::Y));
        assert_eq!(gaze_left.gaze_angle_deg, 30.0);
        let gaze_right_body_l = sweep.iter().find(|p| p.id == "gaze_right_bodyL").unwrap();
        assert!(matches!(gaze_right_body_l.gaze_axis, RotationAxis::Y));
        assert_eq!(gaze_right_body_l.gaze_angle_deg, -30.0);
        assert_eq!(gaze_right_body_l.body_yaw_deg, 35.0);
        // All ids must be unique.
        let unique_ids: std::collections::HashSet<_> = sweep.iter().map(|p| p.id.clone()).collect();
        assert_eq!(unique_ids.len(), 8);
    }
}

#[cfg(test)]
mod ccd_sweep_tests {
    use super::*;

    fn collider_radius(shape: &ColliderShape) -> f32 {
        match shape {
            ColliderShape::Sphere { radius } => *radius,
            ColliderShape::Capsule { radius, .. } => *radius,
            ColliderShape::InsideSphere { radius } => *radius,
            ColliderShape::InsideCapsule { radius, .. } => *radius,
            ColliderShape::Plane { .. } => 0.0,
        }
    }

    #[test]
    fn ccd_sweep_uses_world_collider_and_straddles_threshold() {
        let v = spring_bone_ccd_sweep();
        assert!(!v.is_empty());
        // every cell carries exactly one world-fixed collider
        for (mtoon, scene) in &v {
            assert_eq!(scene.colliders.len(), 1, "{}: one collider", mtoon.id);
            assert!(
                matches!(
                    scene.colliders[0].attach,
                    ColliderAttach::WorldCoordinates { .. }
                ),
                "{}: world-fixed collider",
                mtoon.id
            );
        }
        // threshold straddle: at least one thin-radius cell and one thick-radius cell
        let radii: Vec<f32> = v
            .iter()
            .map(|(_, s)| collider_radius(&s.colliders[0].shape))
            .collect();
        assert!(radii.iter().any(|&r| r <= 0.01), "a thin collider exists");
        assert!(radii.iter().any(|&r| r >= 0.04), "a thick collider exists");
        // unique ids, all ccd_ prefixed
        let ids: std::collections::HashSet<_> = v.iter().map(|(m, _)| m.id.clone()).collect();
        assert_eq!(ids.len(), v.len(), "unique ids");
        assert!(v.iter().all(|(m, _)| m.id.starts_with("ccd_")));
    }

    #[test]
    fn ccd_sweep_has_12_cells() {
        assert_eq!(spring_bone_ccd_sweep().len(), 12);
    }

    #[test]
    fn ccd_sweep_both_speed_tags_present() {
        let v = spring_bone_ccd_sweep();
        assert!(v.iter().any(|(m, _)| m.id.ends_with("_fast")));
        assert!(v.iter().any(|(m, _)| m.id.ends_with("_slow")));
    }

    #[test]
    fn ccd_sweep_both_shape_kinds_present() {
        let v = spring_bone_ccd_sweep();
        assert!(v.iter().any(|(m, _)| m.id.contains("_sphere_")));
        assert!(v.iter().any(|(m, _)| m.id.contains("_capsule_")));
    }

    #[test]
    fn ccd_collider_world_spec_matches_scene() {
        let v = spring_bone_ccd_sweep();
        for (mtoon, scene) in &v {
            let spec = ccd_collider_world_spec(scene);
            assert!(
                spec.radius > 0.0,
                "{}: collider world spec radius must be positive",
                mtoon.id
            );
            // World center must be deterministic and nonzero (not the chain's rest column)
            assert!(
                spec.center[0].abs() > 0.05,
                "{}: world center must be laterally offset from chain rest column (x=0)",
                mtoon.id
            );
        }
    }

    /// Fix 1: capsule CCD cells must report head/tail endpoints that match the
    /// VRM file's actual world geometry.
    ///
    /// VRM capsule geometry (node_translation = [0.10, 1.26, 0.0], offset = [0,0,0],
    /// tail_offset = [0.0, -0.10, 0.0]):
    ///   head = node_translation + offset = [0.10, 1.26, 0.0]
    ///   tail = node_translation + tail_offset = [0.10, 1.16, 0.0]
    #[test]
    fn ccd_collider_world_spec_capsule_head_tail_match_vrm_endpoints() {
        let v = spring_bone_ccd_sweep();
        // Use the first capsule cell (any radius, any speed).
        let (mtoon, scene) = v
            .iter()
            .find(|(m, _)| m.id.contains("capsule"))
            .expect("at least one capsule cell");

        // Verify the scene has the expected geometry.
        let c = &scene.colliders[0];
        let center = match &c.attach {
            ColliderAttach::WorldCoordinates { center } => *center,
            _ => panic!("expected WorldCoordinates"),
        };
        let tail_offset = match &c.shape {
            ColliderShape::Capsule { tail_offset, .. } => *tail_offset,
            _ => panic!("expected Capsule"),
        };

        // Expected world endpoints from the VRM spec convention:
        //   head = node_translation + offset (offset=[0,0,0])
        //   tail = node_translation + tail_offset
        let expected_head = [
            center[0] + c.offset[0],
            center[1] + c.offset[1],
            center[2] + c.offset[2],
        ];
        let expected_tail = [
            center[0] + tail_offset[0],
            center[1] + tail_offset[1],
            center[2] + tail_offset[2],
        ];

        // Pinned values for the CCD sweep geometry:
        //   node_center = [0.10, 1.26, 0.0], offset=[0,0,0], tail=[0,-0.10,0]
        //   → head = [0.10, 1.26, 0.0],  tail = [0.10, 1.16, 0.0]
        assert!(
            (expected_head[0] - 0.10_f32).abs() < 1e-5,
            "{}: head.x should be 0.10, got {}",
            mtoon.id,
            expected_head[0]
        );
        assert!(
            (expected_head[1] - 1.26_f32).abs() < 1e-5,
            "{}: head.y should be 1.26, got {}",
            mtoon.id,
            expected_head[1]
        );
        assert!(
            (expected_tail[1] - 1.16_f32).abs() < 1e-5,
            "{}: tail.y should be 1.16, got {}",
            mtoon.id,
            expected_tail[1]
        );

        let spec = ccd_collider_world_spec(scene);
        assert_eq!(spec.kind, "capsule");

        let head = spec
            .capsule_head
            .expect("capsule spec must have capsule_head");
        let tail = spec
            .capsule_tail
            .expect("capsule spec must have capsule_tail");

        // spec.capsule_head must equal expected_head (VRM file's head endpoint)
        assert!(
            (head[0] - expected_head[0]).abs() < 1e-5
                && (head[1] - expected_head[1]).abs() < 1e-5
                && (head[2] - expected_head[2]).abs() < 1e-5,
            "{}: capsule_head {:?} must match VRM head endpoint {:?}",
            mtoon.id,
            head,
            expected_head,
        );
        // spec.capsule_tail must equal expected_tail (VRM file's tail endpoint)
        assert!(
            (tail[0] - expected_tail[0]).abs() < 1e-5
                && (tail[1] - expected_tail[1]).abs() < 1e-5
                && (tail[2] - expected_tail[2]).abs() < 1e-5,
            "{}: capsule_tail {:?} must match VRM tail endpoint {:?}",
            mtoon.id,
            tail,
            expected_tail,
        );
    }
}

#[cfg(test)]
mod expression_clip_sweep_tests {
    use super::*;

    #[test]
    fn expression_clip_sweep_has_11_presets_with_expr_ids() {
        let sweep = expression_clip_sweep();
        assert_eq!(sweep.len(), 11);
        let ids: Vec<&str> = sweep.iter().map(|p| p.id.as_str()).collect();
        for name in [
            "blink",
            "happy",
            "angry",
            "sad",
            "relaxed",
            "surprised",
            "aa",
            "ih",
            "ou",
            "ee",
            "oh",
        ] {
            assert!(
                ids.contains(&format!("expr_{name}").as_str()),
                "missing expr_{name}"
            );
        }
        for p in &sweep {
            assert!(p.is_preset, "{} should be preset", p.id);
            assert_eq!(p.duration_s, 1.0);
            assert_eq!(p.id, format!("expr_{}", p.expression_name));
        }
    }
}

#[cfg(test)]
mod vrma_arkit_expression_sweep_tests {
    use super::*;

    #[test]
    fn exactly_52_names_lexicographic_and_unique() {
        let names = arkit_blendshape_names();
        assert_eq!(names.len(), 52);
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            names.to_vec(),
            sorted,
            "must stay in canonical lexicographic order"
        );
        sorted.dedup();
        assert_eq!(sorted.len(), 52, "duplicate blendshape names");
    }

    #[test]
    fn sweep_is_all_custom_with_arkit_ids() {
        let variants = vrma_arkit_expression_sweep();
        assert_eq!(variants.len(), 52);
        for v in &variants {
            assert!(!v.is_preset, "{}: ARKit tracks are custom-classified", v.id);
            assert_eq!(v.id, format!("vrma_arkit_{}", v.expression_name));
            assert!((v.duration_s - 1.0).abs() < 1e-6);
        }
        assert!(variants.iter().any(|v| v.expression_name == "jawOpen"));
        assert!(variants.iter().any(|v| v.expression_name == "tongueOut"));
    }
}

/// Finger-bone sweep: one variant per VRM 1.0 optional finger bone
/// (30 = 15 per hand). Fingers curl about Z (±45°, sign mirrored per
/// side); thumbs rotate about Y (±30°, mirrored). Sign choices are for
/// visual plausibility only — the conformance signal is the dumped
/// quaternion matching across renderers, not anatomy. Reuses
/// `VrmaHumanoidParams`; emission pairs with `skeleton_with_fingers()`.
pub fn vrma_finger_sweep() -> Vec<crate::vrma_params::VrmaHumanoidParams> {
    use crate::vrma_params::{RotationAxis, VrmaHumanoidParams};
    const SEGMENTS: [(&str, &str); 15] = [
        ("ThumbMetacarpal", "thumb_metacarpal"),
        ("ThumbProximal", "thumb_proximal"),
        ("ThumbDistal", "thumb_distal"),
        ("IndexProximal", "index_proximal"),
        ("IndexIntermediate", "index_intermediate"),
        ("IndexDistal", "index_distal"),
        ("MiddleProximal", "middle_proximal"),
        ("MiddleIntermediate", "middle_intermediate"),
        ("MiddleDistal", "middle_distal"),
        ("RingProximal", "ring_proximal"),
        ("RingIntermediate", "ring_intermediate"),
        ("RingDistal", "ring_distal"),
        ("LittleProximal", "little_proximal"),
        ("LittleIntermediate", "little_intermediate"),
        ("LittleDistal", "little_distal"),
    ];
    let mut out = Vec::with_capacity(30);
    for (side, side_label, sign) in [("left", "l", 1.0_f32), ("right", "r", -1.0_f32)] {
        for (seg, seg_label) in SEGMENTS {
            let is_thumb = seg.starts_with("Thumb");
            let (axis, angle_deg) = if is_thumb {
                (RotationAxis::Y, 30.0 * sign)
            } else {
                (RotationAxis::Z, -45.0 * sign)
            };
            out.push(VrmaHumanoidParams {
                id: format!("vrma_finger_{side_label}_{seg_label}"),
                bone_name: format!("{side}{seg}"),
                axis,
                angle_deg,
                duration_s: 1.0,
            });
        }
    }
    out
}

#[cfg(test)]
mod vrma_finger_sweep_tests {
    use super::*;

    #[test]
    fn thirty_variants_one_per_finger_bone() {
        let variants = vrma_finger_sweep();
        assert_eq!(variants.len(), 30);
        let mut bones: Vec<&str> = variants.iter().map(|v| v.bone_name.as_str()).collect();
        bones.sort_unstable();
        bones.dedup();
        assert_eq!(bones.len(), 30, "each finger bone exactly once");
        assert!(variants.iter().any(|v| v.bone_name == "leftIndexProximal"));
        assert!(variants.iter().any(|v| v.bone_name == "rightThumbDistal"));
    }

    #[test]
    fn finger_bones_resolve_in_finger_skeleton() {
        let sk = crate::humanoid::skeleton_with_fingers();
        for v in vrma_finger_sweep() {
            assert!(
                sk.bone_to_node.contains_key(&v.bone_name),
                "{} not in finger skeleton",
                v.bone_name
            );
        }
    }
}

#[cfg(test)]
mod vrma_multichannel_sweep_tests {
    use super::*;

    #[test]
    fn six_variants_each_combining_at_least_two_channels() {
        let variants = vrma_multichannel_sweep();
        assert_eq!(variants.len(), 6);
        let mut ids: Vec<&str> = variants.iter().map(|v| v.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 6, "duplicate ids");
        for v in &variants {
            let channel_kinds = [
                !v.bones.is_empty(),
                v.hips_offset_m.is_some(),
                !v.expressions.is_empty(),
                v.look_at.is_some(),
            ]
            .iter()
            .filter(|b| **b)
            .count();
            assert!(channel_kinds >= 2, "{}: not multi-channel", v.id);
        }
    }

    #[test]
    fn double_drive_variant_pairs_preset_with_custom() {
        let variants = vrma_multichannel_sweep();
        let dd = variants
            .iter()
            .find(|v| v.id == "vrma_multi_double_drive")
            .expect("double-drive variant");
        assert!(dd.expressions.iter().any(|e| e.is_preset));
        assert!(dd.expressions.iter().any(|e| !e.is_preset));
    }
}
