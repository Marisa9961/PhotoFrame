use crate::tr;
mod memory;
pub use memory::estimate_peak_bytes;
use memory::{available_memory, process_memory};

use crate::{
    render::{self, Engine, ExportReport},
    settings::Settings,
};
use anyhow::{Result, anyhow};
use std::{
    path::PathBuf,
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
    settings: Settings,
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

#[cfg(test)]
mod tests;
