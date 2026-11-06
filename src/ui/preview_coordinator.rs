use super::{
    Studio,
    preview::{PreviewRequest, PreviewTrigger},
    preview_display::PreviewDisplay,
};
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
};

#[derive(Default)]
pub(super) struct PreviewCoordinator {
    pub display: PreviewDisplay,
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
