//! VRM 0.x expressions emit. Produces `blendShapeMaster.blendShapeGroups[]`
//! entries with preset names (joy, neutral, sorrow, etc.) and per-mesh
//! morph-target bindings.
//!
//! Note: 0.x weight range is 0–100 (Unity convention), NOT 0–1 like 1.0.
//! Schema: `docs/upstream-specs/vrm-specification/specification/0.0/schema/vrm.blendshape.schema.json`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// 0.x preset names. Canonical per spec §VRM/BlendShape.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlendShapePreset {
    Neutral,
    Joy,
    Angry,
    Sorrow,
    Fun,
    A,
    I,
    U,
    E,
    O,
    Blink,
    BlinkL,
    BlinkR,
    Lookup,
    Lookdown,
    Lookleft,
    Lookright,
    Unknown,
}

impl BlendShapePreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Joy => "joy",
            Self::Angry => "angry",
            Self::Sorrow => "sorrow",
            Self::Fun => "fun",
            Self::A => "a",
            Self::I => "i",
            Self::U => "u",
            Self::E => "e",
            Self::O => "o",
            Self::Blink => "blink",
            Self::BlinkL => "blink_l",
            Self::BlinkR => "blink_r",
            Self::Lookup => "lookup",
            Self::Lookdown => "lookdown",
            Self::Lookleft => "lookleft",
            Self::Lookright => "lookright",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpressionsV0Params {
    pub groups: Vec<BlendShapeGroup>,
}

#[derive(Clone, Debug)]
pub struct BlendShapeGroup {
    pub name: String,
    pub preset: BlendShapePreset,
    pub binds: Vec<BlendShapeBind>,
}

#[derive(Clone, Debug)]
pub struct BlendShapeBind {
    pub mesh_index: u32,
    pub morph_target_index: u32,
    pub weight_0_to_100: f32,
}

pub fn emit_blend_shape_master(params: &ExpressionsV0Params) -> Value {
    let groups: Vec<Value> = params
        .groups
        .iter()
        .map(|g| {
            json!({
                "name": g.name,
                "presetName": g.preset.as_str(),
                "binds": g.binds.iter().map(|b| json!({
                    "mesh": b.mesh_index,
                    "index": b.morph_target_index,
                    "weight": b.weight_0_to_100,
                })).collect::<Vec<_>>(),
                "materialValues": [],
                "isBinary": false,
            })
        })
        .collect();

    json!({ "blendShapeGroups": groups })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names_match_spec_canonical() {
        assert_eq!(BlendShapePreset::Joy.as_str(), "joy");
        assert_eq!(BlendShapePreset::Neutral.as_str(), "neutral");
        assert_eq!(BlendShapePreset::BlinkL.as_str(), "blink_l");
        assert_eq!(BlendShapePreset::BlinkR.as_str(), "blink_r");
        assert_eq!(BlendShapePreset::Lookup.as_str(), "lookup");
        assert_eq!(BlendShapePreset::Lookright.as_str(), "lookright");
    }

    #[test]
    fn emits_blend_shape_master_with_joy_group() {
        let params = ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "Joy".into(),
                preset: BlendShapePreset::Joy,
                binds: vec![BlendShapeBind {
                    mesh_index: 0,
                    morph_target_index: 3,
                    weight_0_to_100: 100.0,
                }],
            }],
        };
        let v = emit_blend_shape_master(&params);
        assert_eq!(v["blendShapeGroups"][0]["presetName"], "joy");
        assert_eq!(v["blendShapeGroups"][0]["binds"][0]["mesh"], 0);
        assert_eq!(v["blendShapeGroups"][0]["binds"][0]["weight"], 100.0);
    }

    #[test]
    fn weight_uses_0_100_range_not_0_1() {
        // Methodology pin: 0.x weights are Unity-convention 0–100, not 0–1.
        let params = ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "Half".into(),
                preset: BlendShapePreset::Joy,
                binds: vec![BlendShapeBind {
                    mesh_index: 0,
                    morph_target_index: 0,
                    weight_0_to_100: 50.0,
                }],
            }],
        };
        let v = emit_blend_shape_master(&params);
        assert_eq!(v["blendShapeGroups"][0]["binds"][0]["weight"], 50.0);
    }

    #[test]
    fn emits_material_values_and_is_binary() {
        let params = ExpressionsV0Params {
            groups: vec![BlendShapeGroup {
                name: "Test".into(),
                preset: BlendShapePreset::Neutral,
                binds: vec![],
            }],
        };
        let v = emit_blend_shape_master(&params);
        assert!(v["blendShapeGroups"][0]["materialValues"].is_array());
        assert_eq!(
            v["blendShapeGroups"][0]["materialValues"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(v["blendShapeGroups"][0]["isBinary"], false);
    }

    #[test]
    fn empty_groups_produces_empty_array() {
        let params = ExpressionsV0Params { groups: vec![] };
        let v = emit_blend_shape_master(&params);
        assert_eq!(v["blendShapeGroups"].as_array().unwrap().len(), 0);
    }
}
