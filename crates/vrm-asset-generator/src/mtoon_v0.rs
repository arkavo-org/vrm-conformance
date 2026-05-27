//! VRM 0.x MToon emit. Produces `materialProperties[]` entries in the
//! Unity-shader-style key-value shape. Shared math lives in mtoon_common.rs.
//!
//! Schema reference: `docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.material.schema.json`.
//!
//! 0.x has NO `_OutlineLightingMix` key — that axis is v1-only and surfaces
//! as `NotApplicable { reason: OutlineLightingMixV1Only }` in the sweep registry.

use crate::params::{MToonParams, OutlineWidthMode};
use serde_json::{json, Value};

/// Emit one entry of the 0.x `materialProperties` array for the given
/// MToon parameter set. Caller assembles the surrounding array.
pub fn emit_material_property(params: &MToonParams) -> Value {
    let shading_shift = crate::mtoon_common::shading_shift_factor(params);
    let shading_toony = crate::mtoon_common::shading_toony_factor(params);
    let [rim_r, rim_g, rim_b] = crate::mtoon_common::parametric_rim_color_factor(params);
    let outline_width = crate::mtoon_common::outline_width_factor(params);

    let outline_color_rgba = {
        let [r, g, b] = params.outline_color_factor;
        [r, g, b, 1.0_f32]
    };

    json!({
        "name": params.id,
        "renderQueue": 2000 + params.render_queue_offset_number,
        "shader": "VRM/MToon",
        "floatProperties": {
            "_ShadeShift": shading_shift,
            "_ShadeToony": shading_toony,
            "_OutlineWidth": outline_width,
            "_OutlineWidthMode": match params.outline_width_mode {
                OutlineWidthMode::None => 0,
                OutlineWidthMode::WorldCoordinates => 1,
                OutlineWidthMode::ScreenCoordinates => 2,
            },
            // Note: 0.x MToon does NOT have _OutlineLightingMix —
            // outline_lighting_mix_factor is v1-only. Sweep variants exercising
            // it are NotApplicable on the 0.x side (OutlineLightingMixV1Only).
        },
        "vectorProperties": {
            "_Color": [1.0_f32, 1.0, 1.0, 1.0],
            "_ShadeColor": [0.7_f32, 0.7, 0.7, 1.0],
            "_OutlineColor": outline_color_rgba,
            "_RimColor": [rim_r, rim_g, rim_b, 1.0_f32],
        },
        "textureProperties": {},
        "keywordMap": {},
        "tagMap": {},
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::MToonParams;

    #[test]
    fn emits_unity_shader_key_naming() {
        let p = MToonParams::defaults("test_v0");
        let v = emit_material_property(&p);
        assert_eq!(v["shader"], "VRM/MToon");
        // Unity-shader keys MUST have leading underscores.
        let float_props = v["floatProperties"].as_object().unwrap();
        for k in float_props.keys() {
            assert!(
                k.starts_with('_'),
                "key {k} missing Unity-shader leading underscore"
            );
        }
        let vector_props = v["vectorProperties"].as_object().unwrap();
        for k in vector_props.keys() {
            assert!(
                k.starts_with('_'),
                "key {k} missing Unity-shader leading underscore"
            );
        }
    }

    #[test]
    fn does_not_emit_outline_lighting_mix_v1_only_field() {
        let p = MToonParams::defaults("test_v0");
        let v = emit_material_property(&p);
        // 0.x has no outline lighting mix; the v1-only axis surfaces as
        // NotApplicable in the sweep registry, not as a 0.x material field.
        let float_props = v["floatProperties"].as_object().unwrap();
        assert!(!float_props.contains_key("_OutlineLightingMix"));
        assert!(!float_props.contains_key("_OutlineLightingMixFactor"));
    }

    #[test]
    fn outline_width_mode_maps_to_int_enum() {
        let mut p = MToonParams::defaults("test_v0");
        p.outline_width_mode = OutlineWidthMode::None;
        assert_eq!(
            emit_material_property(&p)["floatProperties"]["_OutlineWidthMode"],
            0
        );
        p.outline_width_mode = OutlineWidthMode::WorldCoordinates;
        assert_eq!(
            emit_material_property(&p)["floatProperties"]["_OutlineWidthMode"],
            1
        );
        p.outline_width_mode = OutlineWidthMode::ScreenCoordinates;
        assert_eq!(
            emit_material_property(&p)["floatProperties"]["_OutlineWidthMode"],
            2
        );
    }

    #[test]
    fn shader_is_vrm_mtoon() {
        let p = MToonParams::defaults("test_v0");
        let v = emit_material_property(&p);
        assert_eq!(v["shader"], "VRM/MToon");
    }

    #[test]
    fn name_is_the_params_id() {
        let p = MToonParams::defaults("custom_id_v0");
        let v = emit_material_property(&p);
        assert_eq!(v["name"], "custom_id_v0");
    }
}
