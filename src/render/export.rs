use super::{ComposeTimings, Engine, jpeg, load};
use crate::settings::{Format, Settings};
use crate::tr;
use anyhow::{Result, bail};
use image::DynamicImage;
use std::{
    fs::OpenOptions,
    io::BufWriter,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct ExportTimings {
    pub decode: Duration,
    pub compose: ComposeTimings,
    pub encode_write: Duration,
    pub total: Duration,
    pub jpeg_encodes: u32,
}
pub struct ExportReport {
    pub path: PathBuf,
    pub timings: ExportTimings,
}

pub fn export(
    engine: &mut Engine,
    source: &Path,
    directory: &Path,
    settings: &Settings,
) -> Result<PathBuf> {
    Ok(export_profiled(engine, source, directory, settings)?.path)
}
pub fn export_profiled(
    engine: &mut Engine,
    source: &Path,
    directory: &Path,
    settings: &Settings,
) -> Result<ExportReport> {
    let total = Instant::now();
    let photo = load(source)?;
    let decode = total.elapsed();
    let image = engine.compose(&photo, settings, false)?;
    drop(photo);
    let mut timings = ExportTimings {
        decode,
        compose: engine.timings.clone(),
        ..ExportTimings::default()
    };
    let stage = Instant::now();
    std::fs::create_dir_all(directory)?;
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    for index in 0..10000 {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{index}")
        };
        let path = directory.join(format!("{stem}_frame{suffix}.{}", settings.extension()));
        let file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        };
        let result = (|| -> Result<()> {
            let mut writer = BufWriter::new(file);
            match settings.format {
                Format::Jpeg => {
                    let encoded = engine.jpeg.encode_rgba(image, settings.quality)?;
                    timings.jpeg_encodes = 1;
                    std::io::Write::write_all(&mut writer, &encoded)?;
                }
                Format::Png => DynamicImage::ImageRgba8(image)
                    .write_to(&mut writer, image::ImageFormat::Png)?,
                Format::Webp => DynamicImage::ImageRgba8(image)
                    .write_to(&mut writer, image::ImageFormat::WebP)?,
            }
            std::io::Write::flush(&mut writer)?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        timings.encode_write = stage.elapsed();
        timings.total = total.elapsed();
        return Ok(ExportReport { path, timings });
    }
    bail!(tr!("errors.output_duplicates"))
}

/// Encode once at the requested quality, preserving the supplied dimensions.
pub fn encode_jpeg(image: image::RgbImage, quality: u8) -> Result<Vec<u8>> {
    jpeg::Encoder::default().encode_rgb(image, quality)
}
pub const fn jpeg_backend() -> &'static str {
    jpeg::backend()
}
