use crate::tr;
mod base;
mod caption;
mod effects;
mod export;
mod jpeg;
mod layout;
mod photo;
mod preview;

pub use export::{ExportReport, ExportTimings, encode_jpeg, export, export_profiled, jpeg_backend};
pub use layout::{Layout, layout};
pub use photo::{Photo, load, load_preview};
pub use preview::PreviewCache;

use crate::settings::Settings;
use anyhow::{Result, bail};
use effects::row_inset;
use image::{DynamicImage, Pixel, Rgba, RgbaImage, imageops};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Preview stages may overlap; their durations must not be summed as wall time.
#[derive(Clone, Debug, Default)]
pub struct ComposeTimings {
    pub resize: Duration,
    pub background: Duration,
    pub shadow: Duration,
    pub photograph: Duration,
    pub text: Duration,
}
#[derive(Default)]
pub struct Engine {
    captions: caption::CaptionRenderer,
    timings: ComposeTimings,
    jpeg: jpeg::Encoder,
    preview_base: Option<preview::CachedBase>,
}
impl Engine {
    /// Render for the physical pixels available to the preview, independent of export size.
    pub fn compose_preview(
        &mut self,
        photo: &Photo,
        settings: &Settings,
        viewport: (u32, u32),
    ) -> Result<RgbaImage> {
        self.compose_preview_inner(photo, settings, viewport, None)
    }

    /// Reuse a bounded, caption-free base for an immutable decoded preview photo.
    /// Switching photos or changing base geometry/effects replaces the single entry.
    pub fn compose_cached_preview(
        &mut self,
        photo: &Arc<Photo>,
        settings: &Settings,
        viewport: (u32, u32),
    ) -> Result<RgbaImage> {
        self.compose_preview_inner(photo, settings, viewport, Some(photo))
    }

    fn compose_preview_inner(
        &mut self,
        photo: &Photo,
        settings: &Settings,
        viewport: (u32, u32),
        source: Option<&Arc<Photo>>,
    ) -> Result<RgbaImage> {
        anyhow::ensure!(
            viewport.0 > 0 && viewport.1 > 0,
            tr!("errors.viewport_size")
        );
        let pixels = self.compose_at_edge(photo, settings, Some(viewport), source)?;
        // Only exceptionally tiny viewports cannot contain even a 1 px photo and its frame.
        if pixels.width() > viewport.0 || pixels.height() > viewport.1 {
            Ok(DynamicImage::ImageRgba8(pixels)
                .resize(viewport.0, viewport.1, imageops::FilterType::Lanczos3)
                .into_rgba8())
        } else {
            Ok(pixels)
        }
    }

    pub fn compose(
        &mut self,
        photo: &Photo,
        settings: &Settings,
        preview: bool,
    ) -> Result<RgbaImage> {
        self.compose_at_edge(photo, settings, preview.then_some((1400, 1400)), None)
    }

    fn compose_at_edge(
        &mut self,
        photo: &Photo,
        settings: &Settings,
        preview_viewport: Option<(u32, u32)>,
        source: Option<&Arc<Photo>>,
    ) -> Result<RgbaImage> {
        self.timings = ComposeTimings::default();
        let stage = Instant::now();
        let mut s = settings.clone();
        s.normalize();
        self.captions.prepare_font(s.font.as_deref())?;
        let initial = layout(photo.image.width(), photo.image.height(), &s);
        let max = if s.max_edge > 0 {
            s.max_edge
        } else {
            initial.width.max(initial.height)
        };
        let l = if let Some(viewport) = preview_viewport {
            layout::preview_layout(photo.image.width(), photo.image.height(), &s, viewport)
        } else {
            let factor = (max as f64 / initial.width.max(initial.height) as f64).min(1.);
            layout(
                (photo.image.width() as f64 * factor).round().max(1.) as u32,
                (photo.image.height() as f64 * factor).round().max(1.) as u32,
                &s,
            )
        };
        let (w, h) = (l.photo_w, l.photo_h);
        if u64::from(l.width) * u64::from(l.height) > 120_000_000 {
            bail!(tr!("errors.output_limit"));
        }
        let key = preview::BaseKey::new(&l, &s);
        let cached = source.and_then(|source| {
            self.preview_base
                .as_ref()
                .filter(|base| Arc::ptr_eq(source, &base.photo) && base.key == key)
        });
        let mut canvas = if let Some(base) = cached {
            base.pixels.clone()
        } else {
            // Release obsolete buffers before allocating their replacement. Export never caches.
            self.preview_base = None;
            let setup = stage.elapsed();
            let layers = base::prepare(photo, &s, &l, preview_viewport.is_some());
            let fg = layers.foreground;
            let mut canvas = layers.canvas;
            self.timings = layers.timings;
            self.timings.resize += setup;
            let radius = (w.min(h) as f32 * s.radius as f32 / 100.).round() as u32;
            let stage = Instant::now();
            let opaque =
                !photo.image.color().has_alpha() || fg.pixels().all(|pixel| pixel[3] == 255);
            if let Some(mask) = layers.shadow {
                for (y, (row, alpha)) in canvas
                    .as_mut()
                    .chunks_exact_mut(l.width as usize * 4)
                    .zip(mask.as_raw().chunks_exact(l.width as usize))
                    .enumerate()
                {
                    let covered = if opaque && y >= l.y as usize && y < (l.y + h) as usize {
                        let inset = row_inset(y as u32 - l.y, w, h, radius);
                        (l.x + inset, l.x + w - inset)
                    } else {
                        (0, 0)
                    };
                    for (x, (pixel, &alpha)) in
                        row.as_chunks_mut::<4>().0.iter_mut().zip(alpha).enumerate()
                    {
                        if alpha > 0 && !(x >= covered.0 as usize && x < covered.1 as usize) {
                            Rgba::from_slice_mut(pixel).blend(&Rgba([0, 0, 0, alpha]));
                        }
                    }
                }
            }
            self.timings.shadow += stage.elapsed();
            let stage = Instant::now();
            let width = l.width as usize * 4;
            for y in 0..h {
                let inset = row_inset(y, w, h, radius);
                let begin = (y * w + inset) as usize * 4;
                let length = (w - 2 * inset) as usize * 4;
                let target = (l.y + y) as usize * width + (l.x + inset) as usize * 4;
                let destination = &mut canvas.as_mut()[target..target + length];
                let source = &fg.as_raw()[begin..begin + length];
                if opaque {
                    destination.copy_from_slice(source);
                } else {
                    for (pixel, source) in destination
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .zip(source.as_chunks::<4>().0)
                    {
                        Rgba::from_slice_mut(pixel).blend(Rgba::from_slice(source));
                    }
                }
            }
            drop(fg);
            self.timings.photograph = stage.elapsed();
            if let Some(source) = source.filter(|_| canvas.as_raw().len() <= preview::BASE_BUDGET) {
                self.preview_base = Some(preview::CachedBase {
                    photo: source.clone(),
                    key,
                    pixels: canvas.clone(),
                });
            }
            canvas
        };
        let stage = Instant::now();
        self.captions.draw(photo, &s, &l, &mut canvas)?;
        self.timings.text = stage.elapsed();
        if preview_viewport.is_none() && canvas.width().max(canvas.height()) > max {
            // Integer rounding of the photo and margins must not exceed the requested cap.
            canvas = DynamicImage::ImageRgba8(canvas)
                .resize(max, max, imageops::FilterType::Lanczos3)
                .to_rgba8();
        }
        Ok(canvas)
    }
}

#[cfg(test)]
mod tests;
