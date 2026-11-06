use super::FileStamp;
use crate::tr;
use anyhow::{Context, Result};
use image::RgbaImage;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

const BUDGET: usize = 4 * 1024 * 1024;

struct Entry {
    path: PathBuf,
    stamp: FileStamp,
    bounds: (u32, u32),
    pixels: Arc<RgbaImage>,
}

/// Retain only one resized logo; source-sized decoded buffers are released.
#[derive(Default)]
pub(super) struct LogoCache(Option<Entry>);

impl LogoCache {
    pub fn clear(&mut self) {
        self.0 = None;
    }

    pub fn image(&mut self, path: &Path, bounds: (u32, u32)) -> Result<Arc<RgbaImage>> {
        let stamp = match FileStamp::read(path).context(tr!("errors.logo")) {
            Ok(stamp) => stamp,
            Err(error) => {
                self.clear();
                return Err(error);
            }
        };
        if let Some(entry) = self.0.as_ref().filter(|entry| {
            stamp.is_reliable()
                && entry.path == path
                && entry.stamp == stamp
                && entry.bounds == bounds
        }) {
            return Ok(entry.pixels.clone());
        }
        self.clear();
        let pixels = Arc::new(
            image::open(path)
                .context(tr!("errors.logo"))?
                .thumbnail(bounds.0, bounds.1)
                .to_rgba8(),
        );
        if pixels.as_raw().len() <= BUDGET
            && stamp.is_reliable()
            && FileStamp::read(path).is_ok_and(|current| current == stamp)
        {
            self.0 = Some(Entry {
                path: path.to_owned(),
                stamp,
                bounds,
                pixels: pixels.clone(),
            });
        }
        Ok(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_logos_reuse_pixels_and_resize_or_missing_files_invalidate_them() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/baseline-444.jpg");
        let mut cache = LogoCache::default();
        let first = cache.image(&path, (40, 40)).unwrap();
        assert!(Arc::ptr_eq(&first, &cache.image(&path, (40, 40)).unwrap()));
        let smaller = cache.image(&path, (20, 20)).unwrap();
        assert!(!Arc::ptr_eq(&first, &smaller));
        assert!(smaller.width() <= 20 && smaller.height() <= 20);
        assert!(
            cache
                .image(&path.with_extension("missing"), (20, 20))
                .is_err()
        );
        assert!(cache.0.is_none());
        assert_eq!(*cache.image(&path, (40, 40)).unwrap(), *first);
        cache.clear();
        assert!(cache.0.is_none());
    }
}
