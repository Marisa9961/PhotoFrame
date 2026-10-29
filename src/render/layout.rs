use crate::settings::{Ratio, Settings};

#[derive(Debug)]
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
