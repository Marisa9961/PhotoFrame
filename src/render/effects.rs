use super::Layout;
use image::imageops;

pub(super) fn inside(x: u32, y: u32, w: u32, h: u32, r: u32) -> bool {
    let r = r.min(w / 2).min(h / 2) as i64;
    let dx = (r - x as i64).max(x as i64 - (w as i64 - 1 - r)).max(0);
    let dy = (r - y as i64).max(y as i64 - (h as i64 - 1 - r)).max(0);
    dx * dx + dy * dy <= r * r
}

pub(super) fn row_inset(y: u32, w: u32, h: u32, radius: u32) -> u32 {
    let radius = radius.min(w / 2).min(h / 2);
    if radius == 0 || (y >= radius && y < h - radius) {
        return 0;
    }
    let (mut lo, mut hi) = (0, radius);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if inside(mid, y, w, h, radius) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

// Rasterize the shadow directly at the blur resolution, rather than allocating a full RGBA mask.
pub(super) fn shadow_mask(layout: &Layout, radius: u32) -> image::GrayImage {
    let scale = (480. / layout.width.max(layout.height) as f32).min(1.);
    let width = (layout.width as f32 * scale).max(1.) as u32;
    let height = (layout.height as f32 * scale).max(1.) as u32;
    let sx = layout.width as f32 / width as f32;
    let sy = layout.height as f32 / height as f32;
    let offset = (layout.photo_w.max(layout.photo_h) / 180).max(1);
    let r = radius.min(layout.photo_w / 2).min(layout.photo_h / 2) as f32;
    let w = layout.photo_w as f32;
    let h = layout.photo_h as f32;
    let mut mask = image::GrayImage::new(width, height);
    for (x, y, pixel) in mask.enumerate_pixels_mut() {
        let mut hits = 0;
        for sample_y in 0..4 {
            for sample_x in 0..4 {
                let px = (x as f32 + (sample_x as f32 + 0.5) / 4.) * sx - 0.5 - layout.x as f32;
                let py = (y as f32 + (sample_y as f32 + 0.5) / 4.) * sy
                    - 0.5
                    - (layout.y + offset) as f32;
                let dx = (r - px).max(px - (w - 1. - r)).max(0.);
                let dy = (r - py).max(py - (h - 1. - r)).max(0.);
                if px >= -0.5
                    && px < w - 0.5
                    && py >= -0.5
                    && py < h - 0.5
                    && (r == 0. || dx * dx + dy * dy <= r * r)
                {
                    hits += 1;
                }
            }
        }
        pixel[0] = (hits * 110 / 16) as u8;
    }
    let mask = imageops::blur(
        &mask,
        layout.photo_w.max(layout.photo_h) as f32 / 65. * scale,
    );
    imageops::resize(
        &mask,
        layout.width,
        layout.height,
        imageops::FilterType::Triangle,
    )
}
