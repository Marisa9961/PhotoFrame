use crate::tr;

use crate::{
    render::{self, Engine, ExportReport},
    settings::RenderSettings,
};
use anyhow::{Result, anyhow};
use image::ImageDecoder;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

const MIB: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct BatchOptions {
    /// Zero selects automatic CPU and queue sizing; positive values are for diagnostics.
    pub workers: usize,
    /// Zero uses all currently available memory, with no fixed system reserve.
    pub memory_budget: u64,
}
struct BatchPlan {
    workers: usize,
    memory_budget: u64,
    dynamic_memory: bool,
}
impl BatchOptions {
    fn plan(self, cpus: usize, available: Option<u64>, jobs: usize) -> BatchPlan {
        let memory_budget = if self.memory_budget > 0 {
            self.memory_budget
        } else {
            available.unwrap_or(256 * MIB).max(1)
        };
        let workers = if self.workers > 0 {
            self.workers
        } else if available.is_none() && self.memory_budget == 0 {
            1
        } else {
            // Engines are initialized after admission; idle threads do not reserve image buffers.
            cpus.saturating_sub(1).max(1)
        };
        BatchPlan {
            workers: workers.min(jobs),
            memory_budget,
            dynamic_memory: self.memory_budget == 0,
        }
    }
}
pub enum BatchEvent {
    Configured {
        workers: usize,
        /// Initial budget; automatic mode adjusts it while processing.
        memory_budget: u64,
    },
    Started {
        index: usize,
        estimated_bytes: u64,
    },
    Finished {
        index: usize,
        source: PathBuf,
        result: Result<ExportReport>,
    },
}

struct Usage {
    bytes: u64,
    active: usize,
}
struct MemoryGate {
    budget: u64,
    usage: Mutex<Usage>,
    changed: Condvar,
    dynamic_memory: bool,
    baseline_process_memory: Option<u64>,
}
struct Permit {
    gate: Arc<MemoryGate>,
    cost: u64,
}
impl MemoryGate {
    fn budget_for_available(
        &self,
        used: u64,
        available: Option<u64>,
        resident: Option<u64>,
    ) -> u64 {
        if !self.dynamic_memory {
            return self.budget;
        }
        let Some(free) = available else {
            return 0;
        };
        if let Some((baseline, current)) = self.baseline_process_memory.zip(resident) {
            // Credit only materialized allocations. Crediting all reservations before
            // workers allocate their buffers can over-admit jobs and exhaust memory.
            let materialized = current.saturating_sub(baseline).min(used);
            free.saturating_add(materialized)
        } else {
            // Without process measurements, retain a startup ceiling to bound pending reservations.
            free.saturating_add(used).min(self.budget)
        }
    }
    fn acquire(self: &Arc<Self>, cost: u64, cancel: &AtomicBool) -> Option<Permit> {
        let mut usage = self.usage.lock().unwrap();
        loop {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            // An image larger than the budget may run alone; it must never block the queue forever.
            let budget = self.budget_for_available(
                usage.bytes,
                if self.dynamic_memory {
                    available_memory()
                } else {
                    None
                },
                if self.dynamic_memory {
                    process_memory()
                } else {
                    None
                },
            );
            if usage.active == 0 || usage.bytes.saturating_add(cost) <= budget {
                usage.bytes = usage.bytes.saturating_add(cost);
                usage.active += 1;
                return Some(Permit {
                    gate: self.clone(),
                    cost,
                });
            }
            usage = self
                .changed
                .wait_timeout(usage, Duration::from_millis(50))
                .unwrap()
                .0;
        }
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut usage = self.gate.usage.lock().unwrap();
        usage.bytes -= self.cost;
        usage.active -= 1;
        self.gate.changed.notify_all();
    }
}

