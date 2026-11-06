mod dialog;

pub(super) use dialog::ExportDialog;

use crate::ui::components::{ACCENT, LINE, col, label, row, section};
use crate::ui::{Command, Studio};
use gpui::{AnyElement, Context, FontWeight, Task, div, prelude::*, px, relative, rgb};
use photo_frame::{
    batch::{self, BatchEvent, BatchOptions},
    settings::RenderSettings,
    tr,
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[derive(Default)]
pub(super) struct ExportSession {
    pub pending_scope: Option<bool>,
    running: bool,
    progress: (usize, usize),
    failures: Vec<String>,
    success: usize,
    active: BTreeSet<usize>,
    peak: usize,
    started: Option<Instant>,
    cancel: Arc<AtomicBool>,
    task: Option<Task<()>>,
}

impl ExportSession {
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn progress(&self) -> (usize, usize) {
        self.progress
    }
    pub fn failures(&self) -> &[String] {
        &self.failures
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn begin(&mut self, total: usize) {
        self.cancel();
        self.task = None;
        self.cancel = Arc::new(AtomicBool::new(false));
        self.pending_scope = None;
        self.running = true;
        self.progress = (0, total);
        self.failures.clear();
        self.success = 0;
        self.active.clear();
        self.peak = 0;
        self.started = Some(Instant::now());
    }

    /// Return whether a successful export should trigger preference persistence.
    fn record(&mut self, event: BatchEvent) -> bool {
        match event {
            BatchEvent::Configured { .. } => {}
            BatchEvent::Started { index, .. } => {
                self.active.insert(index);
                self.peak = self.peak.max(self.active.len());
            }
            BatchEvent::Finished {
                index,
                source,
                result,
            } => {
                self.active.remove(&index);
                self.progress.0 += 1;
                match result {
                    Ok(_) => {
                        self.success += 1;
                        return true;
                    }
                    Err(error) => self.failures.push(format!(
                        "{}: {error:#}",
                        source.file_name().unwrap_or_default().to_string_lossy()
                    )),
                }
            }
        }
        false
    }

    fn seconds(&self) -> String {
        format!(
            "{:.1}",
            self.started
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.)
        )
    }

    fn progress_status(&self) -> String {
        tr!(
            "status.export_progress",
            processed = self.progress.0,
            total = self.progress.1,
            success = self.success,
            active = self.active.len(),
            seconds = self.seconds()
        )
    }

    fn finish(&mut self) -> String {
        self.running = false;
        let cancelled = self.cancel.load(Ordering::Relaxed);
        if self.progress.0 < self.progress.1 && !cancelled {
            self.failures.push(tr!(
                "errors.incomplete",
                count = self.progress.1 - self.progress.0
            ));
        }
        self.active.clear();
        tr!(
            "status.export_done",
            result = if cancelled {
                tr!("status.cancelled")
            } else {
                tr!("status.completed")
            },
            success = self.success,
            failed = self.failures.len(),
            peak = self.peak,
            seconds = self.seconds()
        )
    }

    pub fn start(
        &mut self,
        paths: Vec<PathBuf>,
        output: PathBuf,
        settings: RenderSettings,
        cx: &mut Context<Studio>,
    ) {
        self.begin(paths.len());
        let cancel = self.cancel.clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            let worker_cancel = cancel.clone();
            let receiver = cx
                .background_executor()
                .spawn(async move {
                    batch::start(
                        paths,
                        output,
                        settings,
                        worker_cancel,
                        BatchOptions::default(),
                    )
                })
                .await;
            let mut receiver = match receiver {
                Ok(receiver) => receiver,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.exports.running = false;
                        this.status = tr!("errors.export_start", error = error);
                        cx.notify();
                    });
                    return;
                }
            };
            loop {
                let (next, event) = cx
                    .background_executor()
                    .spawn(async move {
                        let event = receiver.recv();
                        (receiver, event)
                    })
                    .await;
                receiver = next;
                let Ok(event) = event else {
                    break;
                };
                if this
                    .update(cx, |this, cx| {
                        if this.exports.record(event) {
                            let _ = this.flush_preferences();
                        }
                        this.status = this.exports.progress_status();
                        cx.notify();
                    })
                    .is_err()
                {
                    cancel.store(true, Ordering::Relaxed);
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.status = this.exports.finish();
                cx.notify();
            });
        }));
    }
}

impl Drop for ExportSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Studio {
    pub(super) fn export_panel(&self, cx: &Context<Self>) -> AnyElement {
        col()
            .gap_2()
            .when(!self.exports.is_running(), |d| {
                d.child(
                    div()
                        .id("open-export")
                        .h(px(46.))
                        .w_full()
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x192a1c))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0xd7e5cd)))
                        .on_click(cx.listener(|this, _, window, cx| this.open_export(window, cx)))
                        .child(tr!("export.open")),
                )
            })
            .when(self.exports.is_running(), |d| {
                d.child(
                    row()
                        .justify_between()
                        .child(section(tr!("export.running")))
                        .child(label(format!(
                            "{} / {}",
                            self.exports.progress().0,
                            self.exports.progress().1
                        ))),
                )
                .child(
                    div().h(px(3.)).w_full().rounded_full().bg(rgb(LINE)).child(
                        div()
                            .h_full()
                            .w(relative(
                                self.exports.progress().0 as f32
                                    / self.exports.progress().1.max(1) as f32,
                            ))
                            .rounded_full()
                            .bg(rgb(ACCENT)),
                    ),
                )
                .child(self.button(
                    "cancel",
                    tr!("export.stop"),
                    false,
                    Command::Cancel,
                    cx,
                ))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_frame::render::{ExportReport, ExportTimings};

    #[test]
    fn export_events_track_success_failures_progress_and_peak_until_completion() {
        let mut session = ExportSession::default();
        session.begin(2);
        for index in [0, 1] {
            session.record(BatchEvent::Started {
                index,
                estimated_bytes: 1,
            });
        }
        assert!(session.record(BatchEvent::Finished {
            index: 1,
            source: "good.jpg".into(),
            result: Ok(ExportReport {
                path: "good_frame.jpg".into(),
                timings: ExportTimings::default(),
            }),
        }));
        assert!(!session.record(BatchEvent::Finished {
            index: 0,
            source: "bad.jpg".into(),
            result: Err(anyhow::anyhow!("decode failed")),
        }));
        session.finish();
        assert_eq!(session.progress(), (2, 2));
        assert_eq!((session.success, session.peak), (1, 2));
        assert_eq!(session.failures(), &["bad.jpg: decode failed"]);
        assert!(session.active.is_empty() && !session.is_running());
    }

    #[test]
    fn cancellation_restart_and_drop_keep_each_batch_token_independent() {
        let mut session = ExportSession::default();
        session.begin(3);
        let first = session.cancel.clone();
        session.cancel();
        session.finish();
        assert!(session.failures().is_empty());
        session.begin(2);
        let second = session.cancel.clone();
        assert!(first.load(Ordering::Relaxed));
        assert!(!second.load(Ordering::Relaxed));
        session.finish();
        assert_eq!(
            session.failures().len(),
            1,
            "Unexpected disconnect reports unfinished work"
        );
        drop(session);
        assert!(second.load(Ordering::Relaxed));
    }
}
