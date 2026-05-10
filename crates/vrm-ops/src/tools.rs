//! Operation parameter and result types.
//!
//! These are the structured-CLI args (after `--json` parsing) and the
//! JSON-RPC request `params` / response `result` payloads. Same types,
//! same schemas, two transports.

use serde::{Deserialize, Serialize};

// ---- Phase 1 required operations ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCameraParams {
    pub session_id: String,
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLightingParams {
    pub session_id: String,
    pub directional: Directional,
    pub ambient: Ambient,
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default)]
    pub receive_shadows: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directional {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ambient {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPostProcessingParams {
    pub session_id: String,
    pub tone_mapping: ToneMapping,
    pub exposure: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ToneMapping {
    None,
    Linear,
    Reinhard,
    Aces,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderParams {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
    pub output_path: String,
    pub color_space: ColorSpace,
    pub msaa: u8,
    pub output_type: OutputType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    pub output_path: String,
    pub actual_color_space: ColorSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ColorSpace {
    Linear,
    Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OutputType {
    Color,
    Normal,
    Depth,
    Albedo,
    MToonShadingMask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisposeParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPhysicsParams {
    pub session_id: String,
    pub dt_seconds: f32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPhysicsParams {
    pub session_id: String,
    pub settle_steps: u32,
}

/// Drives a linear root-transform animation so spring-bone chains experience
/// inertia/drag, not just gravity settling. The adapter is expected to
/// step physics at `fps` Hz over `duration_seconds`, interpolating the root
/// translation from `translation_start` to `translation_end` and calling
/// the renderer's physics update between samples. After the call returns,
/// a subsequent `render` captures whatever post-animation pose resulted.
///
/// Translation-only in v0.1 — rotation excitation lands when there's a real
/// test case calling for it. Methodology: `docs/methodology.md`,
/// "Spring bone excitation".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateRootTransformParams {
    pub session_id: String,
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
    pub duration_seconds: f32,
    pub fps: u32,
}

/// Empty result type for ops that return no payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitResult {}
