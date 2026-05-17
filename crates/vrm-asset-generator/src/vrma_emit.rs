//! .vrma (VRMC_vrm_animation) glTF document builder. Mirrors the .vrm
//! emission flow in emit.rs but produces an animation-only glTF
//! document (no mesh, no materials) per the VRMA spec.
//!
//! Per the spec:
//!   - `extensionsUsed` must list "VRMC_vrm_animation"
//!   - `extensions.VRMC_vrm_animation.specVersion` is required ("1.0")
//!   - `humanoid`, `expressions`, `lookAt` are each independently optional
//!   - The first `animations[]` entry is the portable clip

use crate::glb::{write_glb, GlbDocument};
use serde_json::{json, Value};

/// Build a minimal-valid `.vrma` JSON document: extension declaration
/// and a placeholder empty animation. Subsequent helpers in this module
/// (humanoid / expression / lookAt) mutate the returned `Value` to add
/// channels.
pub fn build_empty_vrma() -> Value {
    json!({
        "asset": { "version": "2.0", "generator": "vrm-asset-generator (vrma-v1)" },
        "nodes": [],
        "animations": [
            { "channels": [], "samplers": [] }
        ],
        "extensionsUsed": ["VRMC_vrm_animation"],
        "extensions": {
            "VRMC_vrm_animation": { "specVersion": "1.0" }
        }
    })
}

/// Serialize a complete `.vrma` document (JSON + optional binary buffer)
/// to a GLB byte stream.
pub fn write_vrma_glb(json_doc: &Value, buffer: &[u8]) -> anyhow::Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(json_doc)?;
    let doc = GlbDocument {
        json: json_bytes,
        binary: buffer.to_vec(),
    };
    write_glb(&doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::extract_json_chunk;

    #[test]
    fn empty_vrma_is_valid_glb_with_extension() {
        let doc = build_empty_vrma();
        let bytes = write_vrma_glb(&doc, &[]).unwrap();
        assert_eq!(&bytes[..4], b"glTF");

        let json_chunk = extract_json_chunk(&bytes).expect("GLB has JSON chunk");
        let parsed: Value = serde_json::from_slice(&json_chunk).unwrap();
        let used: Vec<&str> = parsed["extensionsUsed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().unwrap())
            .collect();
        assert!(
            used.contains(&"VRMC_vrm_animation"),
            "extensionsUsed must list VRMC_vrm_animation, got {used:?}"
        );
        assert_eq!(parsed["extensions"]["VRMC_vrm_animation"]["specVersion"], "1.0");
    }
}
