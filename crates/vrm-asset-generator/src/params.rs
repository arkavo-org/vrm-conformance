//! Parameter dictionary for MToon material generation.
//!
//! Every emission is fully described by a `MToonParams` value plus a fixed
//! mesh fixture. The same dictionary that produces an asset's binary content
//! also produces the sidecar `.meta.json` and `.test.yaml`, eliminating
//! desync risk between asset and test plan.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MToonParams {
    pub id: String,

    pub base_color_factor: [f32; 4],
    pub shade_color_factor: [f32; 3],

    pub shading_shift_factor: f32,
    pub shading_toony_factor: f32,
    pub gi_equalization_factor: f32,

    pub parametric_rim_color_factor: [f32; 3],
    pub parametric_rim_fresnel_power_factor: f32,
    pub parametric_rim_lift_factor: f32,
    pub rim_lighting_mix_factor: f32,

    pub matcap_factor: [f32; 3],

    pub outline_width_mode: OutlineWidthMode,
    pub outline_width_factor: f32,
    pub outline_color_factor: [f32; 3],
    pub outline_lighting_mix_factor: f32,

    pub uv_animation_scroll_x_speed_factor: f32,
    pub uv_animation_scroll_y_speed_factor: f32,
    pub uv_animation_rotation_speed_factor: f32,

    /// glTF `material.emissiveFactor` (linear RGB, per-channel ∈ [0,1]).
    /// Default [0,0,0] = no emission, in which case `emissive_multiplier`
    /// has no observable effect and the extension is not emitted.
    pub emissive_factor: [f32; 3],
    /// `VRMC_materials_hdr_emissiveMultiplier-1.0`: when the effective
    /// emission is `emissive_factor * emissive_multiplier`, this lets it
    /// exceed 1.0 (HDR). The extension is emitted only when
    /// `emissive_multiplier != 1.0` AND `emissive_factor != [0,0,0]`.
    /// glTF default if extension omitted is implicit multiplier=1.
    pub emissive_multiplier: f32,

    /// `VRMC_vrm.firstPerson.meshAnnotations[*].type` override for the
    /// avatar's mesh-bearing node. `None` keeps the canonical default
    /// (`auto`) so existing assets stay byte-identical. The four valid
    /// spec values (per VRMC_vrm-1.0 firstPerson.md) are surfaced via
    /// the `FirstPersonType` enum so the sweep can drive each path.
    pub first_person_type: Option<FirstPersonType>,

    pub alpha_mode: AlphaMode,
    /// glTF `alphaCutoff`. Meaningful only when `alpha_mode == Mask`;
    /// emitted in the material JSON only on Mask. glTF default is 0.5.
    pub alpha_cutoff: f32,
    pub transparent_with_z_write: bool,
    pub render_queue_offset_number: i32,

    pub double_sided: bool,
}

impl MToonParams {
    /// Defaults match the VRMC_materials_mtoon spec defaults wherever
    /// defined; otherwise a neutrally-rendering value.
    pub fn defaults(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            shade_color_factor: [0.5, 0.5, 0.5],
            shading_shift_factor: 0.0,
            shading_toony_factor: 0.9,
            gi_equalization_factor: 0.9,
            parametric_rim_color_factor: [0.0, 0.0, 0.0],
            parametric_rim_fresnel_power_factor: 5.0,
            parametric_rim_lift_factor: 0.0,
            rim_lighting_mix_factor: 0.0,
            matcap_factor: [1.0, 1.0, 1.0],
            outline_width_mode: OutlineWidthMode::None,
            outline_width_factor: 0.0,
            outline_color_factor: [0.0, 0.0, 0.0],
            outline_lighting_mix_factor: 1.0,
            uv_animation_scroll_x_speed_factor: 0.0,
            uv_animation_scroll_y_speed_factor: 0.0,
            uv_animation_rotation_speed_factor: 0.0,
            emissive_factor: [0.0, 0.0, 0.0],
            emissive_multiplier: 1.0,
            first_person_type: None,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            transparent_with_z_write: false,
            render_queue_offset_number: 0,
            double_sided: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutlineWidthMode {
    None,
    WorldCoordinates,
    ScreenCoordinates,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

/// `VRMC_vrm.firstPerson.meshAnnotations[*].type` values per the
/// VRMC_vrm-1.0 spec (`firstPerson.md` enum table). camelCase serde
/// because that matches the spec wire form exactly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FirstPersonType {
    /// Renderer splits the mesh by head-bone weight (default when
    /// `meshAnnotations` is absent or missing for a node).
    Auto,
    /// Visible from every camera. The "no culling" baseline.
    Both,
    /// Hidden from first-person (HMD) camera; visible from third-person.
    /// Standard for the head/hair/face meshes of avatars used in VR.
    ThirdPersonOnly,
    /// Visible only from first-person camera; hidden from third-person.
    /// Conventionally used for UI overlays attached to the avatar.
    FirstPersonOnly,
}

impl FirstPersonType {
    /// String value matching the spec's enum (lowerCamelCase). Used by
    /// the generator and by adapters that key off the raw string.
    pub fn as_spec_str(self) -> &'static str {
        match self {
            FirstPersonType::Auto => "auto",
            FirstPersonType::Both => "both",
            FirstPersonType::ThirdPersonOnly => "thirdPersonOnly",
            FirstPersonType::FirstPersonOnly => "firstPersonOnly",
        }
    }
}
