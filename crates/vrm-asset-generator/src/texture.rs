//! Procedural test textures + helpers for embedding them in glTF as
//! data URIs. Used by the KHR_texture_transform sweep to give the
//! sphere mesh a visually-distinct surface so UV transforms are
//! observable in the rendered output.

use base64::Engine;
use image::{ImageFormat, RgbaImage};

/// Render a 16×16 RGBA checkerboard split into four colored quadrants:
/// top-left red, top-right green, bottom-left blue, bottom-right yellow.
/// Picked to give a corpus-readable UV-orientation indicator on the
/// rendered sphere — yaw rotation of the texture mapping shifts the
/// red/green/blue/yellow boundary visibly, and any axis flip swaps
/// channels in a recognisable way. 16×16 is small enough that the PNG
/// fits in ~200 bytes (cheap to embed as a data URI) while still being
/// non-trivially compressible.
pub fn quadrant_checkerboard_16() -> RgbaImage {
    let mut img = RgbaImage::new(16, 16);
    for y in 0..16u32 {
        for x in 0..16u32 {
            let color = match (x < 8, y < 8) {
                (true, true) => [220, 30, 30, 255],    // top-left  = red
                (false, true) => [30, 200, 30, 255],   // top-right = green
                (true, false) => [30, 30, 220, 255],   // bot-left  = blue
                (false, false) => [220, 220, 30, 255], // bot-right = yellow
            };
            img.put_pixel(x, y, image::Rgba(color));
        }
    }
    img
}

/// PNG-encode an image and wrap it in a `data:` URI suitable for
/// `glTF.images[*].uri`. The base64 dance avoids touching the GLB
/// binary chunk — keeps texture additions JSON-only, no new
/// bufferViews to align or accessor maths to update.
pub fn image_as_data_uri(img: &RgbaImage) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(512);
    {
        let mut cursor = std::io::Cursor::new(&mut bytes);
        img.write_to(&mut cursor, ImageFormat::Png)
            .expect("PNG encode of a 16x16 RGBA buffer cannot fail");
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    format!("data:image/png;base64,{b64}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkerboard_has_four_distinct_quadrant_colors() {
        let img = quadrant_checkerboard_16();
        // Sample one pixel from the center of each quadrant.
        let tl = img.get_pixel(3, 3).0;
        let tr = img.get_pixel(11, 3).0;
        let bl = img.get_pixel(3, 11).0;
        let br = img.get_pixel(11, 11).0;
        assert_eq!(tl, [220, 30, 30, 255]);
        assert_eq!(tr, [30, 200, 30, 255]);
        assert_eq!(bl, [30, 30, 220, 255]);
        assert_eq!(br, [220, 220, 30, 255]);
        // Sanity-check uniqueness — every quadrant must be distinguishable.
        let all = [tl, tr, bl, br];
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert_ne!(all[i], all[j], "quadrants {i} and {j} must differ");
            }
        }
    }

    #[test]
    fn data_uri_has_correct_prefix_and_decodes_to_valid_png() {
        let img = quadrant_checkerboard_16();
        let uri = image_as_data_uri(&img);
        assert!(uri.starts_with("data:image/png;base64,"));
        let b64 = uri.strip_prefix("data:image/png;base64,").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("data URI must round-trip through base64 decode");
        // PNG signature: 137 80 78 71 13 10 26 10
        assert_eq!(&decoded[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}
