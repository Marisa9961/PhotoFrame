use crate::tr;
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

use crate::{
    metadata,
    settings::{Background, Settings},
};
use anyhow::{Context, Result, bail};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use effects::{row_inset, shadow_mask};
use image::{DynamicImage, Pixel, Rgba, RgbaImage, imageops};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct ComposeTimings {
    pub resize: Duration,
    pub background: Duration,
    pub shadow: Duration,
    pub photograph: Duration,
    pub text: Duration,
}
pub struct Engine {
    fonts: FontSystem,
    cache: SwashCache,
    font_path: Option<PathBuf>,
    family: Option<String>,
    timings: ComposeTimings,
    jpeg: jpeg::Encoder,
    preview_base: Option<preview::CachedBase>,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            fonts: FontSystem::new(),
            cache: SwashCache::new(),
            font_path: None,
            family: None,
            timings: ComposeTimings::default(),
            jpeg: jpeg::Encoder::default(),
            preview_base: None,
        }
    }
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
        let mut normalized = settings.clone();
        normalized.normalize();
        let full = layout(photo.image.width(), photo.image.height(), &normalized);
        let fit = (viewport.0 as f64 / full.width as f64)
            .min(viewport.1 as f64 / full.height as f64)
            .min(1.);
        let edge = (full.width.max(full.height) as f64 * fit).floor().max(1.) as u32;
        let pixels = self.compose_at_edge(photo, &normalized, Some(edge), source)?;
        // Layout rounding can change the short edge by a pixel.
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
        self.compose_at_edge(photo, settings, preview.then_some(1400), None)
    }

    fn compose_at_edge(
        &mut self,
        photo: &Photo,
        settings: &Settings,
        preview_edge: Option<u32>,
        source: Option<&Arc<Photo>>,
    ) -> Result<RgbaImage> {
        self.timings = ComposeTimings::default();
        let stage = Instant::now();
        let mut s = settings.clone();
        s.normalize();
        if self.font_path != s.font {
            self.family = if let Some(path) = &s.font {
                let bytes = std::fs::read(path).context(tr!("errors.font_read"))?;
                let ids =
                    self.fonts
                        .db_mut()
                        .load_font_source(cosmic_text::fontdb::Source::Binary(
                            std::sync::Arc::new(bytes),
                        ));
                let id = ids.first().context(tr!("errors.font_format"))?;
                self.fonts
                    .db()
                    .face(*id)
                    .and_then(|f| f.families.first())
                    .map(|f| f.0.clone())
            } else {
                None
            };
            self.font_path = s.font.clone();
        }
        let initial = layout(photo.image.width(), photo.image.height(), &s);
        let max = if let Some(edge) = preview_edge {
            edge
        } else if s.max_edge > 0 {
            s.max_edge
        } else {
            initial.width.max(initial.height)
        };
        let factor = (max as f64 / initial.width.max(initial.height) as f64).min(1.);
        let w = (photo.image.width() as f64 * factor).round().max(1.) as u32;
        let h = (photo.image.height() as f64 * factor).round().max(1.) as u32;
        let l = layout(w, h, &s);
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
            let fg = photo
                .image
                .resize_exact(w, h, imageops::FilterType::Lanczos3)
                .to_rgba8();
            self.timings.resize = stage.elapsed();
            let stage = Instant::now();
            let mut canvas = match s.background {
                Background::Solid => RgbaImage::from_pixel(
                    l.width,
                    l.height,
                    Rgba([s.color[0], s.color[1], s.color[2], 255]),
                ),
                Background::Blur => {
                    // Blur a reduced background to keep full-resolution batch export responsive.
                    let bg_scale = (480. / l.width.max(l.height) as f32).min(1.);
                    let bg = photo.image.resize_to_fill(
                        (l.width as f32 * bg_scale).max(1.) as u32,
                        (l.height as f32 * bg_scale).max(1.) as u32,
                        imageops::FilterType::Triangle,
                    );
                    let bg = bg.blur(s.blur as f32 * 0.25);
                    let mut bg = bg
                        .resize_exact(l.width, l.height, imageops::FilterType::Triangle)
                        .to_rgba8();
                    // Dim only the backdrop: the photograph keeps its original exposure and color.
                    for pixel in bg.pixels_mut() {
                        for channel in &mut pixel.0[..3] {
                            *channel = (u32::from(*channel) * (100 - s.background_dim) / 100) as u8;
                        }
                    }
                    bg
                }
            };
            let radius = (w.min(h) as f32 * s.radius as f32 / 100.).round() as u32;
            self.timings.background = stage.elapsed();
            let stage = Instant::now();
            let opaque =
                !photo.image.color().has_alpha() || fg.pixels().all(|pixel| pixel[3] == 255);
            if s.shadow {
                let mask = shadow_mask(&l, radius);
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
            self.timings.shadow = stage.elapsed();
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
        if l.footer > 0 {
            let scale = w.max(h) as f32 / 1000.;
            let font_size = (s.font_size as f32 * scale).max(4.);
            let title = metadata::expand(&s.title, &photo.fields, &s);
            let subtitle = metadata::expand(&s.subtitle, &photo.fields, &s);
            let mut left = l.x;
            let bottom = l.y + h;
            // Extra canvas space from an aspect ratio must not push captions away from the photo.
            let region = (l.footer + w.max(h) * s.padding / 100).min(l.height - bottom);
            if let Some(path) = &s.logo {
                let logo = image::open(path).context(tr!("errors.logo"))?;
                let side = (region as f32 * 0.45).max(1.) as u32;
                let logo = logo.thumbnail(side.min(w / 4).max(1), side);
                imageops::overlay(
                    &mut canvas,
                    &logo,
                    left as i64,
                    (bottom + (region - logo.height()) / 2) as i64,
                );
                left += logo.width() + (16. * scale) as u32;
            }
            let line_height = font_size * 1.4;
            let gap = s.line_gap as f32 * scale;
            let total = if subtitle.is_empty() {
                line_height
            } else {
                line_height * 1.8 + gap
            };
            let top = bottom as f32 + ((region as f32 - total) / 2.).max(0.);
            let available = (l.x + w).saturating_sub(left).max(1);
            self.text(
                &mut canvas,
                &title,
                left,
                top as u32,
                available,
                font_size,
                s.text_color,
                s.align,
                s.font_bold,
                s.font_italic,
                s.font_family.as_deref(),
            );
            if !subtitle.is_empty() {
                self.text(
                    &mut canvas,
                    &subtitle,
                    left,
                    (top + line_height + gap) as u32,
                    available,
                    font_size * 0.72,
                    s.text_color,
                    s.align,
                    s.font_bold,
                    false,
                    s.font_family.as_deref(),
                );
            }
        }
        self.timings.text = stage.elapsed();
        if canvas.width().max(canvas.height()) > max {
            // Integer rounding of the photo and margins must not exceed the requested cap.
            canvas = DynamicImage::ImageRgba8(canvas)
                .resize(max, max, imageops::FilterType::Lanczos3)
                .to_rgba8();
        }
        Ok(canvas)
    }
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        image: &mut RgbaImage,
        text: &str,
        x: u32,
        y: u32,
        width: u32,
        size: f32,
        color: [u8; 3],
        align: u32,
        bold: bool,
        italic: bool,
        family: Option<&str>,
    ) {
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.4));
        buf.set_size(&mut self.fonts, Some(width as f32), Some(size * 1.5));
        buf.set_wrap(&mut self.fonts, Wrap::None);
        let attrs = Attrs::new()
            .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
            .style(if italic { Style::Italic } else { Style::Normal })
            .family(
                self.family
                    .as_deref()
                    .map(Family::Name)
                    .or_else(|| {
                        family.map(|name| match name {
                            "serif" => Family::Serif,
                            "monospace" => Family::Monospace,
                            "sans-serif" => Family::SansSerif,
                            other => Family::Name(other),
                        })
                    })
                    .unwrap_or(Family::SansSerif),
            );
        buf.set_text(&mut self.fonts, text, &attrs, Shaping::Advanced);
        buf.shape_until_scroll(&mut self.fonts, false);
        let text_w = buf.layout_runs().map(|r| r.line_w).fold(0., f32::max);
        // Shrink long lines instead of letting them overlap the photo or edge.
        if text_w > width as f32 {
            let scaled = size * width as f32 / text_w;
            buf.set_metrics(&mut self.fonts, Metrics::new(scaled, scaled * 1.4));
            buf.shape_until_scroll(&mut self.fonts, false);
        }
        let actual = buf.layout_runs().map(|r| r.line_w).fold(0., f32::max);
        let offset = match align {
            0 => 0.,
            2 => (width as f32 - actual).max(0.),
            _ => (width as f32 - actual).max(0.) / 2.,
        };
        buf.draw(
            &mut self.fonts,
            &mut self.cache,
            Color::rgb(color[0], color[1], color[2]),
            |dx, dy, _, _, c| {
                let xx = x as i64 + dx as i64 + offset as i64;
                let yy = y as i64 + dy as i64;
                if xx >= x as i64
                    && xx < (x + width) as i64
                    && xx < image.width() as i64
                    && yy >= 0
                    && yy < image.height() as i64
                {
                    image.get_pixel_mut(xx as u32, yy as u32).blend(&Rgba([
                        c.r(),
                        c.g(),
                        c.b(),
                        c.a(),
                    ]));
                }
            },
        );
    }
}

#[cfg(test)]
mod tests;
