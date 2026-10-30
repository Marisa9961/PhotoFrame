use crate::settings::{Ratio, Settings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub photo_w: u32,
    pub photo_h: u32,
    pub footer: u32,
}
pub fn layout(w: u32, h: u32, s: &Settings) -> Layout {
    let edge = w.max(h) as f64;
    let pad = edge * s.padding as f64 / 100.;
    let footer = edge * s.footer as f64 / 100.;
    let side = edge * s.horizontal_padding.unwrap_or(s.padding) as f64 / 100.;
    let mut cw = w as f64 + side * 2.;
    let mut ch = h as f64 + pad * 2. + footer;
    let ratio = match s.ratio {
        Ratio::Auto => None,
        Ratio::Square => Some(1.),
        Ratio::Landscape => Some(1.5),
        Ratio::Portrait => Some(2. / 3.),
        Ratio::Wide => Some(16. / 9.),
    };
    if let Some(r) = ratio {
        if cw / ch < r {
            cw = ch * r;
        } else {
            ch = cw / r;
        }
    }
    Layout {
        width: cw.ceil() as u32,
        height: ch.ceil() as u32,
        x: ((cw - w as f64) / 2.).round() as u32,
        y: ((ch - footer - h as f64) / 2.).round() as u32,
        photo_w: w,
        photo_h: h,
        footer: footer.round() as u32,
    }
}

/// Choose integer photo dimensions whose rounded margins fit the actual viewport.
/// Doing this before rasterization avoids resizing a finished canvas for a 1 px excess.
pub(super) fn preview_layout(w: u32, h: u32, s: &Settings, viewport: (u32, u32)) -> Layout {
    let fits = |layout: &Layout| layout.width <= viewport.0 && layout.height <= viewport.1;
    let full = layout(w, h, s);
    if fits(&full) {
        return full;
    }
    let source_edge = w.max(h).max(1);
    let at_edge = |edge: u32| {
        let scale = edge as f64 / source_edge as f64;
        layout(
            (w as f64 * scale).round().max(1.) as u32,
            (h as f64 * scale).round().max(1.) as u32,
            s,
        )
    };
    // Monotonic integer dimensions let us find the largest fit without repeated sampling.
    let (mut low, mut high) = (1, source_edge);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if fits(&at_edge(middle)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    // A viewport smaller than even a 1 px photo plus its frame uses the tiny-image fallback.
    at_edge(low)
}
