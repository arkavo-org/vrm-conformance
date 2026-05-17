//! Pose-vector diff for VRMA conformance: per-bone quaternion geodesic
//! distance + hips translation + expression weight deltas + lookAt angle
//! deltas. Returns a structured report; pass/fail decided by per-channel
//! tolerances supplied by the caller.

use serde::{Deserialize, Serialize};
use vrm_ops::tools::{
    DumpExpressionWeightsResult, DumpHumanoidPoseResult, DumpLookAtStateResult,
};

/// Per-channel tolerances. All in their respective units (radians, meters,
/// scalar deltas, degrees). A pass requires every per-channel max to be
/// at or below its tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseTolerances {
    pub per_bone_quaternion_radians: f32,
    pub hips_translation_m: f32,
    pub per_preset_expression: f32,
    pub per_custom_expression: f32,
    pub look_at_yaw_pitch_degrees: f32,
    pub offset_from_head_bone_m: f32,
}

impl Default for PoseTolerances {
    /// v1 defaults from the design spec.
    fn default() -> Self {
        Self {
            per_bone_quaternion_radians: 0.010,
            hips_translation_m: 0.005,
            per_preset_expression: 0.005,
            per_custom_expression: 0.005,
            look_at_yaw_pitch_degrees: 1.0,
            offset_from_head_bone_m: 0.001,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoseDiffReport {
    pub per_bone_rotation_max_rad: f32,
    pub per_bone_rotation_worst_bone: Option<String>,
    pub hips_translation_m: f32,
    pub per_preset_expression_max_delta: f32,
    pub per_preset_expression_worst: Option<String>,
    pub per_custom_expression_max_delta: f32,
    pub per_custom_expression_worst: Option<String>,
    pub look_at_yaw_delta_deg: f32,
    pub look_at_pitch_delta_deg: f32,
    pub offset_from_head_bone_m: f32,
    pub overall_passed: bool,
}

/// Diff the actual pose against a reference. Returns a structured report.
/// Pass requires every per-channel max to be at or below its tolerance.
///
/// Channels are diffed independently:
/// - Humanoid bone rotations: quaternion geodesic distance `2·acos(|q·q'|)`
///   (sign-invariant by construction). Bones in `actual.bones_missing` or
///   `reference.bones_missing` are excluded (methodology hazard #3).
/// - Hips translation: Euclidean distance.
/// - Expression weights: scalar abs-delta. Presets and custom kept
///   separate.
/// - LookAt: yaw and pitch abs-delta in degrees (each compared
///   independently; the worse of the two contributes to pass/fail).
/// - offsetFromHeadBone: Euclidean distance.
pub fn diff_pose(
    actual_humanoid: &DumpHumanoidPoseResult,
    reference_humanoid: &DumpHumanoidPoseResult,
    actual_expressions: &DumpExpressionWeightsResult,
    reference_expressions: &DumpExpressionWeightsResult,
    actual_look_at: &DumpLookAtStateResult,
    reference_look_at: &DumpLookAtStateResult,
    tolerances: &PoseTolerances,
) -> PoseDiffReport {
    // Stub — body lands in tasks 2-4.
    let _ = (
        actual_humanoid,
        reference_humanoid,
        actual_expressions,
        reference_expressions,
        actual_look_at,
        reference_look_at,
        tolerances,
    );
    todo!("filled in by subsequent tasks")
}
