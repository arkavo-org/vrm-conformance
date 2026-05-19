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
                local_rotation_quat: [
                    0.0,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                    std::f32::consts::FRAC_1_SQRT_2,
                ],
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
        gaze_direction_quat: [0.0, 0.2588, 0.0, 0.9659], // 30° yaw
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

#[test]
fn sequence_format_serializes_snake_case() {
    let png = SequenceFormat::PngSequence;
    let mp4 = SequenceFormat::Mp4;
    let mov = SequenceFormat::Mov;
    assert_eq!(serde_json::to_string(&png).unwrap(), r#""png_sequence""#);
    assert_eq!(serde_json::to_string(&mp4).unwrap(), r#""mp4""#);
    assert_eq!(serde_json::to_string(&mov).unwrap(), r#""mov""#);

    // Round-trip
    for fmt in [
        SequenceFormat::PngSequence,
        SequenceFormat::Mp4,
        SequenceFormat::Mov,
    ] {
        let s = serde_json::to_string(&fmt).unwrap();
        let back: SequenceFormat = serde_json::from_str(&s).unwrap();
        assert_eq!(back, fmt);
    }
}

#[test]
fn root_transform_animation_roundtrip() {
    let r = RootTransformAnimation {
        translation_start: [0.0, 0.0, 0.0],
        translation_end: [1.0, 0.0, 0.0],
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: RootTransformAnimation = serde_json::from_str(&s).unwrap();
    assert_eq!(back, r);
}

#[test]
fn vrma_playback_spec_roundtrip() {
    let v = VrmaPlaybackSpec {
        vrma_handle: 7,
        start_seconds: 0.25,
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: VrmaPlaybackSpec = serde_json::from_str(&s).unwrap();
    assert_eq!(back, v);
}

#[test]
fn sequence_frame_roundtrip() {
    let f = SequenceFrame {
        index: 12,
        timestamp_seconds: 0.4,
        path: "/tmp/seq/0012.png".into(),
        blake3: "blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    };
    let s = serde_json::to_string(&f).unwrap();
    let back: SequenceFrame = serde_json::from_str(&s).unwrap();
    assert_eq!(back, f);
}

#[test]
fn render_sequence_params_minimal_roundtrip() {
    let p = RenderSequenceParams {
        session_id: "sess-seq".into(),
        width: 512,
        height: 512,
        output_dir: "/tmp/seq".into(),
        frame_count: 60,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ColorSpace::Linear,
        msaa: 4,
        output_type: OutputType::Color,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: None,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: RenderSequenceParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.frame_count, 60);
    assert_eq!(back.frame_hz, 30.0);
    assert!(back.animate_root_transform.is_none());
    assert!(back.apply_vrma.is_none());
    // None variants must skip serialization to keep the wire format tight.
    assert!(!s.contains("animate_root_transform"));
    assert!(!s.contains("apply_vrma"));
}

#[test]
fn render_sequence_params_with_root_animation_roundtrip() {
    let p = RenderSequenceParams {
        session_id: "sess-seq".into(),
        width: 256,
        height: 256,
        output_dir: "/tmp/seq".into(),
        frame_count: 30,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ColorSpace::Srgb,
        msaa: 4,
        output_type: OutputType::Color,
        output_format: SequenceFormat::Mp4,
        animate_root_transform: Some(RootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [1.0, 0.0, 0.0],
        }),
        apply_vrma: None,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: RenderSequenceParams = serde_json::from_str(&s).unwrap();
    let anim = back.animate_root_transform.unwrap();
    assert_eq!(anim.translation_end[0], 1.0);
    assert!(s.contains(r#""output_format":"mp4""#));
}

#[test]
fn render_sequence_params_with_vrma_roundtrip() {
    let p = RenderSequenceParams {
        session_id: "sess-seq".into(),
        width: 256,
        height: 256,
        output_dir: "/tmp/seq".into(),
        frame_count: 60,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        color_space: ColorSpace::Linear,
        msaa: 1,
        output_type: OutputType::Color,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: None,
        apply_vrma: Some(VrmaPlaybackSpec {
            vrma_handle: 3,
            start_seconds: 0.5,
        }),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: RenderSequenceParams = serde_json::from_str(&s).unwrap();
    let v = back.apply_vrma.unwrap();
    assert_eq!(v.vrma_handle, 3);
    assert_eq!(v.start_seconds, 0.5);
}

#[test]
fn render_sequence_result_png_only_roundtrip() {
    let r = RenderSequenceResult {
        frames: vec![
            SequenceFrame {
                index: 0,
                timestamp_seconds: 0.0,
                path: "/tmp/seq/0000.png".into(),
                blake3: "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
            },
            SequenceFrame {
                index: 1,
                timestamp_seconds: 0.0333,
                path: "/tmp/seq/0001.png".into(),
                blake3: "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .into(),
            },
        ],
        duration_seconds: 2.0,
        actual_color_space: ColorSpace::Linear,
        frame_hz_achieved: 30.0,
        muxed_path: None,
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: RenderSequenceResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back, r);
    assert_eq!(back.frames.len(), 2);
    assert_eq!(back.frames[1].timestamp_seconds, 0.0333);
    assert_eq!(back.duration_seconds, 2.0);
    assert!(back.muxed_path.is_none());
    // None muxed_path must skip serialization
    assert!(!s.contains("muxed_path"));
}

#[test]
fn render_sequence_result_with_mux_roundtrip() {
    let r = RenderSequenceResult {
        frames: vec![], // empty intentional — checks Vec serialization on the wire
        duration_seconds: 2.0,
        actual_color_space: ColorSpace::Srgb,
        frame_hz_achieved: 29.97,
        muxed_path: Some("/tmp/seq/sequence.mp4".into()),
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: RenderSequenceResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back, r);
    assert_eq!(back.muxed_path.as_deref(), Some("/tmp/seq/sequence.mp4"));
    assert_eq!(back.frame_hz_achieved, 29.97);
    assert!(s.contains(r#""muxed_path":"/tmp/seq/sequence.mp4""#));
}
