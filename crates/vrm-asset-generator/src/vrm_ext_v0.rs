//! VRM 0.x extension emit. Wiring only — shared math via mtoon_common.rs.
//! Strictly emit; no parser here.

use crate::expressions_v0::ExpressionsV0Params;
use crate::params::MToonParams;
use serde_json::{json, Value};

/// Build the full `VRM` extension block for a 0.x asset, with a caller-provided
/// `secondaryAnimation` payload and humanoid bone map.
///
/// `human_bones` maps VRM 0.x bone name → glTF node index. Pass an empty map
/// for asset paths that have no skeleton (meshless MToon default). Pass
/// `&skeleton.bone_to_node` for spring-bone paths that build a real skeleton.
///
/// Pass `None` for `secondary_animation` to get the empty default
/// (material-only assets).
pub fn emit_vrm_extension_with_secondary(
    title: &str,
    materials: &[MToonParams],
    expressions: &ExpressionsV0Params,
    secondary_animation: Option<Value>,
    human_bones: &std::collections::BTreeMap<String, usize>,
) -> Value {
    let material_props: Vec<Value> = materials
        .iter()
        .map(crate::mtoon_v0::emit_material_property)
        .collect();

    let secondary = secondary_animation.unwrap_or_else(|| {
        json!({
            "boneGroups": [],
            "colliderGroups": []
        })
    });

    let human_bones_json: Vec<Value> = human_bones
        .iter()
        .map(|(bone, node)| json!({ "bone": bone, "node": node, "useDefaultValues": true }))
        .collect();

    json!({
        "exporterVersion": "vrm-asset-generator/0.x",
        "specVersion": "0.0",
        "meta": {
            "title": title,
            "version": "1",
            "author": "vrm-asset-generator",
            "contactInformation": "",
            "reference": "",
            "texture": -1,
            "allowedUserName": "OnlyAuthor",
            "violentUssageName": "Disallow",
            "sexualUssageName": "Disallow",
            "commercialUssageName": "Disallow",
            "otherPermissionUrl": "",
            "licenseName": "CC0",
            "otherLicenseUrl": ""
        },
        "humanoid": {
            "humanBones": human_bones_json,
            "armStretch": 0.05,
            "legStretch": 0.05,
            "upperArmTwist": 0.5,
            "lowerArmTwist": 0.5,
            "upperLegTwist": 0.5,
            "lowerLegTwist": 0.5,
            "feetSpacing": 0.0,
            "hasTranslationDoF": false
        },
        "firstPerson": {
            "firstPersonBone": -1,
            "firstPersonBoneOffset": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "meshAnnotations": [],
            "lookAtTypeName": "Bone",
            "lookAtHorizontalInner": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 },
            "lookAtHorizontalOuter": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 },
            "lookAtVerticalDown": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 },
            "lookAtVerticalUp": { "curve": [0, 0, 0, 1, 1, 1, 1, 0], "xRange": 90.0, "yRange": 10.0 }
        },
        "blendShapeMaster": crate::expressions_v0::emit_blend_shape_master(expressions),
        "secondaryAnimation": secondary,
        "materialProperties": material_props
    })
}

