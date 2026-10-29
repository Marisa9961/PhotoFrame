use super::{Layout, Photo};
use crate::{metadata, settings::Settings, tr};
use anyhow::{Context, Result};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use image::{Pixel, Rgba, RgbaImage, imageops};
use std::path::{Path, PathBuf};

/// Owns font resources and caption/logo layout, independently of base composition.
pub(super) struct CaptionRenderer {
    fonts: FontSystem,
    cache: SwashCache,
    font_path: Option<PathBuf>,
    family: Option<String>,
}

impl Default for CaptionRenderer {
    fn default() -> Self {
        Self {
            fonts: FontSystem::new(),
            cache: SwashCache::new(),
            font_path: None,
            family: None,
        }
    }
}

impl CaptionRenderer {
    pub fn prepare_font(&mut self, path: Option<&Path>) -> Result<()> {
        if self.font_path.as_deref() != path {
            self.family = if let Some(path) = path {
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
            self.font_path = path.map(Path::to_path_buf);
        }
        Ok(())
    }

    pub fn draw(
        &mut self,
        photo: &Photo,
        s: &Settings,
        l: &Layout,
        canvas: &mut RgbaImage,
    ) -> Result<()> {
        let (w, h) = (l.photo_w, l.photo_h);
        if l.footer > 0 {
            let scale = w.max(h) as f32 / 1000.;
            let font_size = (s.font_size as f32 * scale).max(4.);
            let title = metadata::expand(&s.title, &photo.fields, s);
            let subtitle = metadata::expand(&s.subtitle, &photo.fields, s);
            let mut left = l.x;
            let bottom = l.y + h;
            // Extra canvas space from an aspect ratio must not push captions away from the photo.
            let region = (l.footer + w.max(h) * s.padding / 100).min(l.height - bottom);
            if let Some(path) = &s.logo {
                let logo = image::open(path).context(tr!("errors.logo"))?;
                let side = (region as f32 * 0.45).max(1.) as u32;
                let logo = logo.thumbnail(side.min(w / 4).max(1), side);
                imageops::overlay(
                    canvas,
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
                canvas,
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
                    canvas,
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
        Ok(())
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
