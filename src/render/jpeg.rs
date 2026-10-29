//! Single-pass pure Rust JPEG encoding at the requested quality and dimensions.
use crate::tr;
use anyhow::Context;
use anyhow::Result;
use image::{DynamicImage, RgbImage, RgbaImage};

#[derive(Default)]
pub(crate) struct Encoder {}

pub(crate) const fn backend() -> &'static str {
    "libjpeg-turbo-rs (pure Rust, 4:4:4)"
}

impl Encoder {
    pub(crate) fn encode_rgba(&mut self, image: RgbaImage, quality: u8) -> Result<Vec<u8>> {
        self.encode(DynamicImage::ImageRgba8(image), quality)
    }
    pub(crate) fn encode_rgb(&mut self, image: RgbImage, quality: u8) -> Result<Vec<u8>> {
        self.encode(DynamicImage::ImageRgb8(image), quality)
    }
    fn encode(&mut self, image: DynamicImage, quality: u8) -> Result<Vec<u8>> {
        anyhow::ensure!(
            image.width() > 0 && image.height() > 0,
            tr!("errors.jpeg_size")
        );
        self.compress(&image, quality.clamp(1, 100))
    }

    fn compress(&mut self, image: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
        let (pixels, format) = match image {
            DynamicImage::ImageRgba8(image) => (
                image.as_raw().as_slice(),
                libjpeg_turbo_rs::PixelFormat::Rgba,
            ),
            DynamicImage::ImageRgb8(image) => (
                image.as_raw().as_slice(),
                libjpeg_turbo_rs::PixelFormat::Rgb,
            ),
            _ => unreachable!("JPEG encoder accepts RGB8 or RGBA8"),
        };
        // Accept RGBA directly, without an intermediate full-size RGB allocation.
        libjpeg_turbo_rs::Encoder::new(
            pixels,
            image.width() as usize,
            image.height() as usize,
            format,
        )
        .quality(quality)
        .subsampling(libjpeg_turbo_rs::Subsampling::S444)
        .optimize_huffman(false)
        .progressive(false)
        .arithmetic(false)
        .encode()
        .context(tr!("errors.jpeg_encode"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([
                (x * 3 % 256) as u8,
                (y * 5 % 256) as u8,
                ((x + y) % 256) as u8,
                ((x * y) % 256) as u8,
            ])
        })
    }

    fn segment(bytes: &[u8], marker: u8) -> &[u8] {
        assert_eq!(&bytes[..2], &[0xff, 0xd8]);
        let mut offset = 2;
        loop {
            assert_eq!(bytes[offset], 0xff);
            let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
            if bytes[offset + 1] == marker {
                return &bytes[offset + 4..offset + 2 + length];
            }
            assert_ne!(bytes[offset + 1], 0xda, "Missing JPEG header segment");
            offset += length + 2;
        }
    }

    #[test]
    fn jpeg_reuses_encoder_with_baseline_444_requested_quality_and_dimensions() {
        let mut encoder = Encoder::default();
        for (width, height, quality) in [(127, 83, 85), (19, 41, 70), (96, 65, 95), (127, 83, 85)] {
            let rgba = canvas(width, height);
            let rgb = DynamicImage::ImageRgba8(rgba.clone()).into_rgb8();
            let bytes = encoder.encode_rgba(rgba, quality).unwrap();
            let header = segment(&bytes, 0xc0); // SOF0: baseline, not progressive.
            assert_eq!(header[0], 8);
            assert_eq!(u16::from_be_bytes([header[1], header[2]]) as u32, height);
            assert_eq!(u16::from_be_bytes([header[3], header[4]]) as u32, width);
            assert_eq!(header[5], 3);
            for component in 0..3 {
                assert_eq!(header[7 + component * 3], 0x11);
            }
            // Standard quality scale: luminance DC coefficient = round(16 * scale / 100).
            let scale = if quality < 50 {
                5000 / u32::from(quality)
            } else {
                200 - u32::from(quality) * 2
            };
            assert_eq!(
                segment(&bytes, 0xdb)[1],
                ((16 * scale + 50) / 100).clamp(1, 255) as u8
            );
            assert_eq!(
                image::load_from_memory(&bytes)
                    .unwrap()
                    .into_rgb8()
                    .dimensions(),
                (width, height)
            );
            let rgb_bytes = encoder.encode_rgb(rgb, quality).unwrap();
            assert_eq!(bytes, rgb_bytes, "JPEG encoding discards alpha");
        }
    }

    #[test]
    fn jpeg_empty_input_returns_error() {
        assert!(
            Encoder::default()
                .encode_rgb(RgbImage::new(0, 0), 85)
                .is_err()
        );
    }

    #[test]
    fn jpeg_quality_matches_the_previous_c_backend_reference() {
        let pixels = RgbImage::from_fn(257, 129, |x, y| {
            image::Rgb([
                (x % 256) as u8,
                (y * 2 % 256) as u8,
                ((x + y) / 2 % 256) as u8,
            ])
        });
        let encoded = Encoder::default().encode_rgb(pixels, 85).unwrap();
        let reference =
            image::load_from_memory(include_bytes!("../../tests/fixtures/baseline-444.jpg"))
                .unwrap()
                .into_rgb8();
        let actual = image::load_from_memory(&encoded).unwrap().into_rgb8();
        let error: u64 = reference
            .as_raw()
            .iter()
            .zip(actual.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        assert!(error as f64 / (reference.as_raw().len() as f64) < 1.0);
    }
}
