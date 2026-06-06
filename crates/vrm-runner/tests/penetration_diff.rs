//! Integration tests for `vrm_runner::penetration_diff`.
//!
//! These tests run against hand-written positions JSON fixtures and known
//! collider geometries — no GPU or adapter binary required.

use std::io::Write;
use tempfile::NamedTempFile;
use vrm_runner::penetration_diff::run_penetration_diff;
use vrm_test_plan::ColliderWorldSpec;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Write a string to a named temp file and return it (caller keeps handle so
/// the file lives long enough for the test).
fn write_tempfile(contents: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

/// Minimal test plan YAML with one sphere collider at origin, radius 0.05 m.
fn plan_yaml_with_sphere(center: [f32; 3], radius: f32) -> String {
    format!(
        r#"
id: ccd_test
spec_section: ccd
asset: x.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: {{ dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }}
  ambient: {{ color: [1, 1, 1], intensity: 0.2 }}
output:
  width: 256
  height: 256
  color_space: srgb
  msaa: 1
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: mock
render_sequence:
  frame_count: 3
  frame_hz: 30.0
  physics_dt_seconds: 0.01666
  output_format: png_sequence
  capture_positions: true
ccd_colliders:
  - type: sphere
    center: [{cx}, {cy}, {cz}]
    radius: {r}
"#,
        cx = center[0],
        cy = center[1],
        cz = center[2],
        r = radius,
    )
}

/// Minimal test plan YAML with one capsule collider.
fn plan_yaml_with_capsule(a: [f32; 3], b: [f32; 3], radius: f32) -> String {
    format!(
        r#"
id: ccd_capsule_test
spec_section: ccd
asset: x.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: {{ dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }}
  ambient: {{ color: [1, 1, 1], intensity: 0.2 }}
output:
  width: 256
  height: 256
  color_space: srgb
  msaa: 1
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: mock
render_sequence:
  frame_count: 3
  frame_hz: 30.0
  physics_dt_seconds: 0.01666
  output_format: png_sequence
  capture_positions: true
ccd_colliders:
  - type: capsule
    a: [{ax}, {ay}, {az}]
    b: [{bx}, {by}, {bz}]
    radius: {r}
"#,
        ax = a[0],
        ay = a[1],
        az = a[2],
        bx = b[0],
        by = b[1],
        bz = b[2],
        r = radius,
    )
}

/// Positions JSON with joints clearly outside the sphere (r=0.05, center at
/// origin). All joints at distance 0.10 from origin — no penetration.
const POSITIONS_OUTSIDE_SPHERE: &str = r#"
[
  {
    "frame_index": 0,
    "timestamp_seconds": 0.0,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0], [0.0, 0.10, 0.0]] }
    ]
  },
  {
    "frame_index": 1,
    "timestamp_seconds": 0.0333,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0], [0.0, 0.10, 0.0]] }
    ]
  }
]
"#;

/// Positions JSON where frame 1 has a joint penetrating the sphere (depth ~0.03 m).
/// sphere center=[0,0,0] radius=0.05; joint at [0.02, 0.0, 0.0] → depth 0.03 m.
const POSITIONS_PENETRATING_SPHERE: &str = r#"
[
  {
    "frame_index": 0,
    "timestamp_seconds": 0.0,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0]] }
    ]
  },
  {
    "frame_index": 1,
    "timestamp_seconds": 0.0333,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.02, 0.0, 0.0]] }
    ]
  }
]
"#;

/// Positions JSON for capsule test — all joints outside capsule (axis Y [-0.1, 0.1],
/// radius 0.03). Joints placed at X=0.10 — well outside.
const POSITIONS_OUTSIDE_CAPSULE: &str = r#"
[
  {
    "frame_index": 0,
    "timestamp_seconds": 0.0,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0]] }
    ]
  }
]
"#;

/// Positions JSON for capsule test — joint at [0.02, 0.0, 0.0] penetrates
/// capsule (axis Y [-0.1, 0.1], radius 0.03) by 0.01 m.
const POSITIONS_PENETRATING_CAPSULE: &str = r#"
[
  {
    "frame_index": 0,
    "timestamp_seconds": 0.0,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.02, 0.0, 0.0]] }
    ]
  }
]
"#;

// ── mapping tests ─────────────────────────────────────────────────────────────