/// Events contain no image buffers. Dropping the receiver stops workers from taking further jobs.
pub fn start(
    paths: Vec<PathBuf>,
    output: PathBuf,
    settings: RenderSettings,
    cancel: Arc<AtomicBool>,
    options: BatchOptions,
) -> Result<Receiver<BatchEvent>> {
    let plan = options.plan(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        available_memory(),
        paths.len(),
    );
    let workers = plan.workers;
    let settings = Arc::new(settings);
    let paths = Arc::new(paths);
    let queue = Arc::new(Mutex::new(0usize));
    let gate = Arc::new(MemoryGate {
        budget: plan.memory_budget,
        usage: Mutex::new(Usage {
            bytes: 0,
            active: 0,
        }),
        changed: Condvar::new(),
        dynamic_memory: plan.dynamic_memory,
        baseline_process_memory: process_memory(),
    });
    let (sender, receiver) = mpsc::channel();
    let _ = sender.send(BatchEvent::Configured {
        workers,
        memory_budget: plan.memory_budget,
    });
    let mut launched = 0;
    for worker in 0..workers {
        let (paths, queue, gate, cancel, sender, settings, output) = (
            paths.clone(),
            queue.clone(),
            gate.clone(),
            cancel.clone(),
            sender.clone(),
            settings.clone(),
            output.clone(),
        );
        let launch = std::thread::Builder::new()
            .name(format!("photo-export-{worker}"))
            .spawn(move || {
                let mut engine = None;
                loop {
                    let index = {
                        let mut next = queue.lock().unwrap();
                        if cancel.load(Ordering::Relaxed) || *next >= paths.len() {
                            return;
                        }
                        let index = *next;
                        *next += 1;
                        index
                    };
                    let path = &paths[index];
                    let estimate = estimate_peak_bytes(path, &settings);
                    let (result, permit) = match estimate {
                        Err(error) => (Err(error), None),
                        Ok(cost) => {
                            let Some(permit) = gate.acquire(cost, &cancel) else {
                                return;
                            };
                            if sender
                                .send(BatchEvent::Started {
                                    index,
                                    estimated_bytes: cost,
                                })
                                .is_err()
                            {
                                cancel.store(true, Ordering::Relaxed);
                                return;
                            }
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    render::export_profiled(
                                        engine.get_or_insert_with(Engine::default),
                                        path,
                                        &output,
                                        &settings,
                                    )
                                }))
                                .unwrap_or_else(|_| Err(anyhow!(tr!("errors.export_worker"))));
                            (result, Some(permit))
                        }
                    };
                    let delivered = sender.send(BatchEvent::Finished {
                        index,
                        source: path.clone(),
                        result,
                    });
                    drop(permit);
                    if delivered.is_err() {
                        cancel.store(true, Ordering::Relaxed);
                        return;
                    }
                }
            });
        match launch {
            Ok(_) => launched += 1,
            Err(error) if launched == 0 => return Err(error.into()),
            Err(_) => break, // The existing worker can drain the same queue on its own.
        }
    }
    Ok(receiver)
}

/// Conservative working-set estimate, including full input decode and intermediate output buffers.
pub fn estimate_peak_bytes(path: &Path, settings: &RenderSettings) -> Result<u64> {
    let decoder = image::ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let (w, h) = decoder.dimensions();
    // Decoding/orientation may retain both the source and a transformed buffer.
    let input_bytes = u64::from(decoder.color_type().bytes_per_pixel()).max(4) * 2;
    let pixels = u64::from(w) * u64::from(h);
    anyhow::ensure!(pixels <= 100_000_000, tr!("errors.input_limit"));
    let mut settings = settings.clone();
    settings.normalize();
    let output_pixels = [(w, h), (h, w)]
        .into_iter()
        .map(|(w, h)| {
            let layout = render::export_layout(w, h, &settings);
            u64::from(layout.width) * u64::from(layout.height)
        })
        .max()
        .unwrap_or(0);
    Ok(pixels
        .saturating_mul(input_bytes)
        .saturating_add(output_pixels.saturating_mul(20))
        .saturating_add(64 * MIB))
}

#[cfg(windows)]
fn available_memory() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page: 0,
        avail_page: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended: 0,
    };
    // SAFETY: status has the documented MEMORYSTATUSEX layout and initializes its size field.
    if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
        Some(status.avail_phys)
    } else {
        None
    }
}

