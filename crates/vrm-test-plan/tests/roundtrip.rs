use vrm_test_plan::TestPlan;

const SAMPLE_YAML: &str = r#"
id: mtoon_shading_shift_negative
spec_section: VRMC_materials_mtoon §3.2 shadingShift
asset: generated/mtoon_basic_shadingShift_neg0.5.vrm
camera:
  position: [0.0, 1.4, 1.5]
  target: [0.0, 1.4, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [-0.3, -0.6, -0.7]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [0.5, 0.5, 0.5]
    intensity: 0.3
  cast_shadows: false
  receive_shadows: false
post_processing:
  tone_mapping: none
  exposure: 1.0
output:
  width: 1024
  height: 1024
  color_space: linear
  msaa: 4
diff:
  mode: ssim
  threshold: 0.985
  reference_renderer: vrm-metal-kit
ignore_renderers: []
properties: []
"#;

#[test]
fn parses_sample_plan() {
    let plan: TestPlan = serde_yml::from_str(SAMPLE_YAML).unwrap();
    assert_eq!(plan.id, "mtoon_shading_shift_negative");
    assert_eq!(plan.output.width, 1024);
    assert_eq!(plan.output.msaa, 4);
}

#[test]
fn round_trips_yaml() {
    let plan: TestPlan = serde_yml::from_str(SAMPLE_YAML).unwrap();
    let serialized = serde_yml::to_string(&plan).unwrap();
    let reparsed: TestPlan = serde_yml::from_str(&serialized).unwrap();
    pretty_assertions::assert_eq!(plan, reparsed);
}

#[test]
fn defaults_post_processing_when_omitted() {
    let yaml_without_post_processing = r#"
id: t
spec_section: x
asset: a.vrm
camera:
  position: [0.0, 0.0, 0.0]
  target: [0.0, 0.0, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [0.5, 0.5, 0.5]
    intensity: 0.3
output:
  width: 256
  height: 256
  color_space: linear
  msaa: 4
diff:
  mode: ssim
  threshold: 0.99
  reference_renderer: x
"#;
    let plan: TestPlan = serde_yml::from_str(yaml_without_post_processing).unwrap();
    assert!(matches!(
        plan.post_processing.tone_mapping,
        vrm_test_plan::ToneMapping::None
    ));
    assert_eq!(plan.post_processing.exposure, 1.0);
}
