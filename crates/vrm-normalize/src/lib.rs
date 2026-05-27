//! v0 → v1 normalization for VRM dump responses.
//!
//! This crate is called by the **runner** (never by adapters) so that
//! four adapter implementations of normalization don't produce four bug
//! surfaces. Normalization is one-directional and lossy:
//!
//! - v0 → v1 has a documented preset mapping table (joy→happy, etc.).
//! - v1 → v0 has no lossless mapping and is rejected.
//! - v0 custom blendshapes are passed through with `custom:<name>` markers.
//!
//! See `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`.

pub mod expressions;
pub mod humanoid;
pub mod look_at;

use thiserror::Error;
use vrm_ops::SpecVersion;

#[derive(Debug, Error)]
pub enum NormalizeError {
    #[error("normalization direction unsupported: cannot project {from:?} dump as {to:?}")]
    DirectionUnsupported { from: SpecVersion, to: SpecVersion },
}
