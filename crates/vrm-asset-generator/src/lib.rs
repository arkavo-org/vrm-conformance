//! Parametric VRM 1.0 asset generator. Emits paired
//! `<asset>.vrm + <asset>.meta.json + <asset>.test.yaml` from a single
//! parameter dictionary.

// The `describe` operation catalog is a single large `serde_json::json!`
// literal; each added op deepens the macro expansion. Lift the limit above the
// default 128 so the catalog can keep growing.
#![recursion_limit = "256"]

pub mod buffer;
pub mod chain_mesh;
pub mod cli;
pub mod emit;
pub mod expressions_v0;
pub mod glb;
pub mod humanoid;
pub mod mesh;
pub mod mtoon_common;
pub mod mtoon_v0;
pub mod params;
pub mod sidecar;
pub mod spring_bone;
pub mod spring_bone_v0;
pub mod sweep;
pub mod texture;
pub mod vrm_ext;
pub mod vrm_ext_v0;
pub mod vrma_emit;
pub mod vrma_params;

pub use vrm_ops::SpecVersion;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum SweepApplicability {
    Applicable,
    NotApplicable { reason: NotApplicableReason },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NotApplicableReason {
    PerJointStiffnessV1Only,
    CapsuleColliderV1Only,
    ExtendedCollidersV1Only,
    OutlineLightingMixV1Only,
    VrmaIsVrm1Era,
    ShadingShiftTextureV1Only,
    RimMultiplyTextureV1Only,
    KhrTextureTransformV1Only,
    KhrMaterialsEmissiveStrengthV1Only,
    // Extend with new variants as discovered; never use free text.
}

#[cfg(test)]
mod applicability_tests {
    use super::*;

    #[test]
    fn applicable_round_trips() {
        let a = SweepApplicability::Applicable;
        let s = serde_json::to_string(&a).unwrap();
        assert_eq!(s, r#"{"kind":"Applicable"}"#);
        let back: SweepApplicability = serde_json::from_str(&s).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn not_applicable_carries_reason() {
        let n = SweepApplicability::NotApplicable {
            reason: NotApplicableReason::OutlineLightingMixV1Only,
        };
        let s = serde_json::to_string(&n).unwrap();
        assert_eq!(
            s,
            r#"{"kind":"NotApplicable","reason":"OutlineLightingMixV1Only"}"#
        );
        let back: SweepApplicability = serde_json::from_str(&s).unwrap();
        assert_eq!(back, n);
    }

    #[test]
    fn new_v1_only_reasons_serialize() {
        for r in [
            NotApplicableReason::ShadingShiftTextureV1Only,
            NotApplicableReason::RimMultiplyTextureV1Only,
            NotApplicableReason::KhrTextureTransformV1Only,
            NotApplicableReason::KhrMaterialsEmissiveStrengthV1Only,
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: NotApplicableReason = serde_json::from_str(&s).unwrap();
            assert_eq!(r, back);
        }
    }
}
