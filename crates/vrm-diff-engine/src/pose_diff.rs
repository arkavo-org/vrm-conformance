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
    // Per-bone rotation: quaternion geodesic distance, sign-invariant.
    // Bones in either bones_missing list are skipped — methodology
    // hazard #3 (a renderer that doesn't expose the bone shouldn't
    // contribute to diff).
    let missing_actual: std::collections::HashSet<&String> =
        actual_humanoid.bones_missing.iter().collect();
    let missing_reference: std::collections::HashSet<&String> =
        reference_humanoid.bones_missing.iter().collect();

    let reference_by_name: std::collections::HashMap<&String, &[f32; 4]> = reference_humanoid
        .bones
        .iter()
        .map(|b| (&b.name, &b.local_rotation_quat))
        .collect();

    let mut per_bone_max = 0.0_f32;
    let mut worst_bone: Option<String> = None;

    for bone in &actual_humanoid.bones {
        if missing_actual.contains(&bone.name) || missing_reference.contains(&bone.name) {
            continue;
        }
        let Some(ref_quat) = reference_by_name.get(&bone.name) else {
            // The reference doesn't have this bone — treat as excluded.
            continue;
        };
        let geodesic = quaternion_geodesic_rad(&bone.local_rotation_quat, ref_quat);
        if geodesic > per_bone_max {
            per_bone_max = geodesic;
            worst_bone = Some(bone.name.clone());
        }
    }

    // Hips translation: Euclidean distance.
    let hips = euclidean_distance(
        &actual_humanoid.hips_translation,
        &reference_humanoid.hips_translation,
    );

    // Expressions + lookAt stay at zero defaults until tasks 3 and 4.
    let _ = (
        actual_expressions,
        reference_expressions,
        actual_look_at,
        reference_look_at,
    );

    let humanoid_pass =
        per_bone_max <= tolerances.per_bone_quaternion_radians
            && hips <= tolerances.hips_translation_m;

    PoseDiffReport {
        per_bone_rotation_max_rad: per_bone_max,
        per_bone_rotation_worst_bone: worst_bone,
        hips_translation_m: hips,
        per_preset_expression_max_delta: 0.0,
        per_preset_expression_worst: None,
        per_custom_expression_max_delta: 0.0,
        per_custom_expression_worst: None,
        look_at_yaw_delta_deg: 0.0,
        look_at_pitch_delta_deg: 0.0,
        offset_from_head_bone_m: 0.0,
        overall_passed: humanoid_pass,
    }
}

/// Geodesic distance between two quaternions: `2·acos(|dot|)`. Sign-
/// invariant — `q` and `-q` yield identical results (same orientation).
fn quaternion_geodesic_rad(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    // Clamp into [-1, 1] to absorb float rounding above the legal range.
    let clamped = dot.abs().min(1.0);
    2.0 * clamped.acos()
}

fn euclidean_distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vrm_ops::tools::{HumanoidBoneRotation, LookAtAppliedVia};

    fn empty_expressions() -> DumpExpressionWeightsResult {
        DumpExpressionWeightsResult {
            presets: std::collections::BTreeMap::new(),
            custom: std::collections::BTreeMap::new(),
        }
    }

    fn identity_look_at() -> DumpLookAtStateResult {
        DumpLookAtStateResult {
            gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            applied_via: LookAtAppliedVia::Off,
            offset_from_head_bone: [0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn identical_pose_passes() {
        let pose = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "leftUpperArm".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &pose,
            &pose,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(report.overall_passed);
        assert_eq!(report.per_bone_rotation_max_rad, 0.0);
        assert_eq!(report.hips_translation_m, 0.0);
    }

    #[test]
    fn quaternion_geodesic_is_sign_invariant() {
        let p_pos = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let p_neg = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, -1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &p_pos,
            &p_neg,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(
            report.per_bone_rotation_max_rad < 1e-5,
            "expected ~0 (sign-invariant), got {}",
            report.per_bone_rotation_max_rad
        );
    }

    #[test]
    fn hips_translation_fails_outside_tolerance() {
        let pose_a = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let pose_b = DumpHumanoidPoseResult {
            bones: vec![],
            hips_translation: [0.020, 0.0, 0.0],  // 20mm — exceeds 5mm default
            bones_missing: vec![],
        };
        let report = diff_pose(
            &pose_a,
            &pose_b,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(!report.overall_passed);
        assert!((report.hips_translation_m - 0.020).abs() < 1e-5);
    }

    #[test]
    fn missing_bones_excluded_from_diff() {
        // Actual side declares leftThumbDistal as missing — that bone
        // should not contribute to per-bone diff even if it appears in
        // the reference.
        let actual = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec!["leftThumbDistal".into()],
        };
        let reference = DumpHumanoidPoseResult {
            bones: vec![HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            }],
            hips_translation: [0.0, 0.0, 0.0],
            bones_missing: vec![],
        };
        let report = diff_pose(
            &actual,
            &reference,
            &empty_expressions(),
            &empty_expressions(),
            &identity_look_at(),
            &identity_look_at(),
            &PoseTolerances::default(),
        );
        assert!(report.overall_passed);
    }
}
