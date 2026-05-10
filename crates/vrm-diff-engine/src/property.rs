//! Bounding-box-relative property assertions.
//!
//! v0.1 assumption: background is the magenta sentinel [255, 0, 255]. Any
//! non-magenta pixel is "avatar." The screen-space bbox is the smallest
//! rectangle containing all avatar pixels. Region samples are taken within
//! that bbox.

use image::RgbImage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BboxRegion {
    BboxFull,
    BboxLowerLeftQuadrant,
    BboxLowerRightQuadrant,
    BboxUpperLeftQuadrant,
    BboxUpperRightQuadrant,
    BboxCenterStripHorizontal,
    BboxCenterStripVertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyAssertion {
    pub name: String,
    pub region: BboxRegion,
    pub expected: f32,
    pub tolerance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyResult {
    pub name: String,
    pub actual: f32,
    pub expected: f32,
    pub tolerance: f32,
    pub passed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PropertyError {
    #[error("avatar bbox is empty (image is all-background)")]
    EmptyBbox,
    #[error("region sampled zero non-background pixels (region falls outside avatar)")]
    EmptyRegion,
}

pub fn compute_avatar_bbox(img: &RgbImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            // Magenta sentinel = background.
            if p.0 == [255, 0, 255] {
                continue;
            }
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    if max_x < min_x || max_y < min_y {
        None
    } else {
        Some((min_x, min_y, max_x, max_y))
    }
}

pub fn region_pixel_range(bbox: (u32, u32, u32, u32), region: BboxRegion) -> (u32, u32, u32, u32) {
    let (x0, y0, x1, y1) = bbox;
    let mid_x = x0 + (x1 - x0) / 2;
    let mid_y = y0 + (y1 - y0) / 2;
    let strip_x = ((x1 - x0) / 4).max(1);
    let strip_y = ((y1 - y0) / 4).max(1);

    match region {
        BboxRegion::BboxFull => (x0, y0, x1, y1),
        BboxRegion::BboxUpperLeftQuadrant => (x0, y0, mid_x, mid_y),
        BboxRegion::BboxUpperRightQuadrant => (mid_x, y0, x1, mid_y),
        BboxRegion::BboxLowerLeftQuadrant => (x0, mid_y, mid_x, y1),
        BboxRegion::BboxLowerRightQuadrant => (mid_x, mid_y, x1, y1),
        BboxRegion::BboxCenterStripHorizontal => (
            x0,
            mid_y.saturating_sub(strip_y),
            x1,
            (mid_y + strip_y).min(y1),
        ),
        BboxRegion::BboxCenterStripVertical => (
            mid_x.saturating_sub(strip_x),
            y0,
            (mid_x + strip_x).min(x1),
            y1,
        ),
    }
}

pub fn eval_property(
    img: &RgbImage,
    pa: &PropertyAssertion,
) -> Result<PropertyResult, PropertyError> {
    let bbox = compute_avatar_bbox(img).ok_or(PropertyError::EmptyBbox)?;
    let (rx0, ry0, rx1, ry1) = region_pixel_range(bbox, pa.region);

    let mut sum = 0f64;
    let mut count = 0u64;
    for y in ry0..=ry1 {
        for x in rx0..=rx1 {
            let p = img.get_pixel(x, y);
            if p.0 == [255, 0, 255] {
                continue;
            }
            let lum = 0.2126 * (p.0[0] as f64 / 255.0)
                + 0.7152 * (p.0[1] as f64 / 255.0)
                + 0.0722 * (p.0[2] as f64 / 255.0);
            sum += lum;
            count += 1;
        }
    }
    if count == 0 {
        return Err(PropertyError::EmptyRegion);
    }
    let actual = sum / count as f64;
    let actual = actual as f32;

    let passed = (actual - pa.expected).abs() <= pa.tolerance;

    Ok(PropertyResult {
        name: pa.name.clone(),
        actual,
        expected: pa.expected,
        tolerance: pa.tolerance,
        passed,
    })
}
