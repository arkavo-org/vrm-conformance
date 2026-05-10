//! Parametric VRM 1.0 asset generator. Emits paired
//! `<asset>.vrm + <asset>.meta.json + <asset>.test.yaml` from a single
//! parameter dictionary.

pub mod buffer;
pub mod cli;
pub mod emit;
pub mod glb;
pub mod humanoid;
pub mod mesh;
pub mod params;
pub mod sidecar;
pub mod spring_bone;
pub mod sweep;
pub mod vrm_ext;