#[test]
fn to_collider_spec_sphere_maps_correctly() {
    use vrm_runner::penetration_diff::to_collider_spec;
    let spec = ColliderWorldSpec::Sphere {
        center: [1.0, 2.0, 3.0],
        radius: 0.05,
    };
    let engine_spec = to_collider_spec(&spec);
    match engine_spec {
        vrm_diff_engine::penetration::ColliderSpec::Sphere { center, radius } => {
            assert_eq!(center, [1.0_f32, 2.0, 3.0]);
            assert!((radius - 0.05).abs() < 1e-6);
        }
        other => panic!("expected Sphere, got {other:?}"),
    }
}

#[test]
fn to_collider_spec_capsule_maps_correctly() {
    use vrm_runner::penetration_diff::to_collider_spec;
    let spec = ColliderWorldSpec::Capsule {
        a: [0.0, -0.1, 0.0],
        b: [0.0, 0.1, 0.0],
        radius: 0.03,
    };
    let engine_spec = to_collider_spec(&spec);
    match engine_spec {
        vrm_diff_engine::penetration::ColliderSpec::Capsule { a, b, radius } => {
            assert_eq!(a, [0.0_f32, -0.1, 0.0]);
            assert_eq!(b, [0.0_f32, 0.1, 0.0]);
            assert!((radius - 0.03).abs() < 1e-6);
        }
        other => panic!("expected Capsule, got {other:?}"),
    }
}

// ── run_penetration_diff tests ────────────────────────────────────────────────

#[test]
fn run_penetration_diff_passes_when_no_penetration() {
    let plan_yaml = plan_yaml_with_sphere([0.0, 0.0, 0.0], 0.05);
    let plan_file = write_tempfile(&plan_yaml);
    let pos_file = write_tempfile(POSITIONS_OUTSIDE_SPHERE);

    let report = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    )
    .unwrap();

    assert!(
        report.passed,
        "joints outside sphere should pass; max_depth={:.4}",
        report.max_penetration_depth_m
    );
    assert_eq!(report.max_penetration_depth_m, 0.0);
}

#[test]
fn run_penetration_diff_fails_when_penetrating_sphere() {
    let plan_yaml = plan_yaml_with_sphere([0.0, 0.0, 0.0], 0.05);
    let plan_file = write_tempfile(&plan_yaml);
    let pos_file = write_tempfile(POSITIONS_PENETRATING_SPHERE);

    let report = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    )
    .unwrap();

    assert!(
        !report.passed,
        "joint inside sphere should fail; max_depth={:.4}",
        report.max_penetration_depth_m
    );
    // depth = 0.05 - 0.02 = 0.03 m
    assert!(
        (report.max_penetration_depth_m - 0.03).abs() < 1e-4,
        "expected ~0.030 m, got {}",
        report.max_penetration_depth_m
    );
    // Penetration is in the entry with frame_index=1 (both slice index and
    // real frame_index agree for contiguous [0, 1] data).
    assert_eq!(
        report.worst_frame_index, 1,
        "worst_frame_index should be the real frame_index=1"
    );
    assert_eq!(
        report.worst_frame_slice, 1,
        "worst_frame_slice should be slice position 1"
    );
}

#[test]
fn run_penetration_diff_passes_for_capsule_outside() {
    let plan_yaml = plan_yaml_with_capsule([0.0, -0.1, 0.0], [0.0, 0.1, 0.0], 0.03);
    let plan_file = write_tempfile(&plan_yaml);
    let pos_file = write_tempfile(POSITIONS_OUTSIDE_CAPSULE);

    let report = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    )
    .unwrap();

    assert!(report.passed, "joint outside capsule should pass");
}

#[test]
fn run_penetration_diff_fails_for_capsule_penetrating() {
    let plan_yaml = plan_yaml_with_capsule([0.0, -0.1, 0.0], [0.0, 0.1, 0.0], 0.03);
    let plan_file = write_tempfile(&plan_yaml);
    let pos_file = write_tempfile(POSITIONS_PENETRATING_CAPSULE);

    let report = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    )
    .unwrap();

    assert!(
        !report.passed,
        "joint inside capsule should fail; depth={}",
        report.max_penetration_depth_m
    );
    // joint at (0.02, 0, 0), capsule axis along Y; dist to segment = 0.02, radius 0.03
    // depth = 0.03 - 0.02 = 0.01 m
    assert!(
        (report.max_penetration_depth_m - 0.01).abs() < 1e-4,
        "expected ~0.010 m, got {}",
        report.max_penetration_depth_m
    );
}

