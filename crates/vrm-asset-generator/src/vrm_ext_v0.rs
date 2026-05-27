//! VRM 0.x extension emit. Wiring only — shared math lives in mtoon_common.rs.
//! Strictly emit; no parser here (round-tripping is not a v1 goal).
//!
//! Slice 1 Task 11: initial stub that produces a minimal `VRM` extension block.
//! Task 15 replaces this with the full assembly (humanoid, expressions, materials).

use serde_json::{json, Value};

/// Stub: emits a minimal `VRM` extension block. Real content lands in
/// Task 15.
pub fn emit_stub_vrm_extension() -> Value {
    json!({
        "exporterVersion": "vrm-asset-generator-0.x-stub",
        "specVersion": "0.0",
        "meta": {
            "title": "stub",
            "version": "1",
            "author": "vrm-asset-generator",
            "licenseName": "CC0",
        },
        "humanoid": { "humanBones": [] },
        "firstPerson": {},
        "blendShapeMaster": { "blendShapeGroups": [] },
        "secondaryAnimation": { "boneGroups": [], "colliderGroups": [] },
        "materialProperties": [],
    })
}
