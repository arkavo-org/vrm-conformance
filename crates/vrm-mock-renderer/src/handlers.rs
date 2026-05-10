//! Per-op handlers. Each function returns either a typed result that the
//! dispatch loop will wrap in a successful JsonRpcResponse, or an RpcError
//! that becomes the response's `error` field.

use crate::render::synthesize_png;
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

fn invalid_session(id: &str) -> RpcError {
    RpcError {
        code: -32602,
        message: format!("invalid session_id: {id}"),
        data: None,
    }
}
