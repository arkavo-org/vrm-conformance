//! Convert a `vrm_test_plan::TestPlan` into the per-op parameter values
//! the runner sends to the adapter.

use vrm_ops::tools as ops;
use vrm_test_plan as plan;

pub fn camera_params(session_id: &str, p: &plan::Camera) -> ops::SetCameraParams {
    ops::SetCameraParams {
        session_id: session_id.into(),
        position: p.position,
        target: p.target,
        up: p.up,
        fov_degrees: p.fov_degrees,
    }
}

pub fn lighting_params(session_id: &str, p: &plan::Lighting) -> ops::SetLightingParams {
    ops::SetLightingParams {
        session_id: session_id.into(),
        directional: ops::Directional {
            dir: p.directional.dir,
            color: p.directional.color,
            intensity: p.directional.intensity,
        },
        ambient: ops::Ambient {
            color: p.ambient.color,
            intensity: p.ambient.intensity,
        },
        cast_shadows: p.cast_shadows,
        receive_shadows: p.receive_shadows,
    }
}

pub fn post_processing_params(
    session_id: &str,
    p: &plan::PostProcessing,
) -> ops::SetPostProcessingParams {
    let tone_mapping = match p.tone_mapping {
        plan::ToneMapping::None => ops::ToneMapping::None,
        plan::ToneMapping::Linear => ops::ToneMapping::Linear,
        plan::ToneMapping::Reinhard => ops::ToneMapping::Reinhard,
        plan::ToneMapping::Aces => ops::ToneMapping::Aces,
    };
    ops::SetPostProcessingParams {
        session_id: session_id.into(),
        tone_mapping,
        exposure: p.exposure,
    }
}

pub fn animate_root_transform_params(
    session_id: &str,
    a: &plan::RootTransformAnimation,
) -> ops::AnimateRootTransformParams {
    ops::AnimateRootTransformParams {
        session_id: session_id.into(),
        translation_start: a.translation_start,
        translation_end: a.translation_end,
        duration_seconds: a.duration_seconds,
        fps: a.fps,
    }
}

pub fn render_params(session_id: &str, p: &plan::Output, output_path: String) -> ops::RenderParams {
    let color_space = match p.color_space {
        plan::ColorSpace::Linear => ops::ColorSpace::Linear,
        plan::ColorSpace::Srgb => ops::ColorSpace::Srgb,
    };
    ops::RenderParams {
        session_id: session_id.into(),
        width: p.width,
        height: p.height,
        output_path,
        color_space,
        msaa: p.msaa,
        output_type: ops::OutputType::Color,
    }
}

pub fn render_sequence_params(
    session_id: &str,
    output: &plan::Output,
    block: &plan::RenderSequenceBlock,
    output_dir: String,
) -> ops::RenderSequenceParams {
    let color_space = match output.color_space {
        plan::ColorSpace::Linear => ops::ColorSpace::Linear,
        plan::ColorSpace::Srgb => ops::ColorSpace::Srgb,
    };
    let output_format = match block.output_format {
        plan::SequenceFormat::PngSequence => ops::SequenceFormat::PngSequence,
        plan::SequenceFormat::Mp4 => ops::SequenceFormat::Mp4,
        plan::SequenceFormat::Mov => ops::SequenceFormat::Mov,
    };
    ops::RenderSequenceParams {
        session_id: session_id.into(),
        width: output.width,
        height: output.height,
        output_dir,
        frame_count: block.frame_count,
        frame_hz: block.frame_hz,
        physics_dt_seconds: block.physics_dt_seconds,
        color_space,
        msaa: output.msaa,
        output_type: ops::OutputType::Color,
        output_format,
        animate_root_transform: block.animate_root_transform.as_ref().map(|a| {
            ops::RootTransformAnimation {
                translation_start: a.translation_start,
                translation_end: a.translation_end,
            }
        }),
        apply_vrma: block.apply_vrma.as_ref().map(|v| ops::VrmaPlaybackSpec {
            vrma_handle: v.vrma_handle,
            start_seconds: v.start_seconds,
        }),
        capture_positions: block.capture_positions,
        capture_synthetic_colliders: block.capture_synthetic_colliders,
    }
}

