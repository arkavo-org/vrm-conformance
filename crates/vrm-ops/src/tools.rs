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

/// Dump world-space joint positions for spring-bone chains as of the most
/// recent state-advancing op (`render`, `step_physics`, `reset_physics`,
/// `animate_root_transform`). The op itself does NOT advance physics.
///
/// If `spring_index` is omitted, all springs in the loaded model are
/// returned. If provided, only that spring's positions are returned;
/// out-of-range indices return an empty `springs` array — this is
/// intentionally permissive so callers can probe spring count without state.
///
/// Adapters that have no spring-bone system or return rest-pose only (e.g.
/// univrm L3) MAY return `-32000 Unimplemented` with the standard phase
/// envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpBonePositionsParams {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring_index: Option<usize>,
}

/// Per-spring joint positions captured at a single simulation instant (world-space XYZ, metres).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpringPositions {
    pub name: String,
    pub joint_positions: Vec<[f32; 3]>,
}

/// Result of `dump_bone_positions`: one `SpringPositions` entry per spring chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpBonePositionsResult {
    pub springs: Vec<SpringPositions>,
}

/// Load a `.vrma` file (VRMC_vrm_animation glTF) and return an opaque handle
/// plus a summary of the channels it contains. Only the first animation
/// (`animations[0]`) is treated as the portable clip per VRMA spec; multi-
/// animation `.vrma` files are accepted but only `animations[0]` is sampled.
///
/// Adapters that do not implement VRMA MUST return `-32000 Unimplemented`
/// with `data: { phase: "vrma-v1" }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmaParams {
    /// Filesystem path to a `.vrma` file. BLAKE3 refs (`blake3:<64-hex>`)
    /// are also accepted by adapters that resolve content-addressed inputs.
    pub vrma_path: String,
}

/// Summary of the channels a loaded `.vrma` references.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VrmaChannelSummary {
    /// Count of humanoid bones referenced (by `humanoid.humanBones`).
    pub humanoid_bones: u32,
    /// Count of expressions referenced (preset + custom combined).
    pub expressions: u32,
    /// True if the `.vrma` contains a `lookAt` block.
    pub has_look_at: bool,
    /// Duration of `animations[0]` in seconds.
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmaResult {
    /// Opaque handle the adapter assigns; subsequent ops reference this.
    pub vrma_handle: u32,
    pub channel_summary: VrmaChannelSummary,
}

/// Sample the loaded `.vrma` at `time_seconds` and write the resulting pose
/// onto the avatar identified by `vrm_handle`. Linear interpolation is the
/// spec-mandated default. State-advancing — the subsequent `dump_*` ops
/// capture this op's effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyVrmaAtTimeParams {
    pub session_id: String,
    pub vrma_handle: u32,
    pub vrm_handle: u32,
    pub time_seconds: f32,
}

/// Per-channel application counts. Lets callers verify that each channel
/// in the loaded `.vrma` was actually applied (zero counts surface
/// silent-skip bugs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VrmaChannelsApplied {
    pub humanoid_bones: u32,
    pub expressions: u32,
    pub look_at: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyVrmaAtTimeResult {
    pub channels_applied: VrmaChannelsApplied,
}

/// Empty result type for ops that return no payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_bone_positions_params_roundtrip_with_spring_index() {
        let p = DumpBonePositionsParams {
            session_id: "sess-1".into(),
            spring_index: Some(2),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: DumpBonePositionsParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.spring_index, Some(2));
    }

    #[test]
    fn dump_bone_positions_params_omits_spring_index_when_none() {
        let p = DumpBonePositionsParams {
            session_id: "sess-1".into(),
            spring_index: None,
        };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert!(
            v.get("spring_index").is_none(),
            "spring_index None should be omitted, got {v}"
        );
        let back: DumpBonePositionsParams = serde_json::from_value(v).unwrap();
        assert_eq!(back.spring_index, None);
    }

    #[test]
    fn dump_bone_positions_result_roundtrip() {
        let r = DumpBonePositionsResult {
            springs: vec![SpringPositions {
                name: "hair_chain".into(),
                joint_positions: vec![[0.0, 1.0, 0.0], [0.0, 0.95, 0.0]],
            }],
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: DumpBonePositionsResult = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }
}
