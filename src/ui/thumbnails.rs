use gpui::RenderImage;
use std::{
    collections::{BTreeSet, VecDeque},
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) const ITEM_WIDTH: f32 = 126.;
pub(super) const ITEM_STRIDE: f32 = ITEM_WIDTH + 12.;
const OVERSCAN: usize = 2;
const CACHE_BUDGET: usize = 16 * 1024 * 1024;
const CACHE_ENTRIES: usize = 256;

pub(super) struct FilmstripWindow {
    pub visible: Range<usize>,
    pub rendered: Range<usize>,
    pub offset: f32,
}

impl FilmstripWindow {
    /// Slots include the trailing import button. Clamp offsets after removals/resizes.
    pub fn new(slots: usize, offset: f32, width: f32) -> Self {
        let width = width.max(0.);
        let offset = offset
            .max(0.)
            .min((slots as f32 * ITEM_STRIDE - width).max(0.));
        let start = ((offset / ITEM_STRIDE).floor() as usize).min(slots);
        let end = (((offset + width) / ITEM_STRIDE).ceil() as usize).min(slots);
        Self {
            visible: start..end,
            rendered: start.saturating_sub(OVERSCAN)..end.saturating_add(OVERSCAN).min(slots),
            offset,
        }
    }

    pub fn wanted(&self, photos: &[PathBuf]) -> Vec<PathBuf> {
        // Visible photos first, then the immediate neighbors (never the import slot).
        self.visible
            .clone()
            .chain((self.rendered.start..self.visible.start).rev())
            .chain(self.visible.end..self.rendered.end)
            .filter_map(|index| photos.get(index).cloned())
            .collect()
    }
}

struct Entry {
    path: PathBuf,
    image: Arc<RenderImage>,
    bytes: usize,
}

#[derive(Clone)]
pub(super) struct ThumbnailJob {
    pub path: PathBuf,
    cancelled: Arc<AtomicBool>,
}

impl ThumbnailJob {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// One worker, a small priority window, and an LRU bounded by retained pixel bytes.
pub(super) struct Thumbnails {
    entries: VecDeque<Entry>,
    bytes: usize,
    budget: usize,
    wanted: Vec<PathBuf>,
    attempted: BTreeSet<PathBuf>,
    active: Option<ThumbnailJob>,
    retired: Vec<Arc<RenderImage>>,
}

impl Default for Thumbnails {
    fn default() -> Self {
        Self::new(CACHE_BUDGET)
    }
}

impl Thumbnails {
    fn new(budget: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            budget,
            wanted: Vec::new(),
            attempted: BTreeSet::new(),
            active: None,
            retired: Vec::new(),
        }
    }