pub fn load_vrma_params(
    asset_dir: &camino::Utf8Path,
    v: &plan::VrmaAnimation,
) -> ops::LoadVrmaParams {
    // Resolve relative paths against asset_dir; absolute paths pass through.
    let p = camino::Utf8PathBuf::from(&v.path);
    let resolved = if p.is_absolute() {
        p
    } else {
        asset_dir.join(p)
    };
    ops::LoadVrmaParams {
        vrma_path: resolved.to_string(),
    }
}

pub fn apply_vrma_at_time_params(
    session_id: &str,
    vrma_handle: u32,
    vrm_handle: u32,
    apply_at_time: f32,
) -> ops::ApplyVrmaAtTimeParams {
    ops::ApplyVrmaAtTimeParams {
        session_id: session_id.into(),
        vrma_handle,
        vrm_handle,
        time_seconds: apply_at_time,
    }
}

/// Map a plan's output block to `BenchmarkParams`. Mirrors `render_params`'
/// color-space mapping so the benchmarked scene matches the conformance
/// render. `animate` selects a small vertical root excitation so spring-bone
/// cost is exercised; otherwise the scene is static.
pub fn benchmark_params(
    session_id: &str,
    p: &plan::Output,
    warmup_frames: u32,
    measured_frames: u32,
    animate: bool,
) -> ops::BenchmarkParams {
    let color_space = match p.color_space {
        plan::ColorSpace::Linear => ops::ColorSpace::Linear,
        plan::ColorSpace::Srgb => ops::ColorSpace::Srgb,
    };
    ops::BenchmarkParams {
        session_id: session_id.into(),
        width: p.width,
        height: p.height,
        color_space,
        msaa: p.msaa,
        output_type: ops::OutputType::Color,
        warmup_frames,
        measured_frames,
        animate_root_transform: if animate {
            Some(ops::RootTransformAnimation {
                translation_start: [0.0, 0.0, 0.0],
                translation_end: [0.0, 0.1, 0.0],
            })
        } else {
            None
        },
    }
}

#[cfg(test)]
mod benchmark_params_tests {
    use super::*;

    fn sample_output() -> plan::Output {
        plan::Output {
            width: 256,
            height: 256,
            color_space: plan::ColorSpace::Linear,
            msaa: 4,
        }
    }

    #[test]
    fn benchmark_params_maps_output_and_frames() {
        let p = benchmark_params("sess-1", &sample_output(), 30, 300, false);
        assert_eq!(p.session_id, "sess-1");
        assert_eq!(p.width, 256);
        assert_eq!(p.height, 256);
        assert_eq!(p.msaa, 4);
        assert_eq!(p.color_space, ops::ColorSpace::Linear);
        assert_eq!(p.warmup_frames, 30);
        assert_eq!(p.measured_frames, 300);
        assert!(p.animate_root_transform.is_none());
    }

    #[test]
    fn benchmark_params_sets_animation_when_requested() {
        let p = benchmark_params("s", &sample_output(), 1, 1, true);
        assert!(p.animate_root_transform.is_some());
    }
}

#[cfg(test)]
mod synthetic_collider_threading_tests {
    use super::*;

    #[test]
    fn capture_synthetic_colliders_projects_into_params() {
        let output = plan::Output {
            width: 256,
            height: 256,
            color_space: plan::ColorSpace::Srgb,
            msaa: 1,
        };
        let block = plan::RenderSequenceBlock {
            frame_count: 10,
            frame_hz: 30.0,
            physics_dt_seconds: 0.016_666_668,
            output_format: plan::SequenceFormat::PngSequence,
            animate_root_transform: None,
            apply_vrma: None,
            temporal_ssim_threshold: None,
            capture_positions: false,
            capture_synthetic_colliders: true,
        };
        let p = render_sequence_params("s", &output, &block, "/tmp".into());
        assert!(p.capture_synthetic_colliders);
        // Sanity: capture_positions still projects independently.
        assert_eq!(p.capture_positions, block.capture_positions);
    }
}
