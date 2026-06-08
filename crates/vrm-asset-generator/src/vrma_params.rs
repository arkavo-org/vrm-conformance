//! Parameter dictionaries for VRMA sweep emission. Each sweep variant
//! holds one of these; the variant id becomes the file stem.

use serde::{Deserialize, Serialize};

/// Single-bone rotation sweep. Rotates `bone_name` about the named axis
/// from 0° at t=0 to `angle_deg` at t=`duration_s`, linear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaHumanoidParams {
    pub id: String,
    pub bone_name: String,
    pub axis: RotationAxis,
    pub angle_deg: f32,
    pub duration_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RotationAxis {
    X,
    Y,
    Z,
}

/// Single-expression weight ramp: 0 → 1 → 0 over `duration_s`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaExpressionParams {
    pub id: String,
    pub expression_name: String,
    pub is_preset: bool,
    pub duration_s: f32,
}

/// LookAt direction sweep: rotate gaze about yaw (Y) or pitch (X) axis
/// to the given angle. Avatar config controls whether the .vrm renders
/// gaze via bone or expression path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaLookAtParams {
    pub id: String,
    pub axis: RotationAxis,
    pub angle_deg: f32,
    pub avatar_lookat_type: AvatarLookAtType,
    pub duration_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AvatarLookAtType {
    /// Avatar `VRMC_vrm.lookAt.type: bone` — gaze rotates head/eye bones.
    Bone,
    /// Avatar `VRMC_vrm.lookAt.type: expression` — gaze drives lookUp/
    /// lookDown/lookLeft/lookRight preset expressions.
    Expression,
}

/// Real-avatar gaze sweep variant. Drives a single-axis gaze direction
/// (yaw OR pitch) plus an optional spine yaw (the turned-head case). The
/// .vrma is avatar-agnostic; manual plans pair it with `vroid_default_F_1_0.vrm`.
/// Covers VMK 0.17.1 #332: `body_yaw_deg != 0` exercises head-local gaze
/// resolution; `gaze_angle_deg == 0` at body 0 exercises eye-rest composition
/// (wall-eye at center).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GazeParams {
    pub id: String,
    /// Gaze rotation axis: `Y` = yaw (left/right), `X` = pitch (up/down).
    pub gaze_axis: RotationAxis,
    /// Gaze angle in degrees about `gaze_axis`. 0 = straight ahead.
    pub gaze_angle_deg: f32,
    /// Spine yaw (about Y) in degrees. 0 = upright. Non-zero turns the head.
    pub body_yaw_deg: f32,
    pub duration_s: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanoid_params_roundtrips() {
        let p = VrmaHumanoidParams {
            id: "vrma_humanoid_leftUpperArm_yaw_30".into(),
            bone_name: "leftUpperArm".into(),
            axis: RotationAxis::Y,
            angle_deg: 30.0,
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VrmaHumanoidParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.bone_name, "leftUpperArm");
        assert_eq!(back.axis, RotationAxis::Y);
    }

    #[test]
    fn expression_params_roundtrips() {
        let p = VrmaExpressionParams {
            id: "vrma_expression_preset_happy".into(),
            expression_name: "happy".into(),
            is_preset: true,
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VrmaExpressionParams = serde_json::from_str(&s).unwrap();
        assert!(back.is_preset);
        assert_eq!(back.expression_name, "happy");
    }

    #[test]
    fn lookat_params_roundtrips_with_avatar_config() {
        let p = VrmaLookAtParams {
            id: "vrma_lookat_yaw_neg60_bone".into(),
            axis: RotationAxis::Y,
            angle_deg: -60.0,
            avatar_lookat_type: AvatarLookAtType::Bone,
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VrmaLookAtParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.avatar_lookat_type, AvatarLookAtType::Bone);
        assert!((back.angle_deg - (-60.0)).abs() < 1e-5);
    }

    #[test]
    fn gaze_params_roundtrips() {
        let p = GazeParams {
            id: "gaze_center_bodyL".into(),
            gaze_axis: RotationAxis::Y,
            gaze_angle_deg: 0.0,
            body_yaw_deg: 35.0,
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: GazeParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, "gaze_center_bodyL");
        assert_eq!(back.body_yaw_deg, 35.0);
        assert!(matches!(back.gaze_axis, RotationAxis::Y));
    }
}