#[test]
fn run_penetration_diff_errors_when_plan_has_no_ccd_colliders() {
    // A plan without ccd_colliders must return a clear error.
    let plan_yaml = r#"
id: no_ccd
spec_section: x
asset: x.vrm
camera:
  position: [0, 1.3, 1.5]
  target: [0, 1.3, 0]
  up: [0, 1, 0]
  fov_degrees: 30
lighting:
  directional: { dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 }
  ambient: { color: [1, 1, 1], intensity: 0.2 }
output:
  width: 256
  height: 256
  color_space: srgb
  msaa: 1
diff:
  mode: ssim
  threshold: 0.95
  reference_renderer: mock
"#;
    let plan_file = write_tempfile(plan_yaml);
    let pos_file = write_tempfile(POSITIONS_OUTSIDE_SPHERE);

    let result = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("ccd_colliders"),
        "error message must mention ccd_colliders; got: {err}"
    );
}

#[test]
fn run_penetration_diff_sorts_by_frame_index() {
    // Positions JSON with frames in reversed order — result must sort and report
    // the correct worst_frame_index (the real frame_index from the JSON).
    let reversed_positions = r#"
[
  {
    "frame_index": 1,
    "timestamp_seconds": 0.0333,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0]] }
    ]
  },
  {
    "frame_index": 0,
    "timestamp_seconds": 0.0,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.02, 0.0, 0.0]] }
    ]
  }
]
"#;
    // sphere at origin r=0.05; joint at [0.02, 0, 0] penetrates 0.03 m → in frame_index 0
    // After sorting, frame_index 0 lands at slice position 0.
    let plan_yaml = plan_yaml_with_sphere([0.0, 0.0, 0.0], 0.05);
    let plan_file = write_tempfile(&plan_yaml);
    let pos_file = write_tempfile(reversed_positions);

    let report = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    )
    .unwrap();

    assert!(!report.passed);
    // worst_frame_index must be the real frame_index=0 (not the slice index).
    assert_eq!(
        report.worst_frame_index, 0,
        "worst_frame_index must be the real frame_index=0"
    );
    assert_eq!(
        report.worst_frame_slice, 0,
        "slice index 0 after sort (frame_index=0 was first)"
    );
}

#[test]
fn run_penetration_diff_reports_real_frame_index_for_non_contiguous_frames() {
    // Fix 2: when an adapter captures only a subset of frames (e.g. 0, 5, 10),
    // worst_frame_index must be the real frame_index from the JSON, NOT the
    // 0-based slice index.
    //
    // Here: penetration is in frame_index=10 (slice index 2 after sort).
    // worst_frame_index must report 10, not 2.
    let sparse_positions = r#"
[
  {
    "frame_index": 0,
    "timestamp_seconds": 0.0,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0]] }
    ]
  },
  {
    "frame_index": 5,
    "timestamp_seconds": 0.0833,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.10, 0.0, 0.0]] }
    ]
  },
  {
    "frame_index": 10,
    "timestamp_seconds": 0.1666,
    "springs": [
      { "name": "chain0", "joint_positions": [[0.02, 0.0, 0.0]] }
    ]
  }
]
"#;
    // sphere at origin r=0.05; joint penetrates at frame_index=10 (slice index 2)
    let plan_yaml = plan_yaml_with_sphere([0.0, 0.0, 0.0], 0.05);
    let plan_file = write_tempfile(&plan_yaml);
    let pos_file = write_tempfile(sparse_positions);

    let report = run_penetration_diff(
        pos_file.path().try_into().unwrap(),
        plan_file.path().try_into().unwrap(),
        None,
        0.002,
    )
    .unwrap();

    assert!(!report.passed, "penetration must be detected");
    assert!(
        (report.max_penetration_depth_m - 0.03).abs() < 1e-4,
        "depth ~0.03 m, got {}",
        report.max_penetration_depth_m
    );
    // worst_frame_slice is the engine's 0-based index = 2 (third entry after sort)
    assert_eq!(
        report.worst_frame_slice, 2,
        "engine slice index must be 2 (third entry after sort)"
    );
    // worst_frame_index must be the real frame_index=10 from the JSON
    assert_eq!(
        report.worst_frame_index, 10,
        "worst_frame_index must report the real frame_index=10, not the slice index 2"
    );
}
