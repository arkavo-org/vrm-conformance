//! Verifies the runner's diff-engine bridge: synthesize two PNGs, run
//! diff_one against a synthetic plan, assert structural results.

use camino::Utf8PathBuf;
use vrm_runner::diff::diff_one;
use vrm_test_plan::{
    AmbientLight, Camera, ColorSpace, Diff, DiffMode, DirectionalLight, Lighting, Output,
    PostProcessing, TestPlan, ToneMapping,
};

fn make_solid_png(path: &std::path::Path, w: u32, h: u32, rgb: [u8; 3]) {
    image::RgbImage::from_fn(w, h, |_, _| image::Rgb(rgb))
        .save(path)
        .expect("save png");
}

fn synthetic_plan(id: &str, threshold: f32) -> TestPlan {
    TestPlan {
        id: id.into(),
        spec_section: "test".into(),
        asset: "synthetic.vrm".into(),
        camera: Camera {
            position: [0.0, 0.0, 1.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [0.0, -1.0, 0.0],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false,
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        },
        output: Output {
            width: 64,
            height: 64,
            color_space: ColorSpace::Linear,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold,
            reference_renderer: "test-renderer".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
    }
}

#[test]
fn identical_pngs_pass_ssim_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let render = dir.path().join("render.png");
    let reference = dir.path().join("reference.png");

    // Avatar grey on magenta sentinel background (matches diff-engine
    // bbox-detection convention).
    let mut img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 0, 255]));
    for y in 16..48 {
        for x in 16..48 {
            img.put_pixel(x, y, image::Rgb([128, 128, 128]));
        }
    }
    img.save(&render).unwrap();
    img.save(&reference).unwrap();

    let plan = synthetic_plan("identical_test", 0.985);
    let render_path = Utf8PathBuf::from_path_buf(render).unwrap();
    let reference_path = Utf8PathBuf::from_path_buf(reference).unwrap();

    let result = diff_one(&plan, &render_path, &reference_path, "test-renderer").unwrap();
    assert_eq!(result.test_id, "identical_test");
    assert_eq!(result.renderer, "test-renderer");
    assert_eq!(result.reference_renderer, "test-renderer");
    assert!(
        result.ssim > 0.99,
        "identical PNGs SSIM should be ~1, got {}",
        result.ssim
    );
    assert!(result.ssim_passed);
    assert!(result.overall_passed());
}

#[test]
fn different_pngs_fail_ssim_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let render = dir.path().join("render.png");
    let reference = dir.path().join("reference.png");

    make_solid_png(&render, 64, 64, [0, 0, 0]);
    make_solid_png(&reference, 64, 64, [255, 255, 255]);

    let plan = synthetic_plan("different_test", 0.985);
    let render_path = Utf8PathBuf::from_path_buf(render).unwrap();
    let reference_path = Utf8PathBuf::from_path_buf(reference).unwrap();

    let result = diff_one(&plan, &render_path, &reference_path, "test-renderer").unwrap();
    assert!(
        result.ssim < 0.5,
        "black vs white SSIM should be low, got {}",
        result.ssim
    );
    assert!(!result.ssim_passed);
    assert!(!result.overall_passed());
}

#[test]
fn property_assertion_is_evaluated_on_render() {
    use vrm_diff_engine::property::{BboxRegion, PropertyAssertion};

    let dir = tempfile::tempdir().unwrap();
    let render = dir.path().join("render.png");
    let reference = dir.path().join("reference.png");

    // Avatar at known luminance ~0.5 on magenta sentinel.
    let mut img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 0, 255]));
    for y in 16..48 {
        for x in 16..48 {
            img.put_pixel(x, y, image::Rgb([128, 128, 128]));
        }
    }
    img.save(&render).unwrap();
    img.save(&reference).unwrap();

    let mut plan = synthetic_plan("with_property", 0.985);
    plan.properties.push(PropertyAssertion {
        name: "avg_lum_full".into(),
        region: BboxRegion::BboxFull,
        expected: 128.0 / 255.0,
        tolerance: 0.05,
    });

    let render_path = Utf8PathBuf::from_path_buf(render).unwrap();
    let reference_path = Utf8PathBuf::from_path_buf(reference).unwrap();

    let result = diff_one(&plan, &render_path, &reference_path, "test-renderer").unwrap();
    assert_eq!(result.properties.len(), 1);
    let prop = &result.properties[0];
    assert_eq!(prop.name, "avg_lum_full");
    assert!(prop.passed, "expected ~0.5, got actual={}", prop.actual);
    assert!(result.overall_passed());
}
