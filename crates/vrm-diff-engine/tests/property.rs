use vrm_diff_engine::property::{eval_property, BboxRegion, PropertyAssertion, PropertyError};

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

#[test]
fn center_strip_horizontal_samples_middle_band() {
    // Avatar bbox (20,20,80,80) on a 100×100 image. Striped pattern: alternating
    // bright (≈1.0) and dark (≈0.0) rows. Average luminance over a horizontal
    // strip through the middle should be ≈0.5.
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    for y in 20..=80 {
        let bright = y % 2 == 0;
        let v = if bright { 255 } else { 0 };
        for x in 20..=80 {
            img.put_pixel(x, y, image::Rgb([v, v, v]));
        }
    }
    let pa = PropertyAssertion {
        name: "horiz_strip".into(),
        region: BboxRegion::BboxCenterStripHorizontal,
        expected: 0.5,
        tolerance: 0.1,
    };
    let r = eval_property(&img, &pa).unwrap();
    assert!(
        r.passed,
        "horizontal strip over alternating-row pattern should average ~0.5, actual={}",
        r.actual
    );
}

#[test]
fn center_strip_vertical_samples_middle_band() {
    // Avatar bbox (20,20,80,80) on a 100×100 image. Striped pattern: alternating
    // bright/dark columns. Average luminance over a vertical strip through the
    // middle should be ≈0.5.
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    for y in 20..=80 {
        for x in 20..=80 {
            let bright = x % 2 == 0;
            let v = if bright { 255 } else { 0 };
            img.put_pixel(x, y, image::Rgb([v, v, v]));
        }
    }
    let pa = PropertyAssertion {
        name: "vert_strip".into(),
        region: BboxRegion::BboxCenterStripVertical,
        expected: 0.5,
        tolerance: 0.1,
    };
    let r = eval_property(&img, &pa).unwrap();
    assert!(
        r.passed,
        "vertical strip over alternating-column pattern should average ~0.5, actual={}",
        r.actual
    );
}

#[test]
fn center_strip_horizontal_does_not_underflow_on_tiny_origin_bbox() {
    // Regression test: prior to the fix, a small avatar bbox near the image
    // origin caused `mid_y - strip_y` to underflow u32 (panic in debug, panic
    // on get_pixel in release). The avatar here is a 2×2 bright square at the
    // top-left corner, so bbox = (0, 0, 1, 1) and mid_y = 0, strip_y = 1.
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    img.put_pixel(0, 0, image::Rgb([255, 255, 255]));
    img.put_pixel(1, 0, image::Rgb([255, 255, 255]));
    img.put_pixel(0, 1, image::Rgb([255, 255, 255]));
    img.put_pixel(1, 1, image::Rgb([255, 255, 255]));

    let pa = PropertyAssertion {
        name: "tiny".into(),
        region: BboxRegion::BboxCenterStripHorizontal,
        expected: 1.0,
        tolerance: 0.05,
    };
    // Must not panic.
    let r = eval_property(&img, &pa).unwrap();
    assert!(r.actual.is_finite(), "actual must be a real number");
    assert!(
        r.passed,
        "tiny-bbox horizontal strip should sample the bright avatar, actual={}",
        r.actual
    );
}

#[test]
fn empty_region_returns_error_when_region_falls_outside_avatar() {
    // Avatar bbox = (10,10,15,15) but actual avatar pixels live only in the
    // lower-right triangle of that bbox. Specifically, place pixels at
    // (10,15), (15,10), (13,13), (14,14), (15,15) so the bbox spans
    // (10,10)-(15,15). The upper-left quadrant of that bbox is (10,10)-(12,12),
    // which contains zero avatar pixels — only magenta. eval_property must
    // return Err(EmptyRegion).
    let mut img = image::RgbImage::from_pixel(100, 100, image::Rgb([255, 0, 255]));
    img.put_pixel(10, 15, image::Rgb([200, 200, 200]));
    img.put_pixel(15, 10, image::Rgb([200, 200, 200]));
    img.put_pixel(13, 13, image::Rgb([200, 200, 200]));
    img.put_pixel(14, 14, image::Rgb([200, 200, 200]));
    img.put_pixel(15, 15, image::Rgb([200, 200, 200]));

    let pa = PropertyAssertion {
        name: "ul_outside".into(),
        region: BboxRegion::BboxUpperLeftQuadrant,
        expected: 0.0,
        tolerance: 0.05,
    };
    let err = eval_property(&img, &pa).expect_err("expected EmptyRegion error");
    assert!(
        matches!(err, PropertyError::EmptyRegion),
        "expected PropertyError::EmptyRegion, got {err:?}"
    );
}
