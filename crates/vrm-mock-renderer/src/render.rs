//! Deterministic parameter-encoded PNG synthesis. Not a renderer in any
//! meaningful sense — pixel values are a stable, idempotent function of
//! `MToonParams`. Identical params → byte-identical `RgbImage`.

use image::{Rgb, RgbImage};
use vrm_asset_generator::params::{MToonParams, OutlineWidthMode};

const MAGENTA: [u8; 3] = [255, 0, 255];

/// Synthesize a deterministic mock-rendered avatar fingerprint at the
/// given resolution. The image always has:
///   * Magenta sentinel background outside the avatar bbox.
///   * A centered avatar bbox covering 50% of width × 50% of height.
///   * Fill RGB encoded from `base_color_factor` × shading-shift brightness.
///   * Top stripe encoded from `shade_color_factor` × shading-toony.
///   * Right stripe encoded from `parametric_rim_color_factor` × rim-mix.
///   * 1-pixel outline ring colored by `outline_color_factor` when
///     `outline_width_mode != None`.
pub fn synthesize_png(params: &MToonParams, width: u32, height: u32) -> RgbImage {
    let mut img = RgbImage::from_pixel(width, height, Rgb(MAGENTA));

    let (x0, y0, x1, y1) = avatar_bbox(width, height);

    // Brightness multiplier from shading_shift_factor:
    // shift in [-1.0, 1.0] → brightness in [0.0, 1.0], centered at 0.5.
    let shift_brightness = ((params.shading_shift_factor + 1.0) / 2.0).clamp(0.0, 1.0);
    let fill = scale_color3(rgba_to_rgb(params.base_color_factor), shift_brightness);

    // Shade stripe: shade_color modulated by toony factor.
    let shade = scale_color3(
        params.shade_color_factor,
        params.shading_toony_factor.clamp(0.0, 1.0),
    );

    // Rim stripe: rim color modulated by mix factor.
    let rim = scale_color3(
        params.parametric_rim_color_factor,
        params.rim_lighting_mix_factor.clamp(0.0, 1.0),
    );

    // Body fill.
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, Rgb(fill));
        }
    }

    // Top stripe — 8 pixels high inside the bbox top.
    let top_stripe_end = (y0 + 8).min(y1);
    for y in y0..top_stripe_end {
        for x in x0..x1 {
            img.put_pixel(x, y, Rgb(shade));
        }
    }

    // Right stripe — 8 pixels wide inside the bbox right edge.
    let right_stripe_start = (x1.saturating_sub(8)).max(x0);
    for y in y0..y1 {
        for x in right_stripe_start..x1 {
            img.put_pixel(x, y, Rgb(rim));
        }
    }

    // Outline ring (when enabled).
    if !matches!(params.outline_width_mode, OutlineWidthMode::None) {
        let outline = float3_to_u8(params.outline_color_factor);
        for x in x0..x1 {
            img.put_pixel(x, y0, Rgb(outline));
            img.put_pixel(x, y1.saturating_sub(1), Rgb(outline));
        }
        for y in y0..y1 {
            img.put_pixel(x0, y, Rgb(outline));
            img.put_pixel(x1.saturating_sub(1), y, Rgb(outline));
        }
    }

    img
}

fn avatar_bbox(width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x0 = width / 4;
    let y0 = height / 4;
    let x1 = (width * 3) / 4;
    let y1 = (height * 3) / 4;
    (x0, y0, x1, y1)
}

fn rgba_to_rgb(rgba: [f32; 4]) -> [f32; 3] {
    [rgba[0], rgba[1], rgba[2]]
}

fn scale_color3(rgb: [f32; 3], scale: f32) -> [u8; 3] {
    let s = scale.clamp(0.0, 1.0);
    [
        f_to_u8(rgb[0] * s),
        f_to_u8(rgb[1] * s),
        f_to_u8(rgb[2] * s),
    ]
}

fn float3_to_u8(rgb: [f32; 3]) -> [u8; 3] {
    [f_to_u8(rgb[0]), f_to_u8(rgb[1]), f_to_u8(rgb[2])]
}

fn f_to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
