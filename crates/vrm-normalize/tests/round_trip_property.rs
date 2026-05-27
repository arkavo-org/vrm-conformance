//! Round-trip property: for any pair of v0-shape weight maps that are
//! losslessly equivalent (same preset names, same weights), normalization
//! to v1 produces equal v1-shape maps.
//!
//! Slice 1 success criterion #3. The stronger cross-renderer property —
//! adapter A and adapter B agree on dump(as_spec_version=V1) even when
//! native dumps differ — depends on this guarantee plus correct adapter
//! implementation. Slice 1 ships the unit-level proof; cross-renderer
//! extension lands in a later slice.

use std::collections::HashMap;
use vrm_normalize::expressions::{normalize_preset_name_v0_to_v1, normalize_weights_v0_to_v1};

#[test]
fn equivalent_v0_maps_normalize_to_equal_v1_maps() {
    let mut a: HashMap<String, f32> = HashMap::new();
    a.insert("joy".into(), 100.0);
    a.insert("neutral".into(), 50.0);
    a.insert("blink_l".into(), 25.0);

    let mut b: HashMap<String, f32> = HashMap::new();
    b.insert("joy".into(), 100.0);
    b.insert("neutral".into(), 50.0);
    b.insert("blink_l".into(), 25.0);

    let norm_a = normalize_weights_v0_to_v1(&a);
    let norm_b = normalize_weights_v0_to_v1(&b);
    assert_eq!(norm_a, norm_b);

    assert_eq!(norm_a.get("happy"), Some(&1.0));
    assert_eq!(norm_a.get("neutral"), Some(&0.5));
    assert_eq!(norm_a.get("blinkLeft"), Some(&0.25));
}

#[test]
fn preset_mapping_is_deterministic() {
    for v0 in [
        "joy",
        "neutral",
        "blink_l",
        "lookleft",
        "a",
        "i",
        "u",
        "e",
        "o",
        "MyCustomShape",
    ] {
        let one = normalize_preset_name_v0_to_v1(v0);
        let two = normalize_preset_name_v0_to_v1(v0);
        assert_eq!(one, two, "mapping non-deterministic for {v0}");
    }
}

#[test]
fn weight_scaling_round_trips_within_float_precision() {
    for v0_weight in [0.0_f32, 25.0, 50.0, 75.0, 100.0] {
        let mut m = HashMap::new();
        m.insert("joy".into(), v0_weight);
        let v1 = normalize_weights_v0_to_v1(&m);
        let v1_weight = *v1.get("happy").unwrap();
        let expected = v0_weight / 100.0;
        assert!(
            (v1_weight - expected).abs() < 1e-6,
            "v0_weight={v0_weight}, v1_weight={v1_weight}, expected={expected}"
        );
    }
}

#[test]
fn custom_preset_passthrough_preserves_uniqueness() {
    let mut a: HashMap<String, f32> = HashMap::new();
    a.insert("MyCustom1".into(), 50.0);
    a.insert("MyCustom2".into(), 75.0);
    let norm_a = normalize_weights_v0_to_v1(&a);
    assert_eq!(norm_a.get("custom:MyCustom1"), Some(&0.5));
    assert_eq!(norm_a.get("custom:MyCustom2"), Some(&0.75));
    assert_eq!(norm_a.len(), 2);
}
