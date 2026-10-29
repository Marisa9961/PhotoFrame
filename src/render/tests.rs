use super::effects::inside;
use super::*;
use crate::settings::Ratio;

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
fn round_corners() {
    assert!(!inside(0, 0, 100, 100, 10));
    assert!(inside(50, 50, 100, 100, 10));
}
#[test]
fn row_spans_match_the_original_pixel_mask() {
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
