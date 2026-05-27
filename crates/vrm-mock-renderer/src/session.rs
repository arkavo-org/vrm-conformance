//! Per-load_vrm session state. `Session` holds the parameters extracted
//! from the asset's `.meta.json` sidecar plus the most-recent camera /
//! lighting / post values the runner has set on it. `SessionRegistry`
//! owns sessions for the lifetime of the adapter process.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use vrm_asset_generator::params::MToonParams;
use vrm_ops::SpecVersion;

#[derive(Debug, Clone)]
pub struct Session {
    pub asset_path: Utf8PathBuf,
    pub params: MToonParams,
    /// Detected from `extensionsUsed` in the .vrm glTF JSON chunk at load
    /// time. Falls back to `V1` when no recognised VRM extension is present.
    pub source_spec_version: SpecVersion,
    pub camera: Option<vrm_ops::tools::SetCameraParams>,
    pub lighting: Option<vrm_ops::tools::SetLightingParams>,
    pub post_processing: Option<vrm_ops::tools::SetPostProcessingParams>,
}

impl Session {
    pub fn load(asset_path: &Utf8Path) -> Result<Self> {
        let meta_path = asset_path.with_extension("meta.json");
        let meta_bytes = std::fs::read(meta_path.as_std_path())
            .with_context(|| format!("read meta sidecar: {meta_path}"))?;
        // The sidecar shape is `{"id":..., "params": MToonParams, ...}`.
        // We only need params; pull it out with a small envelope.
        #[derive(serde::Deserialize)]
        struct Sidecar {
            params: MToonParams,
        }
        let sidecar: Sidecar = serde_json::from_slice(&meta_bytes)
            .with_context(|| format!("parse meta sidecar: {meta_path}"))?;

        // Detect spec version by inspecting `extensionsUsed` in the GLB JSON
        // chunk.  Reading only the JSON chunk is cheap — no mesh data parsed.
        let source_spec_version = detect_spec_version_from_file(asset_path);

        Ok(Self {
            asset_path: asset_path.to_owned(),
            params: sidecar.params,
            source_spec_version,
            camera: None,
            lighting: None,
            post_processing: None,
        })
    }
}

/// Read the .vrm file, extract the glTF JSON chunk, and infer the VRM spec
/// version from `extensionsUsed`.  Returns `V1` on any I/O or parse error
/// (preserves the pre-Task-31 default behaviour so existing tests are
/// unaffected).
fn detect_spec_version_from_file(path: &Utf8Path) -> SpecVersion {
    let bytes = match std::fs::read(path.as_std_path()) {
        Ok(b) => b,
        Err(_) => return SpecVersion::V1,
    };
    let json_bytes = match vrm_asset_generator::glb::extract_json_chunk(&bytes) {
        Some(b) => b,
        None => return SpecVersion::V1,
    };
    let json: serde_json::Value = match serde_json::from_slice(&json_bytes) {
        Ok(v) => v,
        Err(_) => return SpecVersion::V1,
    };
    detect_spec_version(&json)
}

/// Pure detection: inspect `extensionsUsed` in a parsed glTF JSON document.
///
/// - `"VRMC_vrm"` → `SpecVersion::V1`  (VRM 1.0)
/// - `"VRM"`      → `SpecVersion::V0`  (VRM 0.x)
/// - anything else / missing → fallback `SpecVersion::V1`
pub fn detect_spec_version(gltf_json: &serde_json::Value) -> SpecVersion {
    let used = gltf_json["extensionsUsed"].as_array();
    if let Some(arr) = used {
        for v in arr {
            if let Some(s) = v.as_str() {
                if s == "VRMC_vrm" {
                    return SpecVersion::V1;
                }
                if s == "VRM" {
                    return SpecVersion::V0;
                }
            }
        }
    }
    // Fallback — no recognised VRM extension found.  Default to V1 to
    // preserve legacy behaviour (all corpus assets today are V1).
    SpecVersion::V1
}

