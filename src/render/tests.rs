use super::effects::inside;
use super::*;
use crate::settings::Ratio;

fn cached_photo() -> Arc<Photo> {
    Arc::new(Photo {
        image: DynamicImage::ImageRgba8(RgbaImage::from_fn(320, 200, |x, y| {
            Rgba([x as u8, y as u8, 120, (x % 256) as u8])
        })),
        fields: [("camera".into(), "Camera A".into())].into(),
    })
}

fn assert_base_reused(engine: &Engine) {
    assert_eq!(engine.timings.resize, Duration::ZERO);
    assert_eq!(engine.timings.background, Duration::ZERO);
    assert_eq!(engine.timings.shadow, Duration::ZERO);
    assert_eq!(engine.timings.photograph, Duration::ZERO);
}

#[test]
fn cached_preview_caption_edits_match_fresh_render_without_rebuilding_base() {
    let photo = cached_photo();
    let mut engine = Engine::default();
    let mut reference = Engine::default();
    let mut settings = Settings {
        footer: 30,
        title: "Original".into(),
        subtitle: String::new(),
        ..Settings::default()
    };
    let original = engine
        .compose_cached_preview(&photo, &settings, (400, 400))
        .unwrap();
    for edit in 0..10 {
        match edit {
            0 => settings.title = "{camera}".into(),
            1 => settings.subtitle = "Second line".into(),
            2 => settings.text_color = [20, 200, 90],
            3 => settings.font_size = 42,
            4 => settings.line_gap = 8,
            5 => settings.align = 0,
            6 => settings.font_bold = !settings.font_bold,
            7 => settings.font_italic = !settings.font_italic,
            8 => settings.font_family = Some("monospace".into()),
            _ => {
                settings.force_custom = true;
                settings
                    .custom
                    .insert("camera".into(), "Edited camera".into());
                settings.uppercase = true;
            }
        }
        let cached = engine
            .compose_cached_preview(&photo, &settings, (400, 400))
            .unwrap();
        assert_base_reused(&engine);
        assert_eq!(
            cached,
            reference
                .compose_preview(&photo, &settings, (400, 400))
                .unwrap()
        );
        assert_ne!(cached, original);
    }
    // Preferences that only affect export must not invalidate the preview base.
    settings.max_edge = 100;
    settings.quality = 50;
    settings.format = crate::settings::Format::Png;
    engine
        .compose_cached_preview(&photo, &settings, (400, 400))
        .unwrap();
    assert_base_reused(&engine);
}

#[test]
fn cached_preview_invalidates_for_each_base_dependency_and_viewport() {
    let photo = cached_photo();
    let mut engine = Engine::default();
    let mut reference = Engine::default();
    let mut settings = Settings::default();
    engine
        .compose_cached_preview(&photo, &settings, (400, 400))
        .unwrap();
    for edit in 0..10 {
        match edit {
            0 => settings.padding += 2,
            1 => settings.horizontal_padding = Some(12),
            2 => settings.footer += 8,
            3 => settings.ratio = Ratio::Square,
            4 => settings.blur = 15,
            5 => settings.background_dim = 70,
            6 => settings.radius = 10,
            7 => settings.shadow = false,
            8 => settings.background = Background::Solid,
            _ => settings.color = [12, 34, 56],
        }
        let cached = engine
            .compose_cached_preview(&photo, &settings, (400, 400))
            .unwrap();
        assert!(engine.timings.background > Duration::ZERO);
        assert_eq!(
            cached,
            reference
                .compose_preview(&photo, &settings, (400, 400))
                .unwrap()
        );
    }
    let small = engine
        .compose_cached_preview(&photo, &settings, (120, 80))
        .unwrap();
    assert!(engine.timings.resize > Duration::ZERO);
    assert_eq!(
        small,
        reference
            .compose_preview(&photo, &settings, (120, 80))
            .unwrap()
    );
    engine
        .compose_cached_preview(&photo, &settings, (120, 80))
        .unwrap();
    assert_base_reused(&engine);
}

#[test]
fn cached_preview_replaces_source_and_does_not_affect_export() {
    let mut photo = cached_photo();
    let mut engine = Engine::default();
    let settings = Settings::default();
    let original = engine
        .compose_cached_preview(&photo, &settings, (400, 400))
        .unwrap();
    assert!(Arc::get_mut(&mut photo).is_none());
    let old = Arc::downgrade(&photo);
    let mut replacement = Photo {
        image: photo.image.clone(),
        fields: photo.fields.clone(),
    };
    replacement.image.as_mut_rgba8().unwrap().fill(255);
    photo = Arc::new(replacement);
    let changed = engine
        .compose_cached_preview(&photo, &settings, (400, 400))
        .unwrap();
    assert!(old.upgrade().is_none());
    assert_ne!(original, changed);
    assert_eq!(
        changed,
        Engine::default()
            .compose_preview(&photo, &settings, (400, 400))
            .unwrap()
    );
    let exported = engine.compose(&photo, &settings, false).unwrap();
    assert!(engine.preview_base.is_none());
    assert_eq!(
        exported,
        Engine::default().compose(&photo, &settings, false).unwrap()
    );
}

