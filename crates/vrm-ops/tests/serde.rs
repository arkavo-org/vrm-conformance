use vrm_ops::{tools::*, JsonRpcRequest, JsonRpcResponse, RpcError};

#[test]
fn load_vrm_request_serializes() {
    let req = JsonRpcRequest::new(
        1,
        "load_vrm",
        LoadVrmParams {
            path: "/tmp/test.vrm".into(),
        },
    );
    let s = serde_json::to_string(&req).unwrap();
    assert!(s.contains(r#""method":"load_vrm""#));
    assert!(s.contains(r#""path":"/tmp/test.vrm""#));
    assert!(s.contains(r#""jsonrpc":"2.0""#));
}

#[test]
fn render_response_deserializes() {
    let raw = r#"{
        "jsonrpc": "2.0",
        "id": 7,
        "result": {
            "output_path": "/tmp/out.png",
            "actual_color_space": "Linear"
        }
    }"#;
    let resp: JsonRpcResponse<RenderResult> = serde_json::from_str(raw).unwrap();
    let result = resp.into_result().unwrap();
    assert_eq!(result.output_path, "/tmp/out.png");
    assert!(matches!(result.actual_color_space, ColorSpace::Linear));
}

#[test]
fn animate_root_transform_round_trips() {
    let params = AnimateRootTransformParams {
        session_id: "sess-7".into(),
        translation_start: [0.0, 0.0, 0.0],
        translation_end: [0.2, 0.0, 0.0],
        duration_seconds: 0.5,
        fps: 60,
    };
    let req = JsonRpcRequest::new(2, "animate_root_transform", params);
    let s = serde_json::to_string(&req).unwrap();
    assert!(s.contains(r#""method":"animate_root_transform""#));
    assert!(s.contains(r#""duration_seconds":0.5"#));
    assert!(s.contains(r#""fps":60"#));

    let req2: JsonRpcRequest<AnimateRootTransformParams> = serde_json::from_str(&s).unwrap();
    assert_eq!(req2.method, "animate_root_transform");
    assert_eq!(req2.params.translation_end, [0.2, 0.0, 0.0]);
    assert_eq!(req2.params.fps, 60);
}

#[test]
fn unimplemented_error_round_trips() {
    let err = RpcError::unimplemented("step_physics", "v1.x");
    let s = serde_json::to_string(&err).unwrap();
    let parsed: RpcError = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed.code, -32000);
    assert_eq!(parsed.data.unwrap()["phase"].as_str().unwrap(), "v1.x");
}

#[test]
fn load_vrma_params_roundtrip() {
    let p = LoadVrmaParams {
        vrma_path: "/tmp/test.vrma".into(),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: LoadVrmaParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_path, "/tmp/test.vrma");
    assert!(s.contains(r#""vrma_path":"/tmp/test.vrma""#));
}

#[test]
fn load_vrma_result_roundtrip() {
    let r = LoadVrmaResult {
        vrma_handle: 42,
        channel_summary: VrmaChannelSummary {
            humanoid_bones: 15,
            expressions: 3,
            has_look_at: true,
            duration_seconds: 1.0,
        },
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: LoadVrmaResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_handle, 42);
    assert_eq!(back.channel_summary.humanoid_bones, 15);
    assert!(back.channel_summary.has_look_at);
}

#[test]
fn apply_vrma_at_time_params_roundtrip() {
    let p = ApplyVrmaAtTimeParams {
        session_id: "sess-vrma".into(),
        vrma_handle: 7,
        vrm_handle: 3,
        time_seconds: 0.5,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: ApplyVrmaAtTimeParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_handle, 7);
    assert_eq!(back.time_seconds, 0.5);
}

#[test]
fn apply_vrma_at_time_result_roundtrip() {
    let r = ApplyVrmaAtTimeResult {
        channels_applied: VrmaChannelsApplied {
            humanoid_bones: 12,
            expressions: 2,
            look_at: false,
        },
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: ApplyVrmaAtTimeResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.channels_applied.humanoid_bones, 12);
    assert!(!back.channels_applied.look_at);
}

#[test]
fn dump_humanoid_pose_result_roundtrip() {
    let r = DumpHumanoidPoseResult {
        bones: vec![
            HumanoidBoneRotation {
                name: "leftUpperArm".into(),
                local_rotation_quat: [0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2],
            },
            HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        hips_translation: [0.0, 0.05, 0.0],
        bones_missing: vec!["leftThumbDistal".into()],
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: DumpHumanoidPoseResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.bones.len(), 2);
    assert_eq!(back.bones[0].name, "leftUpperArm");
    assert_eq!(back.hips_translation[1], 0.05);
    assert_eq!(back.bones_missing[0], "leftThumbDistal");
}

#[test]
fn dump_expression_weights_result_roundtrip() {
    let mut presets = std::collections::BTreeMap::new();
    presets.insert("happy".to_string(), 0.83_f32);
    presets.insert("blink".to_string(), 0.02_f32);
    let mut custom = std::collections::BTreeMap::new();
    custom.insert("smug".to_string(), 0.5_f32);
    let r = DumpExpressionWeightsResult { presets, custom };
    let s = serde_json::to_string(&r).unwrap();
    let back: DumpExpressionWeightsResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.presets.get("happy"), Some(&0.83));
    assert_eq!(back.custom.get("smug"), Some(&0.5));
}

#[test]
fn dump_look_at_state_result_roundtrip() {
    let r = DumpLookAtStateResult {
        gaze_direction_quat: [0.0, 0.2588, 0.0, 0.9659],  // 30° yaw
        yaw_deg: 30.0,
        pitch_deg: 0.0,
        applied_via: LookAtAppliedVia::Bone,
        offset_from_head_bone: [0.0, 0.06, 0.0],
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: DumpLookAtStateResult = serde_json::from_str(&s).unwrap();
    assert!(matches!(back.applied_via, LookAtAppliedVia::Bone));
    assert_eq!(back.yaw_deg, 30.0);

    // Variant serialization sanity
    assert!(s.contains(r#""applied_via":"bone""#));
}

#[test]
fn dump_look_at_state_applied_via_off_serializes() {
    let r = DumpLookAtStateResult {
        gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        applied_via: LookAtAppliedVia::Off,
        offset_from_head_bone: [0.0, 0.0, 0.0],
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""applied_via":"off""#));
}
