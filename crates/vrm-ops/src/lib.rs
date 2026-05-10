//! Operation catalog + JSON-RPC stdio transport. Source of truth for both
//! the structured CLI surface and the MCP wrapper.
//!
//! Spec: `docs/operation-contract.md`. Stdio framing follows LSP header convention
//! (`Content-Length: NNN\r\n\r\n` + body).

pub mod stdio;
pub mod tools;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<P> {
    pub jsonrpc: JsonRpcVersion,
    pub id: u64,
    pub method: String,
    pub params: P,
}

impl<P> JsonRpcRequest<P> {
    pub fn new(id: u64, method: impl Into<String>, params: P) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse<R> {
    pub jsonrpc: JsonRpcVersion,
    pub id: u64,
    #[serde(default = "none_option", skip_serializing_if = "Option::is_none")]
    pub result: Option<R>,
    #[serde(default = "none_option", skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

fn none_option<T>() -> Option<T> {
    None
}

impl<R> JsonRpcResponse<R> {
    pub fn ok(id: u64, result: R) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: u64, error: RpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn into_result(self) -> Result<R, RpcError> {
        match (self.result, self.error) {
            (Some(r), None) => Ok(r),
            (None, Some(e)) => Err(e),
            _ => Err(RpcError {
                code: -32700,
                message: "malformed response: missing both result and error".into(),
                data: None,
            }),
        }
    }
}

/// Marker that always serializes as `"2.0"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonRpcVersion;

impl Serialize for JsonRpcVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == "2.0" {
            Ok(JsonRpcVersion)
        } else {
            Err(serde::de::Error::custom(format!(
                "expected jsonrpc 2.0, got {s}"
            )))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[error("jsonrpc error {code}: {message}")]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    pub fn unimplemented(method: &str, phase: &str) -> Self {
        Self {
            code: -32000,
            message: format!("{method}: not implemented in this adapter version"),
            data: Some(serde_json::json!({ "phase": phase })),
        }
    }

    pub fn load_failed(report: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: "LoadFailed".into(),
            data: Some(serde_json::json!({ "validator_report": report.into() })),
        }
    }

    pub fn render_failed(reason: impl Into<String>) -> Self {
        Self {
            code: -32002,
            message: "RenderFailed".into(),
            data: Some(serde_json::json!({ "reason": reason.into() })),
        }
    }
}
