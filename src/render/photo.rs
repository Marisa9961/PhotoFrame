use crate::metadata;
use crate::tr;
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageDecoder, ImageReader, imageops::FilterType};
use std::{collections::BTreeMap, path::Path};

pub struct Photo {
    pub image: DynamicImage,
    pub fields: BTreeMap<String, String>,
}
pub fn load(path: &Path) -> Result<Photo> {
    load_at_edge(path, None)
}

/// Load an oriented, bounded image for previews; export always uses `load`.
pub fn load_preview(path: &Path, max_edge: u32) -> Result<Photo> {
    anyhow::ensure!(max_edge > 0, tr!("errors.preview_size"));
    load_at_edge(path, Some(max_edge))
}

fn load_at_edge(path: &Path, max_edge: Option<u32>) -> Result<Photo> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let jpeg = reader.format() == Some(image::ImageFormat::Jpeg);
    let mut decoder = reader.into_decoder()?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > 100_000_000 {
        bail!(tr!("errors.input_limit"));
    }
    let fields = decoder
        .exif_metadata()
        .ok()
        .flatten()
        .map(|v| metadata::parse(&v))
        .unwrap_or_default();
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = if let Some(edge) = max_edge.filter(|_| jpeg) {
        // Unusual JPEG variants retain the existing image/zune-jpeg decoder.
        match decode_jpeg_preview(path, edge, (w, h)) {
            Ok(image) => image,
            Err(_) => DynamicImage::from_decoder(decoder)?,
        }
    } else {
        DynamicImage::from_decoder(decoder)?
    };
    image.apply_orientation(orientation);
    let dimensions = match orientation {
        image::metadata::Orientation::Rotate90
        | image::metadata::Orientation::Rotate270
        | image::metadata::Orientation::Rotate90FlipH
        | image::metadata::Orientation::Rotate270FlipH => (h, w),
        _ => (w, h),
    };
    if let Some(edge) = max_edge {
        if image.width().max(image.height()) > edge {
            image = image.resize(edge, edge, FilterType::Lanczos3);
        }
        // Keep cached previews at 8 bits per channel, including high-precision TIFF inputs.
        image = if image.color().has_alpha() {
            DynamicImage::ImageRgba8(image.into_rgba8())
        } else {
            DynamicImage::ImageRgb8(image.into_rgb8())
        };
    }
    let mut fields: BTreeMap<String, String> = fields;
    fields.insert(
        "filename".into(),
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    );
    fields.insert("width".into(), dimensions.0.to_string());
    fields.insert("height".into(), dimensions.1.to_string());
    Ok(Photo { image, fields })
}

fn decode_jpeg_preview(path: &Path, edge: u32, dimensions: (u32, u32)) -> Result<DynamicImage> {
    let bytes = std::fs::read(path)?;
    decode_jpeg_pixels(&bytes, edge, dimensions)
}

fn decode_jpeg_pixels(bytes: &[u8], edge: u32, dimensions: (u32, u32)) -> Result<DynamicImage> {
    let mut decoder = libjpeg_turbo_rs::Decoder::new_with_limits(
        bytes,
        libjpeg_turbo_rs::DecodeLimits {
            max_pixels: 100_000_000,
            ..Default::default()
        },
    )?;
    let header = decoder.header();
    anyhow::ensure!(
        (u32::from(header.width), u32::from(header.height)) == dimensions,
        tr!("errors.photo_changed")
    );
    let scaling = jpeg_preview_scale(
        header.width.max(header.height) as usize,
        edge,
        header.is_lossless,
    );
    decoder.set_scale(scaling);
    decoder.set_output_format(libjpeg_turbo_rs::PixelFormat::Rgb);
    let decoded = decoder.decode_image()?;
    anyhow::ensure!(
        decoded.precision == 8 && decoded.pixel_format == libjpeg_turbo_rs::PixelFormat::Rgb,
        tr!("errors.jpeg_rgb")
    );
    let image =
        image::RgbImage::from_raw(decoded.width as u32, decoded.height as u32, decoded.data)
            .context(tr!("errors.jpeg_pixels"))?;
    Ok(DynamicImage::ImageRgb8(image))
}

fn jpeg_preview_scale(edge: usize, target: u32, lossless: bool) -> libjpeg_turbo_rs::ScalingFactor {
    use libjpeg_turbo_rs::ScalingFactor as Scale;
    if !lossless {
        for denom in [8, 4, 2] {
            let scale = Scale::new(1, denom);
            if scale.scale_dim(edge) >= target as usize {
                return scale;
            }
        }
    }
    Scale::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_jpeg_preview_allocates_scaled_pixels_before_resampling() {
        let image = image::RgbImage::from_pixel(1600, 1200, image::Rgb([100, 120, 140]));
        let bytes = super::super::encode_jpeg(image, 85).unwrap();
        let preview = decode_jpeg_pixels(&bytes, 320, (1600, 1200)).unwrap();
        assert_eq!((preview.width(), preview.height()), (400, 300));
        assert_eq!(preview.as_bytes().len(), 400 * 300 * 3);
        let pixel = preview.into_rgb8().get_pixel(0, 0).0;
        assert!(pixel[0].abs_diff(100) < 3);
        assert!(pixel[1].abs_diff(120) < 3);
        assert!(pixel[2].abs_diff(140) < 3);
    }

    #[test]
    fn c_generated_progressive_and_cmyk_inputs_use_scaled_rust_decoding() {
        for data in [
            include_bytes!("../../tests/fixtures/progressive-420.jpg").as_slice(),
            include_bytes!("../../tests/fixtures/cmyk.jpg").as_slice(),
        ] {
            let preview = decode_jpeg_pixels(data, 80, (257, 129)).unwrap();
            assert_eq!((preview.width(), preview.height()), (129, 65));
            let full = image::load_from_memory(data)
                .unwrap()
                .resize(129, 65, FilterType::Triangle)
                .into_rgb8();
            let preview = preview.into_rgb8();
            let error: u64 = full
                .as_raw()
                .iter()
                .zip(preview.as_raw())
                .map(|(a, b)| u64::from(a.abs_diff(*b)))
                .sum();
            assert!(error as f64 / (full.as_raw().len() as f64) < 6.0);
        }
        assert!(decode_jpeg_pixels(b"not a jpeg", 80, (257, 129)).is_err());
    }
}
