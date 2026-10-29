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
