use super::{Layout, Photo, load_preview};
use crate::settings::{Background, Settings};
use crate::tr;
use anyhow::Result;
use image::RgbaImage;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

// One caption-free canvas, bounded independently of the decoded-photo LRU.
pub(super) const BASE_BUDGET: usize = 32 * 1024 * 1024;

#[derive(PartialEq)]
pub(super) struct BaseKey {
    layout: Layout,
    background: Background,
    color: [u8; 3],
    blur: u32,
    dim: u32,
    radius: u32,
    shadow: bool,
}

impl BaseKey {
    pub fn new(layout: &Layout, settings: &Settings) -> Self {
        Self {
            layout: layout.clone(),
            background: settings.background,
            color: settings.color,
            blur: settings.blur,
            dim: settings.background_dim,
            radius: settings.radius,
            shadow: settings.shadow,
        }
    }
}

pub(super) struct CachedBase {
    // A strong reference prevents both address reuse and mutation via Arc::get_mut.
    pub photo: Arc<Photo>,
    pub key: BaseKey,
    pub pixels: RgbaImage,
}

#[derive(PartialEq)]
struct Stamp {
    length: u64,
    modified: Option<SystemTime>,
}
impl Stamp {
    fn read(path: &Path) -> Result<Self> {
        let metadata = path.metadata()?;
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

struct Entry {
    path: PathBuf,
    stamp: Stamp,
    edge: u32,
    photo: Arc<Photo>,
}

/// LRU of reduced photos. The budget covers retained pixel buffers, not in-flight work.
pub struct PreviewCache {
    entries: VecDeque<Entry>,
    bytes: usize,
    budget: usize,
}
impl Default for PreviewCache {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024)
    }
}
impl PreviewCache {
    pub fn new(pixel_budget: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            budget: pixel_budget,
        }
    }

    pub fn load(&mut self, path: &Path, edge: u32) -> Result<Arc<Photo>> {
        anyhow::ensure!(edge > 0, tr!("errors.preview_size"));
        let stamp = Stamp::read(path)?;
        // Buckets prevent repeated decoding while the window is resized a few pixels.
        let edge = edge.div_ceil(512).saturating_mul(512);
        if let Some(index) = self.entries.iter().position(|entry| entry.path == path) {
            let entry = self.entries.remove(index).unwrap();
            let full_size = entry.photo.image.width().max(entry.photo.image.height()) < entry.edge;
            if entry.stamp == stamp && (entry.edge >= edge || full_size) {
                let photo = entry.photo.clone();
                self.entries.push_back(entry);
                return Ok(photo);
            }
            self.bytes -= entry.photo.image.as_bytes().len();
        }
        let photo = Arc::new(load_preview(path, edge)?);
        let bytes = photo.image.as_bytes().len();
        // Never retain an oversized image, or a snapshot modified during decoding.
        if bytes <= self.budget && Stamp::read(path)? == stamp {
            while self.bytes + bytes > self.budget || self.entries.len() >= 16 {
                let entry = self.entries.pop_front().unwrap();
                self.bytes -= entry.photo.image.as_bytes().len();
            }
            self.bytes += bytes;
            self.entries.push_back(Entry {
                path: path.to_owned(),
                stamp,
                edge,
                photo: photo.clone(),
            });
        }
        Ok(photo)
    }

    pub fn retain(&mut self, paths: &[PathBuf]) {
        self.entries.retain(|entry| paths.contains(&entry.path));
        self.bytes = self
            .entries
            .iter()
            .map(|entry| entry.photo.image.as_bytes().len())
            .sum();
    }
}
