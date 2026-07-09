//! Image decoding tests (pure CPU path; GPU upload is exercised by running a game).

use std::io::Cursor;

use fulcrum_render::texture::decode_rgba;
use image::{ImageFormat, Rgba, RgbaImage};

/// Encode a small in-memory PNG fixture: 3×2, uniquely colored pixels.
fn png_fixture() -> Vec<u8> {
    let mut img = RgbaImage::new(3, 2);
    for (i, pixel) in img.pixels_mut().enumerate() {
        *pixel = Rgba([i as u8 * 40, 255 - i as u8 * 40, 7, 255]);
    }
    let mut bytes = Cursor::new(Vec::new());
    img.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn decodes_png_with_correct_dimensions_and_pixels() {
    let (pixels, width, height) = decode_rgba("fixture.png", &png_fixture()).unwrap();
    assert_eq!((width, height), (3, 2));
    assert_eq!(pixels.len(), 3 * 2 * 4);
    // First pixel: i = 0 -> rgba(0, 255, 7, 255).
    assert_eq!(&pixels[0..4], &[0, 255, 7, 255]);
    // Last pixel: i = 5 -> rgba(200, 55, 7, 255).
    assert_eq!(&pixels[20..24], &[200, 55, 7, 255]);
}

#[test]
fn garbage_bytes_error_names_the_path() {
    let err = decode_rgba("bad.png", b"not an image").unwrap_err();
    assert!(err.to_string().contains("bad.png"), "error was: {err}");
}
