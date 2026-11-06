mod display;
mod view;
mod viewport;

use crate::ui::Studio;
use anyhow::Result;
use display::PreviewDisplay;
use gpui::{Context, RenderImage, Task, Timer};
use photo_frame::{
    render::{Engine, PreviewCache},
    settings::RenderSettings,
    tr,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
pub(super) struct PreviewCoordinator {
    pub display: PreviewDisplay,
    pub viewport: viewport::PreviewViewport,
    target: (u32, u32),
    rendering: bool,
    state: Arc<Mutex<Option<PreviewState>>>,
    generation: Arc<AtomicU64>,
    task: Option<Task<()>>,
}

#[derive(Default)]
struct PreviewState {
    engine: Engine,
    photos: PreviewCache,
    sources: Arc<Vec<PathBuf>>,
}

impl PreviewCoordinator {
    pub fn target(&self) -> (u32, u32) {
        self.target
    }
    pub fn set_target(&mut self, target: (u32, u32)) {
        self.target = target;
    }
    pub fn is_rendering(&self) -> bool {
        self.rendering
    }

    pub fn refresh(
        &mut self,
        source: Option<PathBuf>,
        paths: Arc<Vec<PathBuf>>,
        settings: RenderSettings,
        trigger: PreviewTrigger,
        cx: &mut Context<Studio>,
    ) {
        let request = PreviewRequest::new(self.generation.clone());
        // Dropping the task cancels pending timers instead of retaining one per keystroke.
        self.task = None;
        let Some(path) = source else {
            // Drop retained pixels without waiting on a background render's mutex.
            self.state = Arc::new(Mutex::new(None));
            self.display.clear();
            self.rendering = false;
            cx.notify();
            return;
        };
        self.display.select(&path);
        self.rendering = true;
        let target = self.target;
        if target.0 == 0 || target.1 == 0 {
            self.rendering = false;
            cx.notify();
            return;
        }
        let preview_state = self.state.clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            let delay = trigger.delay();
            if !delay.is_zero() {
                Timer::after(delay).await;
            }
            if !request.is_current() {
                return;
            }
            let worker_request = request.clone();
            let preview_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut preview_state = preview_state
                        .lock()
                        .map_err(|_| anyhow::anyhow!(tr!("errors.renderer")))?;
                    if !worker_request.is_current() {
                        return Ok(None);
                    }
                    let state = preview_state.get_or_insert_with(PreviewState::default);
                    if !Arc::ptr_eq(&state.sources, &paths) {
                        state.photos.retain(&paths);
                        state.sources = paths;
                    }
                    let Some(mut pixels) = worker_request.run(
                        || state.photos.load(&path, target.0.max(target.1)),
                        |photo| {
                            state
                                .engine
                                .compose_cached_preview(&photo, &settings, target)
                        },
                    )?
                    else {
                        return Ok(None);
                    };
                    for pixel in pixels.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
                    Ok::<_, anyhow::Error>(Some(image))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !request.is_current() {
                    return;
                }
                this.preview.rendering = false;
                match result {
                    Ok(Some(image)) => {
                        this.preview.display.publish(
                            preview_path,
                            image,
                            std::time::Instant::now(),
                        );
                    }
                    Err(e) => {
                        this.preview.display.clear();
                        this.status = tr!("errors.preview", error = format!("{e:#}"));
                    }
                    Ok(None) => {}
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Drop for PreviewCoordinator {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy)]
pub(super) enum PreviewTrigger {
    Immediate,
    Input,
    Resize,
}

impl PreviewTrigger {
    pub fn delay(self) -> Duration {
        match self {
            Self::Immediate => Duration::ZERO,
            Self::Input => Duration::from_millis(40),
            Self::Resize => Duration::from_millis(80),
        }
    }
}

#[derive(Clone)]
struct PreviewRequest {
    latest: Arc<AtomicU64>,
    generation: u64,
}

impl PreviewRequest {
    pub fn new(latest: Arc<AtomicU64>) -> Self {
        let generation = latest.fetch_add(1, Ordering::Relaxed) + 1;
        Self { latest, generation }
    }

    pub fn is_current(&self) -> bool {
        self.latest.load(Ordering::Relaxed) == self.generation
    }

    /// Stop at stage boundaries, including when an obsolete decode/render failed.
    /// The decoders and compositor themselves are synchronous and run to completion.
    pub fn run<T, U>(
        &self,
        load: impl FnOnce() -> Result<T>,
        compose: impl FnOnce(T) -> Result<U>,
    ) -> Result<Option<U>> {
        if !self.is_current() {
            return Ok(None);
        }
        let photo = load();
        if !self.is_current() {
            return Ok(None);
        }
        let pixels = compose(photo?);
        if !self.is_current() {
            return Ok(None);
        }
        pixels.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_request_runs_and_publishes() {
        let latest = Arc::new(AtomicU64::new(0));
        let requests: Vec<_> = (0..20)
            .map(|_| PreviewRequest::new(latest.clone()))
            .collect();
        for request in &requests[..19] {
            assert_eq!(
                request
                    .run(|| panic!("obsolete decode"), |_: ()| Ok(1))
                    .unwrap(),
                None
            );
        }
        assert_eq!(
            requests[19].run(|| Ok(2), |photo| Ok(photo * 3)).unwrap(),
            Some(6)
        );
    }

    #[test]
    fn request_replaced_during_decode_skips_composition_and_obsolete_errors() {
        let latest = Arc::new(AtomicU64::new(0));
        let request = PreviewRequest::new(latest.clone());
        let result = request
            .run(
                || -> Result<()> {
                    let _newer = PreviewRequest::new(latest);
                    anyhow::bail!("old decode failed");
                },
                |_| -> Result<()> { panic!("obsolete composition") },
            )
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn request_replaced_during_composition_skips_output_but_current_errors_propagate() {
        let latest = Arc::new(AtomicU64::new(0));
        let request = PreviewRequest::new(latest.clone());
        assert_eq!(
            request
                .run(
                    || Ok(()),
                    |_| {
                        let _newer = PreviewRequest::new(latest.clone());
                        Ok(7)
                    }
                )
                .unwrap(),
            None
        );
        let current = PreviewRequest::new(latest);
        assert!(
            current
                .run(
                    || -> Result<()> { anyhow::bail!("decode failed") },
                    |_| Ok(())
                )
                .is_err()
        );
        assert!(
            current
                .run(
                    || Ok(()),
                    |_| -> Result<()> { anyhow::bail!("compose failed") }
                )
                .is_err()
        );
    }
}
