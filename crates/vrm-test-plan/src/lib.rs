//! YAML schema for VRM conformance test plans.
//!
//! See `docs/methodology.md` for why specific defaults exist (tone_mapping=none,
//! shadows off, MSAA 4x).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestPlan {
    pub id: String,
    pub spec_section: String,
    pub asset: String,
    pub camera: Camera,
    pub lighting: Lighting,
    #[serde(default)]
    pub post_processing: PostProcessing,
    pub output: Output,
    pub diff: Diff,
    #[serde(default)]
    pub ignore_renderers: Vec<String>,
    #[serde(default)]
    pub properties: Vec<PropertyAssertion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lighting {
    pub directional: DirectionalLight,
    pub ambient: AmbientLight,
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default)]
    pub receive_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AmbientLight {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostProcessing {
    #[serde(default = "default_tone_mapping")]
    pub tone_mapping: ToneMapping,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
}

impl Default for PostProcessing {
    fn default() -> Self {
        Self {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        }
    }
}

fn default_tone_mapping() -> ToneMapping {
    ToneMapping::None
}

fn default_exposure() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToneMapping {
    None,
    Linear,
    Reinhard,
    Aces,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    pub msaa: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSpace {
    Linear,
    Srgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    pub mode: DiffMode,
    pub threshold: f32,
    pub reference_renderer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffMode {
    Ssim,
    Consensus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyAssertion {
    pub name: String,
    pub region: BboxRegion,
    pub expected: f32,
    pub tolerance: f32,
}

/// Bounding-box-relative regions, robust to small FOV / projection
/// variation across renderers.
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
