use vrm_diff_engine::property::{eval_property, BboxRegion, PropertyAssertion};

fn make_test_image() -> image::RgbImage {
    // 100×100 with a dark gray 50×50 square centered (the "avatar") on a
    // magenta background. Avatar gray = ~0.25 luminance.
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    for y in 25..75 {
        for x in 25..75 {
            img.put_pixel(x, y, image::Rgb([64, 64, 64]));
        }
    }
    img
}

#[test]
fn full_bbox_average_luminance() {
    let img = make_test_image();
    let pa = PropertyAssertion {
        name: "avg_lum".into(),
        region: BboxRegion::BboxFull,
        expected: 0.25,
        tolerance: 0.05,
    };
    let result = eval_property(&img, &pa).unwrap();
    assert!(
        result.passed,
        "expected pass, got actual={} tolerance band ±{}",
        result.actual, pa.tolerance
    );
}

#[test]
fn lower_left_quad_only_samples_lower_left() {
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    // Avatar bbox: 25..75, 25..75. Lower-left quad (in image-Y-down): 50..75, 25..50.
    for y in 25..75 {
        for x in 25..75 {
            // Make lower-left quad bright, others dark.
            let bright = (50..75).contains(&y) && (25..50).contains(&x);
            let v = if bright { 200 } else { 50 };
            img.put_pixel(x, y, image::Rgb([v, v, v]));
        }
    }
    let pa = PropertyAssertion {
        name: "ll".into(),
        region: BboxRegion::BboxLowerLeftQuadrant,
        expected: 200.0 / 255.0,
        tolerance: 0.1,
    };
    let r = eval_property(&img, &pa).unwrap();
    assert!(
        r.passed,
        "lower-left quad should sample only bright pixels, actual={}",
        r.actual
    );
}
