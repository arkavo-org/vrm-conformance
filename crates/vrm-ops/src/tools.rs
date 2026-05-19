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

/// Dump per-bone local rotations + hips translation for the loaded VRM as
/// of the most recent state-advancing op (`apply_vrma_at_time`, `render`,
/// `reset_physics`, etc.). Per the VRMA spec, only the `hips` bone carries
/// translation; all other humanoid bones contribute only rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpHumanoidPoseParams {
    pub session_id: String,
}

/// Single humanoid bone rotation. The name follows the spec's bone-name
/// enum (`hips`, `leftUpperArm`, ...). Quaternion in `[x, y, z, w]` order
/// matching glTF convention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HumanoidBoneRotation {
    pub name: String,
    pub local_rotation_quat: [f32; 4],
}

/// Bones present in the .vrm with their local rotations, plus the hips
/// translation, plus any bones that the .vrma referenced but the .vrm
/// does not have (excluded from per-bone diff per methodology hazard #3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpHumanoidPoseResult {
    pub bones: Vec<HumanoidBoneRotation>,
    pub hips_translation: [f32; 3],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bones_missing: Vec<String>,
}

/// Dump current expression weights (preset + custom). Per the VRMA spec,
/// weights are encoded as the X-component of bound-node translation
/// animation, clamped to [0, 1]; this op returns the clamped values the
/// renderer actually applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpExpressionWeightsParams {
    pub session_id: String,
}

/// Preset and custom expressions kept structurally separate per spec.
/// Preset name set per VRMA spec: happy, angry, sad, relaxed, surprised,
/// aa, ih, ou, ee, oh, blink, blinkLeft, blinkRight, neutral.
/// `lookUp/lookDown/lookLeft/lookRight` are NOT VRMA presets — driven by
/// LookAt and reported via `dump_look_at_state` instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpExpressionWeightsResult {
    pub presets: std::collections::BTreeMap<String, f32>,
    pub custom: std::collections::BTreeMap<String, f32>,
}

/// Dump current eye gaze state. Per the VRMA spec, the .vrma file declares
/// gaze direction via a node rotation quaternion plus `offsetFromHeadBone`.
/// The avatar's `VRMC_vrm.lookAt.type` (bone vs aim) determines how that
/// direction is applied — that distinction lives in the avatar config,
/// not in VRMA. This op exposes both:
///   - the raw VRMA-declared gaze (quat + spec-defined Extrinsic ZXY
///     yaw/pitch)
///   - the avatar's application mode (`applied_via`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpLookAtStateParams {
    pub session_id: String,
}

/// How the avatar (per its `VRMC_vrm.lookAt.type`) applies the gaze
/// direction declared by the .vrma. Reported by the adapter from the
/// avatar's config, not derived from the .vrma.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LookAtAppliedVia {
    /// Avatar `VRMC_vrm.lookAt.type: bone` — gaze rotates head/eye bones.
    Bone,
    /// Avatar `VRMC_vrm.lookAt.type: expression` — gaze drives lookUp/
    /// lookDown/lookLeft/lookRight preset expressions.
    Expression,
    /// Avatar has no LookAt configured, or the renderer doesn't apply it.
    Off,
}

/// Raw quaternion gaze direction + spec-defined Extrinsic ZXY yaw/pitch
/// (yaw = rotation around Y, pitch = rotation around X) + avatar's
/// application mode + head-local offset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpLookAtStateResult {
    pub gaze_direction_quat: [f32; 4],
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub applied_via: LookAtAppliedVia,
    pub offset_from_head_bone: [f32; 3],
}

/// Empty result type for ops that return no payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitResult {}

/// Output format for `render_sequence`. PNG sequence is canonical for diff;
/// MP4 and MOV are convenience formats for site display + reviewer ergonomics.
/// When MP4 or MOV is requested, the adapter MUST still emit the per-frame
/// PNGs (diff consumes them); the muxed file is in addition, not instead.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFormat {
    /// Per-frame PNG at `<output_dir>/<frame_index:04>.png`. Canonical for
    /// diff; lossless; BLAKE3-identifiable for identity short-circuit.
    PngSequence,
    /// PNG sequence + muxed `<output_dir>/sequence.mp4` (h.264 yuv420p,
    /// lossless qp=0).
    Mp4,
    /// PNG sequence + muxed `<output_dir>/sequence.mov` (Apple ProRes 4444).
    Mov,
}

/// Linear root-transform animation interpolated across the captured frames
/// of a `render_sequence` call. The adapter samples translation at
/// `t = i / (frame_count - 1)` for each captured frame i ∈ [0, frame_count),
/// where `frame_count` is taken from the owning `RenderSequenceParams`.
/// Duration is implicit: `frame_count / frame_hz` seconds (both fields on
/// `RenderSequenceParams`).
///
/// Distinct from the v0.1 `AnimateRootTransformParams` because that op
/// carries its own `duration_seconds` + `fps` (single-shot animation, then
/// one render). For sequence-driven animation, those fields are redundant
/// with the sequence's own `frame_count` + `frame_hz`, so they're omitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootTransformAnimation {
    pub translation_start: [f32; 3],
    pub translation_end: [f32; 3],
}

/// VRMA playback spec for `render_sequence`. Samples the loaded `.vrma` at
/// `t = start_seconds + (i / frame_hz)` for each captured frame i.
/// Display clock drives sampling; physics clock is internal to the adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VrmaPlaybackSpec {
    pub vrma_handle: u32,
    /// Offset into the .vrma clip where capture begins. Use 0.0 to start
    /// at clip beginning.
    pub start_seconds: f32,
}

/// One captured frame of a `render_sequence` result. The BLAKE3 hash lets
/// the diff engine short-circuit when both renderers produced byte-identical
/// frames (common for rest-pose lead-in frames).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SequenceFrame {
    pub index: u32,
    pub timestamp_seconds: f32,
    pub path: String,
    /// BLAKE3 hex of the PNG contents, prefixed `blake3:` per the content-
    /// addressing convention in `docs/operation-contract.md`.
    pub blake3: String,
}

/// Capture N frames at a fixed display Hz while advancing physics (and
/// optionally a .vrma clip or root-transform animation) between samples.
/// The adapter steps physics by `physics_dt_seconds` between each captured
/// frame (decoupled from `frame_hz` so simulation determinism and display
/// timing are configurable independently — see `docs/methodology.md`,
/// "Sequence captures").
///
/// Frames land at `output_dir/<frame_index:04>.png` for PNG sequences;
/// a single muxed file at `output_dir/sequence.mp4` (or `.mov`) for the
/// muxed formats. The diff engine consumes PNG sequences canonically;
/// muxed formats are convenience for site display.
///
/// Adapters that cannot produce sequences MUST return `-32000 Unimplemented`
/// with `data: { phase: "v1.x-sequence" }`.
///
/// Validation:
///   - If both `animate_root_transform` and `apply_vrma` are `Some`, the
///     adapter MUST return `-32602 invalid params`.
///   - If `physics_dt_seconds > 1.0 / 60.0`, the adapter SHOULD return
///     `-32602 invalid params` (methodology pin: 60 Hz physics floor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSequenceParams {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
    pub output_dir: String,
    pub frame_count: u32,
    pub frame_hz: f32,
    pub physics_dt_seconds: f32,
    pub color_space: ColorSpace,
    pub msaa: u8,
    pub output_type: OutputType,
    pub output_format: SequenceFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate_root_transform: Option<RootTransformAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_vrma: Option<VrmaPlaybackSpec>,
}

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