#[cfg(windows)]
fn process_memory() -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCounters {
        size: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_nonpaged: usize,
        quota_nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut counters = ProcessMemoryCounters {
        size: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        ..Default::default()
    };
    // SAFETY: the counter layout and size match PROCESS_MEMORY_COUNTERS. The current
    // process pseudo-handle is always valid and must not be closed.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.size) } != 0 {
        Some(counters.working_set as u64)
    } else {
        None
    }
}

#[cfg(not(windows))]
fn process_memory() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("VmRSS:"))?
                .split_whitespace()
                .nth(1)?
                .parse::<u64>()
                .ok()
                .map(|kb| kb * 1024)
        })
}

#[cfg(not(windows))]
fn available_memory() -> Option<u64> {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("MemAvailable:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    const GIB: u64 = 1024 * MIB;

    #[test]
    fn automatic_plan_uses_resources_instead_of_two_worker_cap() {
        let options = BatchOptions::default();
        let roomy = options.plan(16, Some(48 * GIB), 100);
        assert_eq!(roomy.workers, 15);
        assert_eq!(roomy.memory_budget, 48 * GIB);
        assert!(roomy.dynamic_memory);
        let constrained = options.plan(16, Some(384 * MIB), 100);
        assert_eq!(constrained.workers, 15);
        assert_eq!(constrained.memory_budget, 384 * MIB);
        assert_eq!(options.plan(1, Some(48 * GIB), 100).workers, 1);
        assert_eq!(options.plan(16, Some(48 * GIB), 3).workers, 3);
        assert_eq!(options.plan(16, Some(48 * GIB), 0).workers, 0);
    }

    #[test]
    fn missing_memory_information_falls_back_to_one_worker() {
        let plan = BatchOptions::default().plan(64, None, 100);
        assert_eq!(plan.workers, 1);
        assert_eq!(plan.memory_budget, 256 * MIB);
        let diagnostic = BatchOptions {
            workers: 4,
            memory_budget: 2 * GIB,
        }
        .plan(1, None, 10);
        assert_eq!(diagnostic.workers, 4);
        assert!(!diagnostic.dynamic_memory);
    }

    #[test]
    fn memory_pressure_blocks_new_work_without_cancelling_inflight_work() {
        let gate = MemoryGate {
            budget: 4 * GIB,
            dynamic_memory: true,
            baseline_process_memory: Some(0),
            usage: Mutex::new(Usage {
                bytes: 2 * GIB,
                active: 2,
            }),
            changed: Condvar::new(),
        };
        assert_eq!(
            gate.budget_for_available(2 * GIB, Some(10 * GIB), Some(2 * GIB)),
            12 * GIB
        );
        let reduced = gate.budget_for_available(2 * GIB, Some(GIB / 2), Some(2 * GIB));
        assert_eq!(reduced, 2 * GIB + GIB / 2);
        assert!(2 * GIB + GIB > reduced, "A further 1 GiB job must wait");
        assert_eq!(
            gate.budget_for_available(2 * GIB, Some(0), Some(2 * GIB)),
            2 * GIB
        );
        assert_eq!(gate.usage.lock().unwrap().active, 2);
        assert_eq!(gate.budget_for_available(2 * GIB, None, Some(2 * GIB)), 0);
        assert!(gate.budget_for_available(2 * GIB, Some(100 * GIB), Some(2 * GIB)) > 4 * GIB);
    }

    #[test]
    fn pending_reservations_are_not_credited_as_allocated_memory() {
        let gate = MemoryGate {
            budget: 2 * GIB,
            dynamic_memory: true,
            baseline_process_memory: Some(100 * MIB),
            usage: Mutex::new(Usage {
                bytes: 0,
                active: 0,
            }),
            changed: Condvar::new(),
        };
        assert_eq!(
            gate.budget_for_available(2 * GIB, Some(6 * GIB), Some(100 * MIB)),
            6 * GIB
        );
        // Allocating the admitted buffers leaves the same resource envelope.
        assert_eq!(
            gate.budget_for_available(2 * GIB, Some(4 * GIB), Some(2 * GIB + 100 * MIB)),
            6 * GIB
        );
        // Without a resident-memory query, pending work stays bounded by the startup budget.
        assert_eq!(
            gate.budget_for_available(2 * GIB, Some(100 * GIB), None),
            2 * GIB
        );
    }
}
