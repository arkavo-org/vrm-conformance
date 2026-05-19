//! vrm-mock-renderer entrypoint: a stdio JSON-RPC server that satisfies the
//! Phase 1 op contract using `vrm_mock_renderer::handlers`. Reserved Phase
//! 2+ ops return `Unimplemented` with the appropriate phase label.

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::io::{stdin, stdout, BufReader, Write};
use vrm_mock_renderer::{handlers, session::SessionRegistry};
use vrm_ops::stdio::{read_message, write_message};
use vrm_ops::RpcError;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    tracing::info!("vrm-mock-renderer starting");

    let mut registry = SessionRegistry::new();
    let stdin = stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = stdout();
    let mut writer = stdout.lock();

    loop {
        let body = match read_message(&mut reader) {
            Ok(b) => b,
            Err(e) => {
                // EOF or framing error → exit cleanly.
                tracing::info!("stdin closed: {e}");
                return Ok(());
            }
        };

        let req: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("malformed request: {e}");
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}"),
                    }
                });
                write_framed(&mut writer, &resp)?;
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        tracing::debug!(method, id = ?id, "request");

        let result = dispatch(&mut registry, &method, params);

        let resp = match result {
            Ok(v) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": v,
            }),
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": e,
            }),
        };
        write_framed(&mut writer, &resp)?;
    }
}

fn dispatch(
    registry: &mut SessionRegistry,
    method: &str,
    params: Value,
) -> Result<Value, RpcError> {
    match method {
        "load_vrm" => json_result(handlers::load_vrm(registry, deser(params)?)),
        "set_camera" => json_result(handlers::set_camera(registry, deser(params)?)),
        "set_lighting" => json_result(handlers::set_lighting(registry, deser(params)?)),
        "set_post_processing" => {
            json_result(handlers::set_post_processing(registry, deser(params)?))
        }
        "render" => json_result(handlers::render(registry, deser(params)?)),
        "render_sequence" => json_result(handlers::render_sequence(registry, deser(params)?)),
        "dispose" => json_result(handlers::dispose(registry, deser(params)?)),
        "step_physics" => json_result(handlers::step_physics(registry, params)),
        "reset_physics" => json_result(handlers::reset_physics(registry, params)),
        "animate_root_transform" => json_result(handlers::animate_root_transform(registry, params)),
        "dump_bone_positions" => {
            json_result(handlers::dump_bone_positions(registry, deser(params)?))
        }

        "load_vrma" => json_result(handlers::load_vrma(registry, deser(params)?)),
        "apply_vrma_at_time" => json_result(handlers::apply_vrma_at_time(registry, deser(params)?)),
        "dump_humanoid_pose" => json_result(handlers::dump_humanoid_pose(registry, deser(params)?)),
        "dump_expression_weights" => {
            json_result(handlers::dump_expression_weights(registry, deser(params)?))
        }
        "dump_look_at_state" => json_result(handlers::dump_look_at_state(registry, deser(params)?)),

        // Reserved-but-declared ops: return Unimplemented with the phase
        // the operation belongs to. Matches the Swift adapter's labels.
        "set_environment" => Err(handlers::unimplemented(method, "v1.x")),
        "set_expression" => Err(handlers::unimplemented(method, "Phase 3")),
        "set_humanoid_pose" | "set_root_transform" => {
            Err(handlers::unimplemented(method, "Phase 2"))
        }
        _ => Err(RpcError {
            code: -32601,
            message: format!("method not found: {method}"),
            data: None,
        }),
    }
}

fn deser<P: DeserializeOwned>(v: Value) -> Result<P, RpcError> {
    serde_json::from_value(v).map_err(|e| RpcError {
        code: -32602,
        message: format!("invalid params: {e}"),
        data: None,
    })
}

fn json_result<R: serde::Serialize>(r: Result<R, RpcError>) -> Result<Value, RpcError> {
    r.and_then(|v| {
        serde_json::to_value(v).map_err(|e| RpcError {
            code: -32603,
            message: format!("serialize result: {e}"),
            data: None,
        })
    })
}

fn write_framed<W: Write>(w: &mut W, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write_message(w, &body)?;
    Ok(())
}
