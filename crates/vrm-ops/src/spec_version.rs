//! Spec version enum — wire form is `"0.x"` / `"1.0"`. Threaded through
//! generator CLI, manifest schema, test plan, ops contract.
//!
//! See `docs/superpowers/specs/2026-05-26-vrm-0x-conformance-design.md`
//! for the design rationale.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum SpecVersion {
    #[serde(rename = "0.x")]
    V0,
    #[serde(rename = "1.0")]
    V1,
}

impl SpecVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            SpecVersion::V0 => "0.x",
            SpecVersion::V1 => "1.0",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_wire_form() {
        assert_eq!(serde_json::to_string(&SpecVersion::V0).unwrap(), "\"0.x\"");
        assert_eq!(serde_json::to_string(&SpecVersion::V1).unwrap(), "\"1.0\"");
    }

    #[test]
    fn deserializes_from_wire_form() {
        let v0: SpecVersion = serde_json::from_str("\"0.x\"").unwrap();
        let v1: SpecVersion = serde_json::from_str("\"1.0\"").unwrap();
        assert_eq!(v0, SpecVersion::V0);
        assert_eq!(v1, SpecVersion::V1);
    }

    #[test]
    fn rejects_unknown_wire_form() {
        assert!(serde_json::from_str::<SpecVersion>("\"2.0\"").is_err());
        assert!(serde_json::from_str::<SpecVersion>("\"v0\"").is_err());
    }

    #[test]
    fn as_str_round_trips() {
        assert_eq!(SpecVersion::V0.as_str(), "0.x");
        assert_eq!(SpecVersion::V1.as_str(), "1.0");
    }
}
