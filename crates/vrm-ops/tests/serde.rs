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
