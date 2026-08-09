//! Multi-format image decoder (PNG, JPEG, WebP, GIF Frame 0).
//! Excludes SVG to avoid heavy external dependencies.

use anyhow::{Context, Result, bail};
use image::{AnimationDecoder, ImageFormat, codecs::gif::GifDecoder};
pub use image::DynamicImage;
use std::io::Cursor;

/// Decodes binary image bytes (PNG, JPEG, WebP, GIF) into an unscaled `DynamicImage`.
/// For Animated GIFs, Frame 0 is extracted immediately for zero-lag static rendering.
pub fn decode_image_bytes(bytes: &[u8]) -> Result<DynamicImage> {
    if bytes.is_empty() {
        bail!("empty image byte stream");
    }

    // Try guessing format from image header magic bytes
    let format = image::guess_format(bytes).unwrap_or(ImageFormat::Jpeg);

    match format {
        ImageFormat::Gif => {
            let cursor = Cursor::new(bytes);
            if let Ok(decoder) = GifDecoder::new(cursor) {
                let mut frames = decoder.into_frames();
                if let Some(Ok(first_frame)) = frames.next() {
                    return Ok(DynamicImage::ImageRgba8(first_frame.into_buffer()));
                }
            }
            // Fall back to standard memory loader if GifDecoder fails
            image::load_from_memory(bytes).context("failed to decode GIF image")
        }
        _ => image::load_from_memory(bytes)
            .with_context(|| format!("failed to decode {format:?} image bytes")),
    }
}

/// Represents a raw RGBA pixel cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Extract RGBA matrix from DynamicImage.
pub fn to_rgba_matrix(img: &DynamicImage) -> Vec<Vec<RgbaPixel>> {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut matrix = Vec::with_capacity(height as usize);

    for y in 0..height {
        let mut row = Vec::with_capacity(width as usize);
        for x in 0..width {
            let p = rgba.get_pixel(x, y);
            row.push(RgbaPixel {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            });
        }
        matrix.push(row);
    }
    matrix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_bytes() {
        assert!(decode_image_bytes(&[]).is_err());
    }

    #[test]
    fn test_decode_invalid_bytes() {
        assert!(decode_image_bytes(b"not an image").is_err());
    }

    #[test]
    fn test_decode_png() {
        use image::{ImageBuffer, Rgba};
        let mut imgbuf = ImageBuffer::new(2, 2);
        for (_x, _y, pixel) in imgbuf.enumerate_pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        let mut encoded = Vec::new();
        let dynamic = DynamicImage::ImageRgba8(imgbuf);
        dynamic
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
            .unwrap();

        let res = decode_image_bytes(&encoded);
        assert!(res.is_ok(), "Decoding PNG bytes failed");
        let decoded = res.unwrap();
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);

        let matrix = to_rgba_matrix(&decoded);
        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0].len(), 2);
        assert_eq!(matrix[0][0].r, 255);
        assert_eq!(matrix[0][0].g, 0);
        assert_eq!(matrix[0][0].b, 0);
    }
}
