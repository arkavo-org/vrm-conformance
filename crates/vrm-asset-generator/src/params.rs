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

    pub alpha_mode: AlphaMode,
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
            alpha_mode: AlphaMode::Opaque,
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