#[test]
fn oversized_preview_base_is_not_retained() {
    let mut photo = Arc::new(Photo {
        image: DynamicImage::ImageRgb8(image::RgbImage::new(3000, 3000)),
        fields: Default::default(),
    });
    let settings = Settings {
        background: Background::Solid,
        padding: 0,
        horizontal_padding: Some(0),
        footer: 0,
        shadow: false,
        radius: 0,
        ..Settings::default()
    };
    let mut engine = Engine::default();
    let output = engine
        .compose_cached_preview(&photo, &settings, (3000, 3000))
        .unwrap();
    assert!(output.as_raw().len() > preview::BASE_BUDGET);
    assert!(engine.preview_base.is_none());
    assert!(Arc::get_mut(&mut photo).is_some());
}

#[test]
fn preview_fits_display_pixels_without_changing_export_settings() {
    let photo = Photo {
        image: DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            2000,
            1200,
            Rgba([100, 120, 140, 255]),
        )),
        fields: Default::default(),
    };
    let settings = Settings {
        background: Background::Solid,
        padding: 0,
        horizontal_padding: Some(0),
        footer: 0,
        shadow: false,
        max_edge: 100,
        ..Settings::default()
    };
    let mut engine = Engine::default();
    let preview = engine
        .compose_preview(&photo, &settings, (1920, 1080))
        .unwrap();
    assert!(preview.width() > 1400);
    assert!(preview.width() <= 1920 && preview.height() <= 1080);
    let high_dpi = engine
        .compose_preview(&photo, &settings, (3840, 2160))
        .unwrap();
    assert_eq!(high_dpi.dimensions(), (2000, 1200));
    assert_eq!(settings.max_edge, 100);
    let small = engine
        .compose_preview(&photo, &settings, (120, 240))
        .unwrap();
    assert!(small.width() <= 120 && small.height() <= 240);
    assert!(engine.compose_preview(&photo, &settings, (0, 0)).is_err());
}
#[test]
fn ratios_contain_photo() {
    for (w, h) in [(6000, 4000), (4000, 6000), (1, 10000), (10000, 1)] {
        for ratio in [
            Ratio::Auto,
            Ratio::Square,
            Ratio::Landscape,
            Ratio::Portrait,
            Ratio::Wide,
        ] {
            let s = Settings {
                ratio,
                ..Settings::default()
            };
            let l = layout(w, h, &s);
            assert!(l.x + w <= l.width);
            assert!(l.y + h + l.footer <= l.height + 1);
            if ratio == Ratio::Square {
                assert_eq!(l.width, l.height);
            }
        }
    }
}
#[test]
fn row_spans_match_the_original_pixel_mask() {
    // Anchor the reference mask before checking every span against it.
    assert!(!inside(0, 0, 100, 100, 10));
    assert!(inside(50, 50, 100, 100, 10));
    for (w, h) in [(1, 1), (2, 3), (17, 8), (80, 60)] {
        for radius in [0, 1, 2, 8, 100] {
            for y in 0..h {
                let inset = row_inset(y, w, h, radius);
                for x in 0..w {
                    assert_eq!(x >= inset && x < w - inset, inside(x, y, w, h, radius));
                }
            }
        }
    }
}
#[test]
fn small_shadow_mask_remains_close_to_full_resolution_reference() {
    let settings = Settings::default();
    let layout = layout(1160, 772, &settings);
    let radius = 15;
    let mut reference = RgbaImage::new(layout.width, layout.height);
    let offset = (layout.photo_w.max(layout.photo_h) / 180).max(1);
    for y in 0..layout.photo_h {
        for x in 0..layout.photo_w {
            if inside(x, y, layout.photo_w, layout.photo_h, radius) {
                reference.put_pixel(layout.x + x, layout.y + y + offset, Rgba([0, 0, 0, 110]));
            }
        }
    }
    let scale = 480. / layout.width.max(layout.height) as f32;
    let reference = imageops::resize(
        &reference,
        (layout.width as f32 * scale) as u32,
        (layout.height as f32 * scale) as u32,
        imageops::FilterType::Triangle,
    );
    let reference = imageops::blur(&reference, 1160. / 65. * scale);
    let reference = imageops::resize(
        &reference,
        layout.width,
        layout.height,
        imageops::FilterType::Triangle,
    );
    let optimized = shadow_mask(&layout, radius);
    let difference = reference
        .pixels()
        .zip(optimized.pixels())
        .map(|(a, b)| u64::from(a[3].abs_diff(b[0])))
        .sum::<u64>();
    assert!(difference as f64 / (layout.width as f64 * layout.height as f64) < 1.5);
}