#[cfg(test)]
mod source_spec_version_tests {
    use super::*;

    #[test]
    fn detects_v0_from_inline_json() {
        let json = serde_json::json!({ "extensionsUsed": ["VRM"] });
        assert_eq!(detect_spec_version(&json), SpecVersion::V0);
    }

    #[test]
    fn detects_v1_from_inline_json() {
        let json = serde_json::json!({ "extensionsUsed": ["VRMC_vrm"] });
        assert_eq!(detect_spec_version(&json), SpecVersion::V1);
    }

    #[test]
    fn defaults_to_v1_when_extensions_used_missing() {
        let json = serde_json::json!({});
        assert_eq!(detect_spec_version(&json), SpecVersion::V1);
    }

    #[test]
    fn defaults_to_v1_when_extensions_used_empty() {
        let json = serde_json::json!({ "extensionsUsed": [] });
        assert_eq!(detect_spec_version(&json), SpecVersion::V1);
    }

    #[test]
    fn v1_wins_when_both_extensions_present_and_vrmc_vrm_first() {
        // Defensive: if both markers appear (malformed asset), first-match wins.
        let json = serde_json::json!({ "extensionsUsed": ["VRMC_vrm", "VRM"] });
        assert_eq!(detect_spec_version(&json), SpecVersion::V1);
    }

    #[test]
    fn v0_wins_when_vrm_marker_is_first() {
        let json = serde_json::json!({ "extensionsUsed": ["VRM", "VRMC_vrm"] });
        assert_eq!(detect_spec_version(&json), SpecVersion::V0);
    }

    #[test]
    fn ignores_unrelated_extensions() {
        let json = serde_json::json!({
            "extensionsUsed": ["KHR_materials_unlit", "VRMC_materials_mtoon"]
        });
        assert_eq!(detect_spec_version(&json), SpecVersion::V1);
    }

    /// Round-trip test: emit a real VRM 1.0 GLB, run `detect_spec_version_from_file`
    /// on it, and confirm we get `V1` back.  Uses `emit_vrm` from
    /// `vrm-asset-generator` (already a dev dep of this crate).
    #[test]
    fn detect_from_generated_v1_vrm_file() {
        use vrm_asset_generator::{emit::emit_vrm, params::MToonParams};

        let dir = tempfile::tempdir().expect("tempdir");
        let vrm_path =
            camino::Utf8PathBuf::from_path_buf(dir.path().join("roundtrip.vrm")).unwrap();
        emit_vrm(&MToonParams::defaults("roundtrip"), &vrm_path).expect("emit_vrm");

        let version = detect_spec_version_from_file(&vrm_path);
        assert_eq!(
            version,
            SpecVersion::V1,
            "generated VRM 1.0 should detect as V1"
        );
    }

    /// `detect_spec_version_from_file` on a path that does not exist should
    /// fall back gracefully to `V1` rather than panicking.
    #[test]
    fn fallback_on_missing_file() {
        let version = detect_spec_version_from_file(camino::Utf8Path::new(
            "/tmp/this_file_does_not_exist_vrm_conformance_test.vrm",
        ));
        assert_eq!(version, SpecVersion::V1);
    }

    /// `detect_spec_version_from_file` on a file with an invalid GLB header
    /// should fall back gracefully to `V1`.
    #[test]
    fn fallback_on_invalid_glb() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("garbage.vrm");
        std::fs::write(&path, b"not a glb file").expect("write");
        let utf8_path = camino::Utf8PathBuf::from_path_buf(path).expect("utf8 path");
        let version = detect_spec_version_from_file(&utf8_path);
        assert_eq!(version, SpecVersion::V1);
    }
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<String, Session>,
    next_id: u64,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, session: Session) -> String {
        self.next_id += 1;
        let id = format!("mock-{}", self.next_id);
        self.sessions.insert(id.clone(), session);
        id
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id)
    }
}
