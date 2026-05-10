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
