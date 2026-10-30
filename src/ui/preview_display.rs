use gpui::RenderImage;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

const FADE_DURATION: Duration = Duration::from_millis(160);

struct Frame {
    source: PathBuf,
    image: Arc<RenderImage>,
}

/// Retain the displayed photo during loading. An interrupted fade queues only its latest result.
#[derive(Default)]
pub(super) struct PreviewDisplay {
    requested: Option<PathBuf>,
    current: Option<Frame>,
    previous: Option<Frame>,
    pending: Option<Frame>,
    started: Option<Instant>,
    retired: Vec<Arc<RenderImage>>,
}

impl PreviewDisplay {
    pub fn select(&mut self, source: &Path) {
        self.requested = Some(source.to_owned());
        if self
            .pending
            .as_ref()
            .is_some_and(|frame| frame.source != source)
        {
            let frame = self.pending.take().unwrap();
            self.retired.push(frame.image);
        }
    }

    pub fn publish(&mut self, source: PathBuf, image: Arc<RenderImage>, now: Instant) {
        if self.requested.as_deref() != Some(source.as_path()) {
            self.retired.push(image);
            return;
        }
        self.advance(now);
        let frame = Frame { source, image };
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.source == frame.source)
        {
            // Caption edits and resizes update in place, preserving any fade already in progress.
            self.retired
                .push(self.current.replace(frame).unwrap().image);
        } else if self.started.is_some() {
            // Finish the visible blend instead of abruptly making a half-visible image opaque.
            if let Some(old) = self.pending.replace(frame) {
                self.retired.push(old.image);
            }
        } else {
            self.start(frame, now);
        }
    }

    fn start(&mut self, frame: Frame, now: Instant) {
        self.previous = self.current.replace(frame);
        self.started = Some(now);
    }

    pub fn advance(&mut self, now: Instant) {
        if self
            .started
            .is_some_and(|start| now.saturating_duration_since(start) >= FADE_DURATION)
        {
            self.started = None;
            if let Some(old) = self.previous.take() {
                self.retired.push(old.image);
            }
            if let Some(next) = self.pending.take() {
                self.start(next, now);
            }
        }
    }

    pub fn image(&self) -> Option<&Arc<RenderImage>> {
        self.current.as_ref().map(|frame| &frame.image)
    }

    pub fn previous(&self) -> Option<&Arc<RenderImage>> {
        self.previous.as_ref().map(|frame| &frame.image)
    }

    pub fn is_animating(&self) -> bool {
        self.started.is_some()
    }

    pub fn opacity(&self, now: Instant) -> f32 {
        let Some(start) = self.started else {
            return 1.;
        };
        let t = (now.saturating_duration_since(start).as_secs_f32() / FADE_DURATION.as_secs_f32())
            .min(1.);
        t * t * (3. - 2. * t)
    }

    pub fn clear(&mut self) {
        for frame in [
            self.current.take(),
            self.previous.take(),
            self.pending.take(),
        ]
        .into_iter()
        .flatten()
        {
            self.retired.push(frame.image);
        }
        self.requested = None;
        self.started = None;
    }

    pub fn take_retired(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.retired)
    }
}

#[cfg(test)]
mod tests {
    use super::{FADE_DURATION, PreviewDisplay};
    use gpui::RenderImage;
    use std::{path::Path, sync::Arc, time::Instant};

    fn image() -> Arc<RenderImage> {
        Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(1, 1),
        )]))
    }

    fn ready(display: &mut PreviewDisplay, source: &str, now: Instant) -> Arc<RenderImage> {
        let image = image();
        display.select(Path::new(source));
        display.publish(source.into(), image.clone(), now);
        image
    }

    #[test]
    fn loading_preserves_the_old_photo_then_fades_and_releases_it() {
        let mut display = PreviewDisplay::default();
        let start = Instant::now();
        let a = ready(&mut display, "a.jpg", start);
        display.advance(start + FADE_DURATION);
        display.select(Path::new("b.jpg"));
        assert_eq!(display.image().unwrap().id, a.id);
        assert!(!display.is_animating());
        let now = start + FADE_DURATION;
        let b = ready(&mut display, "b.jpg", now);
        assert_eq!(display.previous().unwrap().id, a.id);
        assert_eq!(display.image().unwrap().id, b.id);
        assert_eq!(display.opacity(now), 0.);
        assert!((display.opacity(now + FADE_DURATION / 2) - 0.5).abs() < 0.001);
        display.advance(now + FADE_DURATION);
        assert!(!display.is_animating());
        assert!(display.previous().is_none());
        assert_eq!(display.take_retired()[0].id, a.id);
    }

    #[test]
    fn rapid_switches_queue_only_the_latest_result_without_interrupting_the_blend() {
        let mut display = PreviewDisplay::default();
        let start = Instant::now();
        let a = ready(&mut display, "a.jpg", start);
        display.advance(start + FADE_DURATION);
        let now = start + FADE_DURATION;
        let b = ready(&mut display, "b.jpg", now);
        let c = ready(&mut display, "c.jpg", now + FADE_DURATION / 4);
        let d = ready(&mut display, "d.jpg", now + FADE_DURATION / 2);
        display.publish("c.jpg".into(), image(), now + FADE_DURATION / 2);
        assert_eq!(display.image().unwrap().id, b.id);
        assert_eq!(display.previous().unwrap().id, a.id);
        assert_eq!(display.pending.as_ref().unwrap().image.id, d.id);
        assert!(display.take_retired().iter().any(|image| image.id == c.id));
        display.advance(now + FADE_DURATION);
        assert_eq!(display.image().unwrap().id, d.id);
        assert_eq!(display.previous().unwrap().id, b.id);
        display.advance(now + FADE_DURATION * 2);
        assert!(!display.is_animating());
        assert!(display.pending.is_none() && display.previous().is_none());
    }

    #[test]
    fn edits_update_immediately_without_restarting_a_fade() {
        let mut display = PreviewDisplay::default();
        let start = Instant::now();
        ready(&mut display, "a.jpg", start);
        display.advance(start + FADE_DURATION);
        let edit = image();
        display.publish("a.jpg".into(), edit.clone(), start + FADE_DURATION);
        assert_eq!(display.image().unwrap().id, edit.id);
        assert!(!display.is_animating());
        let now = start + FADE_DURATION;
        ready(&mut display, "b.jpg", now);
        display.publish("b.jpg".into(), image(), now + FADE_DURATION / 2);
        assert!((display.opacity(now + FADE_DURATION / 2) - 0.5).abs() < 0.001);
    }

    #[test]
    fn clearing_during_a_transition_releases_all_layers_and_rejects_late_results() {
        let mut display = PreviewDisplay::default();
        let start = Instant::now();
        ready(&mut display, "a.jpg", start);
        display.advance(start + FADE_DURATION);
        ready(&mut display, "b.jpg", start + FADE_DURATION);
        ready(&mut display, "c.jpg", start + FADE_DURATION);
        display.clear();
        assert!(display.image().is_none() && display.previous().is_none());
        assert!(!display.is_animating());
        assert_eq!(display.take_retired().len(), 3);
        display.publish("c.jpg".into(), image(), start + FADE_DURATION * 2);
        assert!(display.image().is_none());
    }
}
