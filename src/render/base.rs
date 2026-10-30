use super::{ComposeTimings, Layout, Photo, effects::shadow_mask};
use crate::settings::{Background, Settings};
use image::{GrayImage, Rgba, RgbaImage, imageops};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

pub(super) struct Layers {
    pub foreground: RgbaImage,
    pub canvas: RgbaImage,
    pub shadow: Option<GrayImage>,
    pub timings: ComposeTimings,
}

pub(super) fn prepare(
    photo: &Photo,
    settings: &Settings,
    layout: &Layout,
    preview: bool,
) -> Layers {
    let worthwhile = u64::from(layout.width) * u64::from(layout.height) >= 256 * 256
        && (settings.background == Background::Blur || settings.shadow);
    let pool = if preview && worthwhile {
        preview_pool()
    } else {
        None
    };
    prepare_with_pool(photo, settings, layout, pool)
}

fn preview_pool() -> Option<&'static ThreadPool> {
    static POOL: OnceLock<Option<ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        let workers = std::thread::available_parallelism()
            .ok()?
            .get()
            .saturating_sub(1)
            .min(3);
        if workers < 2 {
            return None;
        }
        ThreadPoolBuilder::new()
            .num_threads(workers)
            .thread_name(|index| format!("preview-compose-{index}"))
            .build()
            .ok()
    })
    .as_ref()
}

fn timed<T>(work: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = work();
    (value, started.elapsed())
}

fn prepare_with_pool(photo: &Photo, s: &Settings, l: &Layout, pool: Option<&ThreadPool>) -> Layers {
    let foreground = || {
        timed(|| {
            photo
                .image
                .resize_exact(l.photo_w, l.photo_h, imageops::FilterType::Lanczos3)
                .to_rgba8()
        })
    };
    let background = || timed(|| background(photo, s, l));
    let shadow = || {
        timed(|| {
            s.shadow.then(|| {
                let radius =
                    (l.photo_w.min(l.photo_h) as f32 * s.radius as f32 / 100.).round() as u32;
                shadow_mask(l, radius)
            })
        })
    };
    let ((foreground, resize), (canvas, background), (shadow, shadow_time)) =
        if let Some(pool) = pool {
            let (foreground, (background, shadow)) =
                pool.install(|| rayon::join(foreground, || rayon::join(background, shadow)));
            (foreground, background, shadow)
        } else {
            (foreground(), background(), shadow())
        };
    Layers {
        foreground,
        canvas,
        shadow,
        timings: ComposeTimings {
            resize,
            background,
            shadow: shadow_time,
            ..ComposeTimings::default()
        },
    }
}

fn background(photo: &Photo, s: &Settings, l: &Layout) -> RgbaImage {
    match s.background {
        Background::Solid => RgbaImage::from_pixel(
            l.width,
            l.height,
            Rgba([s.color[0], s.color[1], s.color[2], 255]),
        ),
        Background::Blur => {
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
            for pixel in bg.pixels_mut() {
                for channel in &mut pixel.0[..3] {
                    *channel = (u32::from(*channel) * (100 - s.background_dim) / 100) as u8;
                }
            }
            bg
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn parallel_layers_match_serial_pixels_for_backgrounds_shadows_and_alpha() {
        let pool = ThreadPoolBuilder::new().num_threads(3).build().unwrap();
        for alpha in [128, 255] {
            let photo = Photo {
                image: DynamicImage::ImageRgba8(RgbaImage::from_fn(160, 100, |x, y| {
                    Rgba([x as u8, y as u8, 120, alpha])
                })),
                fields: Default::default(),
            };
            for background in [Background::Solid, Background::Blur] {
                for shadow in [false, true] {
                    let settings = Settings {
                        background,
                        shadow,
                        ..Settings::default()
                    };
                    let layout = super::super::layout(120, 75, &settings);
                    let serial = prepare_with_pool(&photo, &settings, &layout, None);
                    let parallel = prepare_with_pool(&photo, &settings, &layout, Some(&pool));
                    assert_eq!(parallel.foreground, serial.foreground);
                    assert_eq!(parallel.canvas, serial.canvas);
                    assert_eq!(parallel.shadow, serial.shadow);
                }
            }
        }
    }
}
