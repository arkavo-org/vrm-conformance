//! Shared MToon math, independent of 0.x vs 1.0 JSON shape.
//!
//! MToon 0.x (`materialProperties`) and 1.0 (`VRMC_materials_mtoon`) share
//! the same underlying shading math — only the JSON key names and layout
//! differ. This module holds the math; `vrm_ext.rs` (1.0) and
//! `vrm_ext_v0.rs` (0.x) each emit their respective JSON shape over this
//! shared math.
//!
//! All helpers are pure functions taking `&MToonParams` (or individual
//! fields) and returning clamped / validated values ready to be written
//! into the JSON output. No JSON construction happens here.

use crate::params::MToonParams;

/// Clamped shading-shift factor, per VRMC_materials_mtoon spec §3.
///
/// `shadingShiftFactor` ∈ [-1, 1]. Values outside this range are defined
/// as out-of-spec. The clamp is applied here so both 0.x and 1.0 emitters
/// always produce valid output regardless of what the `MToonParams` struct
/// carries.
pub fn shading_shift_factor(p: &MToonParams) -> f32 {
    p.shading_shift_factor.clamp(-1.0, 1.0)
}

/// Clamped shading-toony factor, per VRMC_materials_mtoon spec §3.
///
/// `shadingToonyFactor` ∈ [0, 1].
pub fn shading_toony_factor(p: &MToonParams) -> f32 {
    p.shading_toony_factor.clamp(0.0, 1.0)
}

/// Clamped GI equalization factor, per VRMC_materials_mtoon spec §4.
///
/// `giEqualizationFactor` ∈ [0, 1].
pub fn gi_equalization_factor(p: &MToonParams) -> f32 {
    p.gi_equalization_factor.clamp(0.0, 1.0)
}

/// Parametric rim color factor as a 3-element linear-RGB array.
///
/// Components are not range-clamped by the spec (HDR rim is allowed), so
/// this is a plain pass-through that gives Task 13 a named extraction point
/// to override if 0.x needs different behavior.
pub fn parametric_rim_color_factor(p: &MToonParams) -> [f32; 3] {
    p.parametric_rim_color_factor
}

/// Clamped rim lighting mix factor, per VRMC_materials_mtoon spec §5.
///
/// `rimLightingMixFactor` ∈ [0, 1].
pub fn rim_lighting_mix_factor(p: &MToonParams) -> f32 {
    p.rim_lighting_mix_factor.clamp(0.0, 1.0)
}

/// Clamped outline lighting mix factor, per VRMC_materials_mtoon spec §6.
///
/// `outlineLightingMixFactor` ∈ [0, 1]. Clamped here; 0.x exposes it as
/// `_OutlineLightingMix` with the same valid range.
pub fn outline_lighting_mix_factor(p: &MToonParams) -> f32 {
    p.outline_lighting_mix_factor.clamp(0.0, 1.0)
}

/// Clamped outline width factor, per VRMC_materials_mtoon spec §6.
///
/// `outlineWidthFactor` ∈ [0, 1]. Negative values are not meaningful.
pub fn outline_width_factor(p: &MToonParams) -> f32 {
    p.outline_width_factor.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::MToonParams;

    fn defaults() -> MToonParams {
        MToonParams::defaults("test")
    }

    // --- shading_shift_factor ---

    #[test]
    fn shading_shift_factor_clamps_above_max() {
        let mut p = defaults();
        p.shading_shift_factor = 2.0;
        assert_eq!(shading_shift_factor(&p), 1.0);
    }

    #[test]
    fn shading_shift_factor_clamps_below_min() {
        let mut p = defaults();
        p.shading_shift_factor = -3.0;
        assert_eq!(shading_shift_factor(&p), -1.0);
    }

    #[test]
    fn shading_shift_factor_passes_through_in_range() {
        let mut p = defaults();
        p.shading_shift_factor = -0.5;
        assert!((shading_shift_factor(&p) - (-0.5_f32)).abs() < f32::EPSILON);
    }

    // --- shading_toony_factor ---

    #[test]
    fn shading_toony_factor_clamps_above_one() {
        let mut p = defaults();
        p.shading_toony_factor = 1.5;
        assert_eq!(shading_toony_factor(&p), 1.0);
    }

    #[test]
    fn shading_toony_factor_clamps_below_zero() {
        let mut p = defaults();
        p.shading_toony_factor = -0.1;
        assert_eq!(shading_toony_factor(&p), 0.0);
    }

    #[test]
    fn shading_toony_factor_default_passes_through() {
        let p = defaults();
        // default is 0.9
        assert!((shading_toony_factor(&p) - 0.9_f32).abs() < f32::EPSILON);
    }

    // --- gi_equalization_factor ---

    #[test]
    fn gi_equalization_factor_clamps_above_one() {
        let mut p = defaults();
        p.gi_equalization_factor = 2.0;
        assert_eq!(gi_equalization_factor(&p), 1.0);
    }

    #[test]
    fn gi_equalization_factor_clamps_below_zero() {
        let mut p = defaults();
        p.gi_equalization_factor = -0.5;
        assert_eq!(gi_equalization_factor(&p), 0.0);
    }

    // --- rim_lighting_mix_factor ---

    #[test]
    fn rim_lighting_mix_factor_clamps_above_one() {
        let mut p = defaults();
        p.rim_lighting_mix_factor = 1.5;
        assert_eq!(rim_lighting_mix_factor(&p), 1.0);
    }

    #[test]
    fn rim_lighting_mix_factor_clamps_below_zero() {
        let mut p = defaults();
        p.rim_lighting_mix_factor = -0.1;
        assert_eq!(rim_lighting_mix_factor(&p), 0.0);
    }

    // --- outline_lighting_mix_factor ---

    #[test]
    fn outline_lighting_mix_factor_boundary_at_zero() {
        let mut p = defaults();
        p.outline_lighting_mix_factor = -0.01;
        assert_eq!(outline_lighting_mix_factor(&p), 0.0);
    }

    #[test]
    fn outline_lighting_mix_factor_boundary_at_one() {
        let mut p = defaults();
        p.outline_lighting_mix_factor = 1.1;
        assert_eq!(outline_lighting_mix_factor(&p), 1.0);
    }

    // --- outline_width_factor ---

    #[test]
    fn outline_width_factor_clamps_negative() {
        let mut p = defaults();
        p.outline_width_factor = -0.05;
        assert_eq!(outline_width_factor(&p), 0.0);
    }

    #[test]
    fn outline_width_factor_clamps_above_one() {
        let mut p = defaults();
        p.outline_width_factor = 1.1;
        assert_eq!(outline_width_factor(&p), 1.0);
    }

    // --- parametric_rim_color_factor (pass-through, no clamp) ---

    #[test]
    fn parametric_rim_color_factor_passes_through() {
        let mut p = defaults();
        p.parametric_rim_color_factor = [0.1, 0.2, 0.3];
        let result = parametric_rim_color_factor(&p);
        assert_eq!(result, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn parametric_rim_color_factor_hdr_passes_through() {
        // HDR rim is allowed — no clamp applied
        let mut p = defaults();
        p.parametric_rim_color_factor = [2.0, 1.5, 0.8];
        let result = parametric_rim_color_factor(&p);
        assert_eq!(result, [2.0, 1.5, 0.8]);
    }
}
