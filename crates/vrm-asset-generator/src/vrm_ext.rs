//! Builds the JSON fragments for `VRMC_vrm` and `VRMC_materials_mtoon`.
//!
//! Spec references:
//! - VRMC_vrm: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_vrm-1.0
//! - VRMC_materials_mtoon: https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_materials_mtoon-1.0

use crate::params::{AlphaMode, MToonParams, OutlineWidthMode};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Build the VRMC_vrm extension JSON.
///
/// `bone_to_node` maps VRM bone names to glTF node indices (from the
/// humanoid skeleton). `mesh_node` is the glTF node index that carries the
/// renderable mesh; it is annotated as `auto` first-person so the
/// `firstPerson.meshAnnotations` array is non-empty (the validator rejects
/// an empty `meshAnnotations` entry).
pub fn vrmc_vrm(
    meta_name: &str,
    bone_to_node: &BTreeMap<String, usize>,
    mesh_node: usize,
) -> Value {
    let human_bones: serde_json::Map<String, Value> = bone_to_node
        .iter()
        .map(|(name, idx)| (name.clone(), json!({ "node": idx })))
        .collect();

    json!({
        "specVersion": "1.0",
        "meta": {
            "name": meta_name,
            "version": "0.1.0",
            "authors": ["arkavo-org/vrm-conformance generator"],
            "licenseUrl": "https://vrm.dev/licenses/1.0/",
            "thirdPartyLicenses": "",
            "avatarPermission": "everyone",
            "allowExcessivelyViolentUsage": false,
            "allowExcessivelySexualUsage": false,
            "commercialUsage": "personalNonProfit",
            "allowPoliticalOrReligiousUsage": false,
            "allowAntisocialOrHateUsage": false,
            "creditNotation": "unnecessary",
            "allowRedistribution": true,
            "modification": "allowModification"
        },
        "humanoid": {
            "humanBones": human_bones
        },
        "firstPerson": {
            "meshAnnotations": [
                { "node": mesh_node, "type": "auto" }
            ]
        },
        "lookAt": {
            "type": "bone",
            "offsetFromHeadBone": [0.0, 0.06, 0.0],
            "rangeMapHorizontalInner": { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapHorizontalOuter": { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapVerticalDown":     { "inputMaxValue": 90.0, "outputScale": 10.0 },
            "rangeMapVerticalUp":       { "inputMaxValue": 90.0, "outputScale": 10.0 }
        },
        "expressions": {
            "preset": {}
        }
    })
}

/// Build the per-material VRMC_materials_mtoon extension JSON.
pub fn vrmc_materials_mtoon(p: &MToonParams) -> Value {
    let outline_width_mode = match p.outline_width_mode {
        OutlineWidthMode::None => "none",
        OutlineWidthMode::WorldCoordinates => "worldCoordinates",
        OutlineWidthMode::ScreenCoordinates => "screenCoordinates",
    };

    json!({
        "specVersion": "1.0",
        "transparentWithZWrite": p.transparent_with_z_write,
        "renderQueueOffsetNumber": p.render_queue_offset_number,
        "shadeColorFactor": p.shade_color_factor,
        "shadingShiftFactor": p.shading_shift_factor,
        "shadingToonyFactor": p.shading_toony_factor,
        "giEqualizationFactor": p.gi_equalization_factor,
        "matcapFactor": p.matcap_factor,
        "parametricRimColorFactor": p.parametric_rim_color_factor,
        "parametricRimFresnelPowerFactor": p.parametric_rim_fresnel_power_factor,
        "parametricRimLiftFactor": p.parametric_rim_lift_factor,
        "rimLightingMixFactor": p.rim_lighting_mix_factor,
        "outlineWidthMode": outline_width_mode,
        "outlineWidthFactor": p.outline_width_factor,
        "outlineColorFactor": p.outline_color_factor,
        "outlineLightingMixFactor": p.outline_lighting_mix_factor,
        "uvAnimationScrollXSpeedFactor": p.uv_animation_scroll_x_speed_factor,
        "uvAnimationScrollYSpeedFactor": p.uv_animation_scroll_y_speed_factor,
        "uvAnimationRotationSpeedFactor": p.uv_animation_rotation_speed_factor
    })
}

/// glTF base material wrapping MToon. MToon depends on KHR_materials_unlit
/// in the base material so non-MToon-aware viewers fall back gracefully.
pub fn base_material(p: &MToonParams) -> Value {
    let alpha_mode = match p.alpha_mode {
        AlphaMode::Opaque => "OPAQUE",
        AlphaMode::Mask => "MASK",
        AlphaMode::Blend => "BLEND",
    };

    json!({
        "name": p.id,
        "pbrMetallicRoughness": {
            "baseColorFactor": p.base_color_factor,
            "metallicFactor": 0.0,
            "roughnessFactor": 0.9
        },
        "alphaMode": alpha_mode,
        "doubleSided": p.double_sided,
        "extensions": {
            "KHR_materials_unlit": {},
            "VRMC_materials_mtoon": vrmc_materials_mtoon(p)
        }
    })
}
