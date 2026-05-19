//! Per-op handlers. Each function returns either a typed result that the
//! dispatch loop will wrap in a successful JsonRpcResponse, or an RpcError
//! that becomes the response's `error` field.

use crate::render::{synthesize_frame, synthesize_png, FrameState};
use crate::session::{Session, SessionRegistry};
use camino::Utf8Path;
use vrm_ops::tools as ops;
use vrm_ops::RpcError;

pub fn load_vrm(
    registry: &mut SessionRegistry,
    params: ops::LoadVrmParams,
) -> Result<ops::LoadVrmResult, RpcError> {
    let path = Utf8Path::new(&params.path);
    let session = Session::load(path).map_err(|e| RpcError::load_failed(format!("{path}: {e}")))?;
    let session_id = registry.insert(session);
    Ok(ops::LoadVrmResult { session_id })
}

pub fn set_camera(
    registry: &mut SessionRegistry,
    params: ops::SetCameraParams,
) -> Result<ops::UnitResult, RpcError> {
    let session = registry
        .get_mut(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    session.camera = Some(params);
    Ok(ops::UnitResult {})
}

pub fn set_lighting(
    registry: &mut SessionRegistry,
    params: ops::SetLightingParams,
) -> Result<ops::UnitResult, RpcError> {
    let session = registry
        .get_mut(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    session.lighting = Some(params);
    Ok(ops::UnitResult {})
}

pub fn set_post_processing(
    registry: &mut SessionRegistry,
    params: ops::SetPostProcessingParams,
) -> Result<ops::UnitResult, RpcError> {
    let session = registry
        .get_mut(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    session.post_processing = Some(params);
    Ok(ops::UnitResult {})
}

pub fn render(
    registry: &mut SessionRegistry,
    params: ops::RenderParams,
) -> Result<ops::RenderResult, RpcError> {
    let session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    let img = synthesize_png(&session.params, params.width, params.height);
    if let Some(parent) = std::path::Path::new(&params.output_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| RpcError::render_failed(format!("create output dir: {e}")))?;
        }
    }
    img.save(&params.output_path)
        .map_err(|e| RpcError::render_failed(format!("save png {}: {e}", params.output_path)))?;
    // Mock declares whatever the caller asked for; pixel bytes are the same.
    Ok(ops::RenderResult {
        output_path: params.output_path,
        actual_color_space: params.color_space,
    })
}

pub fn dispose(
    registry: &mut SessionRegistry,
    params: ops::DisposeParams,
) -> Result<ops::UnitResult, RpcError> {
    registry.remove(&params.session_id);
    Ok(ops::UnitResult {})
}

/// Reserved ops all return Unimplemented at the dispatch site. This helper
/// produces the canonical envelope so phase labels stay consistent.
pub fn unimplemented(method: &str, phase: &str) -> RpcError {
    RpcError::unimplemented(method, phase)
}

/// Mock has no physics state. The deterministic synthesis is the same
/// before and after stepping; we just acknowledge the op so test plans
/// with a `physics:` block can run against the mock.
pub fn step_physics(
    _registry: &mut SessionRegistry,
    _params: serde_json::Value,
) -> Result<ops::UnitResult, RpcError> {
    Ok(ops::UnitResult {})
}

pub fn reset_physics(
    _registry: &mut SessionRegistry,
    _params: serde_json::Value,
) -> Result<ops::UnitResult, RpcError> {
    Ok(ops::UnitResult {})
}

/// Mock has no physics state; deterministic synthesis is unaffected by
/// root-transform animation. Acknowledge so plans carrying `animation`
/// blocks can run against the mock for protocol coverage.
pub fn animate_root_transform(
    _registry: &mut SessionRegistry,
    _params: serde_json::Value,
) -> Result<ops::UnitResult, RpcError> {
    Ok(ops::UnitResult {})
}

/// Mock has no spring-bone system. Returns an empty springs array for any
/// loaded session; unknown `session_id` returns -32602 InvalidParams.
pub fn dump_bone_positions(
    registry: &mut SessionRegistry,
    params: ops::DumpBonePositionsParams,
) -> Result<ops::DumpBonePositionsResult, RpcError> {
    let _session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    Ok(ops::DumpBonePositionsResult {
        springs: Vec::new(),
    })
}

pub fn load_vrma(
    _registry: &mut SessionRegistry,
    _params: ops::LoadVrmaParams,
) -> Result<ops::LoadVrmaResult, RpcError> {
    // Deterministic: every load yields the same handle + summary.
    Ok(ops::LoadVrmaResult {
        vrma_handle: 1,
        channel_summary: ops::VrmaChannelSummary {
            humanoid_bones: 15,
            expressions: 1,
            has_look_at: true,
            duration_seconds: 1.0,
        },
    })
}

pub fn apply_vrma_at_time(
    _registry: &mut SessionRegistry,
    _params: ops::ApplyVrmaAtTimeParams,
) -> Result<ops::ApplyVrmaAtTimeResult, RpcError> {
    Ok(ops::ApplyVrmaAtTimeResult {
        channels_applied: ops::VrmaChannelsApplied {
            humanoid_bones: 15,
            expressions: 1,
            look_at: true,
        },
    })
}

pub fn dump_humanoid_pose(
    _registry: &mut SessionRegistry,
    _params: ops::DumpHumanoidPoseParams,
) -> Result<ops::DumpHumanoidPoseResult, RpcError> {
    Ok(ops::DumpHumanoidPoseResult {
        bones: vec![
            ops::HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            },
            ops::HumanoidBoneRotation {
                name: "leftUpperArm".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        hips_translation: [0.0, 0.0, 0.0],
        bones_missing: vec![],
    })
}

pub fn dump_expression_weights(
    _registry: &mut SessionRegistry,
    _params: ops::DumpExpressionWeightsParams,
) -> Result<ops::DumpExpressionWeightsResult, RpcError> {
    let mut presets = std::collections::BTreeMap::new();
    presets.insert("happy".into(), 0.0_f32);
    Ok(ops::DumpExpressionWeightsResult {
        presets,
        custom: std::collections::BTreeMap::new(),
    })
}

pub fn dump_look_at_state(
    _registry: &mut SessionRegistry,
    _params: ops::DumpLookAtStateParams,
) -> Result<ops::DumpLookAtStateResult, RpcError> {
    Ok(ops::DumpLookAtStateResult {
        gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        applied_via: ops::LookAtAppliedVia::Off,
        offset_from_head_bone: [0.0, 0.0, 0.0],
    })
}

pub fn render_sequence(
    registry: &mut SessionRegistry,
    params: ops::RenderSequenceParams,
) -> Result<ops::RenderSequenceResult, RpcError> {
    if params.animate_root_transform.is_some() && params.apply_vrma.is_some() {
        return Err(RpcError {
            code: -32602,
            message:
                "render_sequence: animate_root_transform and apply_vrma are mutually exclusive"
                    .into(),
            data: None,
        });
    }
    if params.physics_dt_seconds > 1.0 / 60.0 + 1e-9 {
        return Err(RpcError {
            code: -32602,
            message: format!(
                "render_sequence: physics_dt_seconds {} exceeds 60 Hz floor (1/60 ≈ 0.01667)",
                params.physics_dt_seconds
            ),
            data: None,
        });
    }

    let session = registry
        .get(&params.session_id)
        .ok_or_else(|| invalid_session(&params.session_id))?;
    let session_params = session.params.clone();

    let root_anim = params.animate_root_transform.clone();
    let vrma_spec = params.apply_vrma.clone();

    let output_dir = camino::Utf8PathBuf::from(&params.output_dir);
    std::fs::create_dir_all(output_dir.as_std_path())
        .map_err(|e| RpcError::render_failed(format!("create output_dir: {e}")))?;

    let mut frames = Vec::with_capacity(params.frame_count as usize);
    for i in 0..params.frame_count {
        let t = if params.frame_count <= 1 {
            0.0
        } else {
            i as f32 / (params.frame_count - 1) as f32
        };

        let root_translation = root_anim.as_ref().map_or([0.0, 0.0, 0.0], |a| {
            lerp3(a.translation_start, a.translation_end, t)
        });

        let vrma_time = vrma_spec
            .as_ref()
            .map(|v| v.start_seconds + (i as f32) / params.frame_hz);

        let state = FrameState {
            index: i,
            frame_count: params.frame_count,
            root_translation,
            vrma_time_seconds: vrma_time,
        };

        let img = synthesize_frame(&session_params, state, params.width, params.height);
        let frame_path = output_dir.join(format!("{i:04}.png"));
        img.save(frame_path.as_std_path())
            .map_err(|e| RpcError::render_failed(format!("save frame {i}: {e}")))?;

        let bytes = std::fs::read(frame_path.as_std_path())
            .map_err(|e| RpcError::render_failed(format!("read frame {i} for blake3: {e}")))?;
        let hash = blake3::hash(&bytes);
        let blake3_hex = format!("blake3:{}", hash.to_hex());

        frames.push(ops::SequenceFrame {
            index: i,
            timestamp_seconds: (i as f32) / params.frame_hz,
            path: frame_path.to_string(),
            blake3: blake3_hex,
        });
    }

    let duration_seconds = if params.frame_hz > 0.0 {
        params.frame_count as f32 / params.frame_hz
    } else {
        0.0
    };

    let muxed_path = match params.output_format {
        ops::SequenceFormat::PngSequence => None,
        ops::SequenceFormat::Mp4 => mux_via_ffmpeg(
            output_dir.as_path(),
            "sequence.mp4",
            &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "0"],
            params.frame_hz,
        )?,
        ops::SequenceFormat::Mov => mux_via_ffmpeg(
            output_dir.as_path(),
            "sequence.mov",
            &["-c:v", "prores_ks", "-profile:v", "4444"],
            params.frame_hz,
        )?,
    };

    Ok(ops::RenderSequenceResult {
        frames,
        duration_seconds,
        actual_color_space: params.color_space,
        frame_hz_achieved: params.frame_hz,
        muxed_path,
    })
}

/// Shell out to ffmpeg to mux per-frame PNGs into a single video file.
/// Returns `Ok(Some(path))` on success, `Ok(None)` when ffmpeg is absent
/// (soft-skip — PNG sequence is still useful on its own), or an `Err`
/// when ffmpeg is present but muxing failed.
fn mux_via_ffmpeg(
    output_dir: &camino::Utf8Path,
    out_file: &str,
    codec_args: &[&str],
    frame_hz: f32,
) -> Result<Option<String>, RpcError> {
    // Probe: is ffmpeg on PATH?
    let probe = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output();
    if probe.is_err() {
        tracing::warn!(
            "ffmpeg not found on PATH; muxed output {} skipped (PNG sequence is still written)",
            out_file
        );
        return Ok(None);
    }

    let mux_path = output_dir.join(out_file);
    let frame_pattern = output_dir.join("%04d.png");
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-y") // overwrite
        .arg("-framerate")
        .arg(format!("{frame_hz}"))
        .arg("-i")
        .arg(frame_pattern.as_std_path())
        .args(codec_args)
        .arg(mux_path.as_std_path());

    let out = cmd
        .output()
        .map_err(|e| RpcError::render_failed(format!("ffmpeg invocation failed: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(RpcError::render_failed(format!(
            "ffmpeg muxing {} failed (exit {}): {}",
            out_file,
            out.status.code().unwrap_or(-1),
            stderr.lines().rev().take(5).collect::<Vec<_>>().join(" / "),
        )));
    }

    Ok(Some(mux_path.to_string()))
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn invalid_session(id: &str) -> RpcError {
    RpcError {
        code: -32602,
        message: format!("invalid session_id: {id}"),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use vrm_asset_generator::params::MToonParams;
    use vrm_ops::tools::{DumpBonePositionsParams, DumpBonePositionsResult};

    fn make_test_session() -> (SessionRegistry, String) {
        let mut registry = SessionRegistry::new();
        let session = Session {
            asset_path: Utf8PathBuf::from("test.vrm"),
            params: MToonParams::defaults("test"),
            camera: None,
            lighting: None,
            post_processing: None,
        };
        let id = registry.insert(session);
        (registry, id)
    }

    #[test]
    fn dump_positions_on_unknown_session_returns_invalid_params() {
        let mut registry = SessionRegistry::default();
        let params = DumpBonePositionsParams {
            session_id: "nope".into(),
            spring_index: None,
        };
        let err = dump_bone_positions(&mut registry, params).unwrap_err();
        assert_eq!(err.code, -32602, "expected InvalidParams, got {err:?}");
    }

    #[test]
    fn dump_positions_returns_empty_springs_for_loaded_session() {
        let (mut registry, session_id) = make_test_session();
        let params = DumpBonePositionsParams {
            session_id,
            spring_index: None,
        };
        let result: DumpBonePositionsResult = dump_bone_positions(&mut registry, params).unwrap();
        assert_eq!(
            result.springs.len(),
            0,
            "mock has no springs; expected empty result"
        );
    }
}