/// Build the full `VRM` extension block for a 0.x asset (empty secondaryAnimation,
/// empty humanBones). Used by the meshless MToon default path — no skeleton available.
pub fn emit_vrm_extension(
    title: &str,
    materials: &[MToonParams],
    expressions: &ExpressionsV0Params,
) -> Value {
    emit_vrm_extension_with_secondary(
        title,
        materials,
        expressions,
        None,
        &std::collections::BTreeMap::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions_v0::ExpressionsV0Params;
    use crate::params::MToonParams;

    #[test]
    fn spec_version_is_0_0() {
        let params = MToonParams::defaults("test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let v = emit_vrm_extension("test", &[params], &expressions);
        assert_eq!(v["specVersion"], "0.0");
    }

    #[test]
    fn meta_block_has_required_fields() {
        let params = MToonParams::defaults("meta_test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let v = emit_vrm_extension("my-title", &[params], &expressions);
        let meta = &v["meta"];
        assert_eq!(meta["title"], "my-title");
        assert!(meta["author"].is_string());
        assert_eq!(meta["licenseName"], "CC0");
        // Spec-correct typos in the 0.x meta block.
        assert!(meta["violentUssageName"].is_string());
        assert!(meta["sexualUssageName"].is_string());
        assert!(meta["commercialUssageName"].is_string());
    }

    #[test]
    fn material_properties_length_matches_input() {
        let params_a = MToonParams::defaults("mat_a");
        let params_b = MToonParams::defaults("mat_b");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let v = emit_vrm_extension("two-mats", &[params_a, params_b], &expressions);
        let mat_props = v["materialProperties"].as_array().unwrap();
        assert_eq!(mat_props.len(), 2);
    }

    #[test]
    fn blend_shape_groups_matches_input_expressions() {
        use crate::expressions_v0::{BlendShapeGroup, BlendShapePreset};
        let params = MToonParams::defaults("expr_test");
        let expressions = ExpressionsV0Params {
            groups: vec![
                BlendShapeGroup {
                    name: "Joy".into(),
                    preset: BlendShapePreset::Joy,
                    binds: vec![],
                },
                BlendShapeGroup {
                    name: "Neutral".into(),
                    preset: BlendShapePreset::Neutral,
                    binds: vec![],
                },
            ],
        };
        let v = emit_vrm_extension("expr_test", &[params], &expressions);
        let groups = v["blendShapeMaster"]["blendShapeGroups"]
            .as_array()
            .unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn block_is_json_serializable() {
        let params = MToonParams::defaults("serial_test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let v = emit_vrm_extension("serial_test", &[params], &expressions);
        let s = serde_json::to_string(&v);
        assert!(s.is_ok(), "must serialize without error");
    }

    #[test]
    fn humanoid_block_is_present() {
        let params = MToonParams::defaults("humanoid_test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let v = emit_vrm_extension("humanoid_test", &[params], &expressions);
        assert!(v["humanoid"].is_object());
        assert!(v["humanoid"]["humanBones"].is_array());
    }

    #[test]
    fn vrm_ext_carries_provided_secondary_animation() {
        use std::collections::BTreeMap;
        let params = MToonParams::defaults("sa_test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let sa = serde_json::json!({
            "boneGroups": [{"comment": "x", "stiffiness": 0.5}],
            "colliderGroups": []
        });
        let ext = emit_vrm_extension_with_secondary(
            "sa_test",
            &[params],
            &expressions,
            Some(sa),
            &BTreeMap::new(),
        );
        assert_eq!(ext["secondaryAnimation"]["boneGroups"][0]["comment"], "x");
    }

    #[test]
    fn vrm_ext_default_secondary_animation_is_empty() {
        let params = MToonParams::defaults("sa_default");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let ext = emit_vrm_extension("sa_default", &[params], &expressions);
        assert_eq!(
            ext["secondaryAnimation"]["boneGroups"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn human_bones_populated_from_map() {
        use std::collections::BTreeMap;
        let params = MToonParams::defaults("hb_test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let mut bones: BTreeMap<String, usize> = BTreeMap::new();
        bones.insert("hips".to_string(), 0);
        bones.insert("head".to_string(), 4);
        let ext =
            emit_vrm_extension_with_secondary("hb_test", &[params], &expressions, None, &bones);
        let human_bones = ext["humanoid"]["humanBones"].as_array().unwrap();
        assert_eq!(human_bones.len(), 2, "expected 2 human bone entries");
        let hips_entry = human_bones
            .iter()
            .find(|e| e["bone"] == "hips")
            .expect("hips entry missing");
        assert_eq!(hips_entry["node"], 0, "hips node index should be 0");
        assert_eq!(
            hips_entry["useDefaultValues"], true,
            "useDefaultValues should be true"
        );
        let head_entry = human_bones
            .iter()
            .find(|e| e["bone"] == "head")
            .expect("head entry missing");
        assert_eq!(head_entry["node"], 4, "head node index should be 4");
    }

    #[test]
    fn emit_vrm_extension_still_has_empty_human_bones() {
        // emit_vrm_extension (no-skeleton path) must still produce empty humanBones
        // — the meshless MToon default is a known limitation and not in scope.
        let params = MToonParams::defaults("empty_bones_test");
        let expressions = ExpressionsV0Params { groups: vec![] };
        let ext = emit_vrm_extension("empty_bones_test", &[params], &expressions);
        let human_bones = ext["humanoid"]["humanBones"].as_array().unwrap();
        assert_eq!(
            human_bones.len(),
            0,
            "meshless path must keep empty humanBones"
        );
    }
}