    pub fn image(&self, path: &Path) -> Option<&Arc<RenderImage>> {
        self.entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| &entry.image)
    }

    pub fn set_wanted(&mut self, wanted: Vec<PathBuf>) {
        if self.wanted == wanted {
            return;
        }
        if let Some(job) = &self.active
            && !wanted.contains(&job.path)
        {
            job.cancelled.store(true, Ordering::Relaxed);
        }
        self.attempted.retain(|path| wanted.contains(path));
        // Touch low-priority neighbors first so visible photos are most recent.
        for path in wanted.iter().rev() {
            if let Some(index) = self.entries.iter().position(|entry| &entry.path == path) {
                let entry = self.entries.remove(index).unwrap();
                self.entries.push_back(entry);
                self.attempted.insert(path.clone());
            }
        }
        self.wanted = wanted;
    }

    pub fn has_pending(&self) -> bool {
        self.wanted
            .iter()
            .any(|path| !self.attempted.contains(path))
    }

    pub fn next_job(&mut self) -> Option<ThumbnailJob> {
        if self.active.is_some() {
            return None;
        }
        let path = self
            .wanted
            .iter()
            .find(|path| !self.attempted.contains(*path))?
            .clone();
        self.attempted.insert(path.clone());
        let job = ThumbnailJob {
            path,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        self.active = Some(job.clone());
        Some(job)
    }

    pub fn finish(&mut self, job: ThumbnailJob, image: Option<Arc<RenderImage>>) {
        self.active = None;
        if job.is_cancelled() || !self.wanted.contains(&job.path) {
            self.attempted.remove(&job.path);
            return;
        }
        // Failed/oversized images are attempted once while they remain in the window.
        let Some(image) = image else {
            return;
        };
        let bytes = image.as_bytes(0).map_or(0, <[u8]>::len);
        if bytes > self.budget {
            return;
        }
        while self.bytes + bytes > self.budget || self.entries.len() >= CACHE_ENTRIES {
            let entry = self.entries.pop_front().unwrap();
            self.bytes -= entry.bytes;
            self.retired.push(entry.image);
        }
        self.bytes += bytes;
        self.entries.push_back(Entry {
            path: job.path,
            image,
            bytes,
        });
    }

    pub fn remove(&mut self, path: &Path) {
        if let Some(job) = &self.active
            && job.path == path
        {
            job.cancelled.store(true, Ordering::Relaxed);
        }
        self.wanted.retain(|wanted| wanted != path);
        self.attempted.remove(path);
        if let Some(index) = self.entries.iter().position(|entry| entry.path == path) {
            let entry = self.entries.remove(index).unwrap();
            self.bytes -= entry.bytes;
            self.retired.push(entry.image);
        }
    }

    pub fn clear(&mut self) {
        self.cancel_active();
        self.wanted.clear();
        self.attempted.clear();
        self.retired
            .extend(self.entries.drain(..).map(|entry| entry.image));
        self.bytes = 0;
    }

    pub fn cancel_active(&self) {
        if let Some(job) = &self.active {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub fn take_retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }
}

pub(super) fn decode(job: &ThumbnailJob) -> anyhow::Result<Option<Arc<RenderImage>>> {
    if job.is_cancelled() {
        return Ok(None);
    }
    let photo = photo_frame::render::load_preview(&job.path, 240)?;
    if job.is_cancelled() {
        return Ok(None);
    }
    let mut pixels = photo.image.thumbnail(240, 120).to_rgba8();
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    if job.is_cancelled() {
        return Ok(None);
    }
    Ok(Some(Arc::new(RenderImage::new(vec![image::Frame::new(
        pixels,
    )]))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(count: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|index| PathBuf::from(format!("photo-{index}.png")))
            .collect()
    }

    fn image() -> Arc<RenderImage> {
        Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255])),
        )]))
    }

    #[test]
    fn large_filmstrips_render_only_the_viewport_and_two_neighbors_on_each_side() {
        let photos = paths(100_000);
        let view = FilmstripWindow::new(photos.len() + 1, 50_000. * ITEM_STRIDE, 3. * ITEM_STRIDE);
        assert_eq!(view.visible, 50_000..50_003);
        assert_eq!(view.rendered, 49_998..50_005);
        assert_eq!(
            view.wanted(&photos),
            [50_000, 50_001, 50_002, 49_999, 49_998, 50_003, 50_004]
                .map(|index| photos[index].clone())
        );
        // Leading and trailing spacers preserve the full strip width.
        assert_eq!(
            view.rendered.start + view.rendered.len() + photos.len() + 1 - view.rendered.end,
            photos.len() + 1
        );
    }

    #[test]
    fn filmstrip_clamps_scroll_after_removing_photos_and_expanding_the_window() {
        let photos = paths(5);
        let end = FilmstripWindow::new(6, 100_000., ITEM_STRIDE * 2.);
        assert_eq!(end.offset, ITEM_STRIDE * 4.);
        assert_eq!(end.visible, 4..6);
        assert_eq!(
            end.wanted(&photos),
            [4, 3, 2].map(|index| photos[index].clone())
        );
        let wide = FilmstripWindow::new(6, end.offset, 10. * ITEM_STRIDE);
        assert_eq!(wide.offset, 0.);
        assert_eq!(wide.rendered, 0..6);
        let empty = FilmstripWindow::new(1, end.offset, 600.);
        assert_eq!(empty.rendered, 0..1);
        assert!(empty.wanted(&[]).is_empty());
        let partial = FilmstripWindow::new(20, ITEM_STRIDE * 4.5, ITEM_STRIDE);
        assert_eq!(partial.visible, 4..6);
    }

    #[test]
    fn scrolling_cancels_offscreen_work_and_waits_for_the_one_active_decoder() {
        let photos = paths(10);
        let mut state = Thumbnails::default();
        state.set_wanted(photos[..3].to_vec());
        let first = state.next_job().unwrap();
        assert!(state.next_job().is_none());
        // Moving the viewport keeps an active job if that photo is still needed.
        state.set_wanted(vec![photos[1].clone(), photos[0].clone()]);
        assert!(!first.is_cancelled());
        state.set_wanted(photos[7..9].to_vec());
        assert!(first.is_cancelled());
        assert!(state.next_job().is_none());
        state.finish(first, Some(image()));
        assert!(state.image(&photos[0]).is_none());
        assert_eq!(state.next_job().unwrap().path, photos[7]);
    }

    #[test]
    fn clearing_and_reimporting_cannot_restore_old_pixels_or_overlap_workers() {
        let photos = paths(1);
        let mut state = Thumbnails::default();
        state.set_wanted(photos.clone());
        let old = state.next_job().unwrap();
        state.clear();
        state.set_wanted(photos.clone());
        assert!(old.is_cancelled());
        assert!(state.next_job().is_none());
        state.finish(old, Some(image()));
        assert!(state.image(&photos[0]).is_none());
        let replacement = state.next_job().unwrap();
        assert!(!replacement.is_cancelled());
        let pixels = image();
        state.finish(replacement, Some(pixels.clone()));
        assert!(Arc::ptr_eq(state.image(&photos[0]).unwrap(), &pixels));
        state.remove(&photos[0]);
        assert!(state.image(&photos[0]).is_none());
        assert!(!state.has_pending());
    }

    #[test]
    fn lru_eviction_obeys_pixel_budget_and_releases_images_for_gpu_cleanup() {
        let photos = paths(3);
        let mut state = Thumbnails::new(4 * 3 * 4 * 2);
        state.set_wanted(photos[..2].to_vec());
        let first = state.next_job().unwrap();
        let a = image();
        let weak = Arc::downgrade(&a);
        state.finish(first, Some(a));
        let second = state.next_job().unwrap();
        state.finish(second, Some(image()));
        state.set_wanted(vec![photos[1].clone(), photos[2].clone()]);
        let third = state.next_job().unwrap();
        state.finish(third, Some(image()));
        assert!(state.image(&photos[0]).is_none());
        assert!(state.image(&photos[1]).is_some());
        assert!(state.image(&photos[2]).is_some());
        assert_eq!(state.bytes, state.budget);
        assert!(weak.upgrade().is_some());
        // The caller drops uploaded GPU tiles before releasing retired CPU buffers.
        drop(state.take_retired());
        assert!(weak.upgrade().is_none());
        state.clear();
        assert_eq!(state.bytes, 0);
        assert_eq!(state.take_retired().len(), 2);
    }

    #[test]
    fn failed_or_oversized_thumbnails_do_not_trigger_an_endless_decode_loop() {
        let photos = paths(2);
        let mut state = Thumbnails::new(1);
        state.set_wanted(photos.clone());
        let failed = state.next_job().unwrap();
        state.finish(failed, None);
        let oversized = state.next_job().unwrap();
        state.finish(oversized, Some(image()));
        assert_eq!(state.bytes, 0);
        assert!(!state.has_pending());
        state.set_wanted(photos.clone());
        assert!(state.next_job().is_none());
        // Leaving and revisiting the viewport allows a retry (e.g. after repairing a file).
        state.set_wanted(Vec::new());
        state.set_wanted(photos);
        assert!(state.next_job().is_some());
    }

    #[test]
    fn removed_job_is_cancelled_before_opening_a_missing_file() {
        let photos = paths(1);
        let mut state = Thumbnails::default();
        state.set_wanted(photos.clone());
        let job = state.next_job().unwrap();
        state.remove(&photos[0]);
        assert!(decode(&job).unwrap().is_none());
        state.finish(job, None);
        assert!(!state.has_pending());
    }

    #[test]
    fn decoded_thumbnails_fit_the_strip_and_use_gpui_bgra_pixels() {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-output");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "thumbnail-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        image::RgbImage::from_pixel(400, 200, image::Rgb([10, 20, 30]))
            .save(&path)
            .unwrap();
        let job = ThumbnailJob {
            path: path.clone(),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let pixels = decode(&job).unwrap().unwrap();
        std::fs::remove_file(path).unwrap();
        let dimensions = pixels.size(0);
        assert_eq!((dimensions.width.0, dimensions.height.0), (240, 120));
        assert_eq!(&pixels.as_bytes(0).unwrap()[..4], &[30, 20, 10, 255]);
    }
}
