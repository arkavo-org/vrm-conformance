//! Expression-preset normalization. v0 `blendShapeMaster` preset names
//! → v1 `VRMC_vrm.expressions.preset` preset names.
//!
//! v0 weight range is 0–100; v1 weight range is 0–1. Normalization
//! divides by 100.

use std::collections::HashMap;

/// Returns the v1 preset name for the given v0 preset name. Unknown v0
/// presets pass through with `custom:<name>` markers — never dropped.
pub fn normalize_preset_name_v0_to_v1(v0_preset: &str) -> String {
    match v0_preset {
        "joy" => "happy".into(),
        "angry" => "angry".into(),
        "sorrow" => "sad".into(),
        "fun" => "relaxed".into(),
        "neutral" => "neutral".into(),
        "a" => "aa".into(),
        "i" => "ih".into(),
        "u" => "ou".into(),
        "e" => "ee".into(),
        "o" => "oh".into(),
        "blink" => "blink".into(),
        "blink_l" => "blinkLeft".into(),
        "blink_r" => "blinkRight".into(),
        "lookup" => "lookUp".into(),
        "lookdown" => "lookDown".into(),
        "lookleft" => "lookLeft".into(),
        "lookright" => "lookRight".into(),
        // Anything else is custom — pass through with marker.
        custom => format!("custom:{custom}"),
    }
}

/// Normalize a complete v0 weight map (preset → weight 0..100) to v1
/// (preset → weight 0..1).
pub fn normalize_weights_v0_to_v1(v0_weights: &HashMap<String, f32>) -> HashMap<String, f32> {
    v0_weights
        .iter()
        .map(|(k, v)| (normalize_preset_name_v0_to_v1(k), v / 100.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joy_maps_to_happy() {
        assert_eq!(normalize_preset_name_v0_to_v1("joy"), "happy");
    }

    #[test]
    fn sorrow_maps_to_sad() {
        assert_eq!(normalize_preset_name_v0_to_v1("sorrow"), "sad");
    }

    #[test]
    fn fun_maps_to_relaxed() {
        assert_eq!(normalize_preset_name_v0_to_v1("fun"), "relaxed");
    }

    #[test]
    fn vowels_map_to_phonetic() {
        assert_eq!(normalize_preset_name_v0_to_v1("a"), "aa");
        assert_eq!(normalize_preset_name_v0_to_v1("i"), "ih");
        assert_eq!(normalize_preset_name_v0_to_v1("u"), "ou");
        assert_eq!(normalize_preset_name_v0_to_v1("e"), "ee");
        assert_eq!(normalize_preset_name_v0_to_v1("o"), "oh");
    }

    #[test]
    fn blink_variants_map_correctly() {
        assert_eq!(normalize_preset_name_v0_to_v1("blink_l"), "blinkLeft");
        assert_eq!(normalize_preset_name_v0_to_v1("blink_r"), "blinkRight");
    }

    #[test]
    fn lookat_variants_map_correctly() {
        assert_eq!(normalize_preset_name_v0_to_v1("lookup"), "lookUp");
        assert_eq!(normalize_preset_name_v0_to_v1("lookdown"), "lookDown");
        assert_eq!(normalize_preset_name_v0_to_v1("lookleft"), "lookLeft");
        assert_eq!(normalize_preset_name_v0_to_v1("lookright"), "lookRight");
    }

    #[test]
    fn custom_passes_through_with_marker() {
        assert_eq!(
            normalize_preset_name_v0_to_v1("MyCustom"),
            "custom:MyCustom"
        );
        assert_eq!(
            normalize_preset_name_v0_to_v1("Surprised"),
            "custom:Surprised"
        );
    }

    #[test]
    fn weights_scale_from_0_100_to_0_1() {
        let mut v0 = HashMap::new();
        v0.insert("joy".into(), 100.0);
        v0.insert("neutral".into(), 50.0);
        let v1 = normalize_weights_v0_to_v1(&v0);
        assert!((v1["happy"] - 1.0).abs() < 1e-6);
        assert!((v1["neutral"] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn zero_weight_round_trips() {
        let mut v0 = HashMap::new();
        v0.insert("joy".into(), 0.0);
        let v1 = normalize_weights_v0_to_v1(&v0);
        assert_eq!(v1["happy"], 0.0);
    }
}
