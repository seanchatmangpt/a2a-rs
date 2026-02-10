//! Scheduler Performance Benchmarks
//!
//! Measures performance of the deterministic scheduler (Λ) with various workloads,
//! comparing BTreeMap vs HashMap to quantify the cost of determinism.

use a2a_rs::construct::runtime::scheduler::{PriorityClass, ScheduledTask, Scheduler};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::collections::HashMap;

// ==================== Helper Functions ====================

/// Generate a batch of tasks for benchmarking
fn generate_tasks(count: usize, num_stations: usize) -> Vec<ScheduledTask> {
    let mut tasks = Vec::with_capacity(count);
    let priorities = [
        PriorityClass::Critical,
        PriorityClass::High,
        PriorityClass::Normal,
        PriorityClass::Low,
        PriorityClass::Idle,
    ];

    for i in 0..count {
        let task_id = format!("task-{:08}", i);
        let station_id = format!("station-{}", i % num_stations);
        let epoch = (i / 100) as u64; // Group into epochs
        let priority = priorities[i % priorities.len()];

        tasks.push(ScheduledTask::new(task_id, station_id, epoch, priority));
    }

    tasks
}

/// Simulate workload: submit -> next -> complete cycle
fn simulate_workload(scheduler: &mut Scheduler, tasks: Vec<ScheduledTask>) {
    // Submit all tasks
    for task in tasks {
        let _ = scheduler.submit(task);
    }

    // Process all tasks
    while let Some(task) = scheduler.next() {
        // Simulate work by immediately completing
        let _ = scheduler.complete(&task.task_id, &task.station_id);
    }
}

// ==================== Core Scheduler Benchmarks ====================

/// Benchmark task submission at various scales
fn bench_scheduler_submit(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler_submit");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter(|| {
                let mut scheduler = Scheduler::new(10);

                // Register stations
                for i in 0..num_stations {
                    scheduler.register_station(format!("station-{}", i), 10);
                }

                // Submit all tasks
                for task in tasks {
                    let _ = scheduler.submit(black_box(task.clone()));
                }

                black_box(scheduler)
            })
        });
    }

    group.finish();
}

/// Benchmark task selection (next operation) at various scales
fn bench_scheduler_next(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler_next");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter_batched(
                || {
                    // Setup: create scheduler with all tasks submitted
                    let mut scheduler = Scheduler::new(10);
                    for i in 0..num_stations {
                        scheduler.register_station(format!("station-{}", i), 10);
                    }
                    for task in tasks {
                        let _ = scheduler.submit(task.clone());
                    }
                    scheduler
                },
                |mut scheduler| {
                    // Measure: execute all next() calls
                    while let Some(task) = scheduler.next() {
                        // Complete immediately to free up WIP slots
                        let _ = scheduler.complete(&task.task_id, &task.station_id);
                    }
                    black_box(scheduler)
                },
                criterion::BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

/// Benchmark full workload cycle (submit -> next -> complete)
fn bench_scheduler_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler_workload");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter(|| {
                let mut scheduler = Scheduler::new(10);

                // Register stations
                for i in 0..num_stations {
                    scheduler.register_station(format!("station-{}", i), 10);
                }

                simulate_workload(black_box(&mut scheduler), black_box(tasks.clone()));
                black_box(scheduler)
            })
        });
    }

    group.finish();
}

// ==================== Stable Sort Performance ====================

/// Benchmark stable sorting performance (core of deterministic scheduling)
fn bench_stable_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("stable_sort");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter(|| {
                // Convert to vec and sort by scheduling key (what scheduler.next() does)
                let mut task_vec: Vec<_> = tasks.iter().collect();
                task_vec.sort_by_key(|t| t.scheduling_key());
                black_box(task_vec)
            })
        });
    }

    group.finish();
}

/// Benchmark unstable sort (for comparison with stable sort)
fn bench_unstable_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("unstable_sort");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter(|| {
                let mut task_vec: Vec<_> = tasks.iter().collect();
                task_vec.sort_unstable_by_key(|t| t.scheduling_key());
                black_box(task_vec)
            })
        });
    }

    group.finish();
}

// ==================== BTreeMap vs HashMap Comparison ====================

/// Benchmark BTreeMap insertion (deterministic, ordered)
fn bench_btreemap_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("btreemap_insertion");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter(|| {
                let mut map = std::collections::BTreeMap::new();
                for task in tasks {
                    map.insert(black_box(task.task_id.clone()), black_box(task.clone()));
                }
                black_box(map)
            })
        });
    }

    group.finish();
}

/// Benchmark HashMap insertion (non-deterministic, faster)
fn bench_hashmap_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_insertion");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(BenchmarkId::from_parameter(scale), &tasks, |b, tasks| {
            b.iter(|| {
                let mut map = HashMap::new();
                for task in tasks {
                    map.insert(black_box(task.task_id.clone()), black_box(task.clone()));
                }
                black_box(map)
            })
        });
    }

    group.finish();
}

/// Benchmark BTreeMap iteration (deterministic order)
fn bench_btreemap_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("btreemap_iteration");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        // Pre-populate map
        let mut map = std::collections::BTreeMap::new();
        for task in &tasks {
            map.insert(task.task_id.clone(), task.clone());
        }

        group.bench_function(BenchmarkId::from_parameter(scale), |b| {
            b.iter(|| {
                let collected: Vec<_> = map.values().collect();
                black_box(collected)
            })
        });
    }

    group.finish();
}

/// Benchmark HashMap iteration (non-deterministic order, but faster)
fn bench_hashmap_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_iteration");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        // Pre-populate map
        let mut map = HashMap::new();
        for task in &tasks {
            map.insert(task.task_id.clone(), task.clone());
        }

        group.bench_function(BenchmarkId::from_parameter(scale), |b| {
            b.iter(|| {
                let collected: Vec<_> = map.values().collect();
                black_box(collected)
            })
        });
    }

    group.finish();
}

/// Benchmark BTreeMap lookup
fn bench_btreemap_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("btreemap_lookup");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        // Pre-populate map
        let mut map = std::collections::BTreeMap::new();
        for task in &tasks {
            map.insert(task.task_id.clone(), task.clone());
        }

        // Create lookup keys
        let lookup_keys: Vec<_> = tasks.iter().map(|t| t.task_id.clone()).collect();

        group.bench_function(BenchmarkId::from_parameter(scale), |b| {
            b.iter(|| {
                for key in &lookup_keys {
                    black_box(map.get(key));
                }
            })
        });
    }

    group.finish();
}

/// Benchmark HashMap lookup
fn bench_hashmap_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_lookup");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        // Pre-populate map
        let mut map = HashMap::new();
        for task in &tasks {
            map.insert(task.task_id.clone(), task.clone());
        }

        // Create lookup keys
        let lookup_keys: Vec<_> = tasks.iter().map(|t| t.task_id.clone()).collect();

        group.bench_function(BenchmarkId::from_parameter(scale), |b| {
            b.iter(|| {
                for key in &lookup_keys {
                    black_box(map.get(key));
                }
            })
        });
    }

    group.finish();
}

// ==================== Priority and Fairness Benchmarks ====================

/// Benchmark scheduler with high priority contention
fn bench_priority_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_contention");

    let scale = 10_000;
    let num_stations = 5;

    // All tasks at Critical priority (worst case for sorting)
    let critical_tasks: Vec<_> = (0..scale)
        .map(|i| {
            ScheduledTask::new(
                format!("task-{:08}", i),
                format!("station-{}", i % num_stations),
                0,
                PriorityClass::Critical,
            )
        })
        .collect();

    group.throughput(Throughput::Elements(scale as u64));
    group.bench_function("all_critical", |b| {
        b.iter(|| {
            let mut scheduler = Scheduler::new(10);
            for i in 0..num_stations {
                scheduler.register_station(format!("station-{}", i), 10);
            }
            simulate_workload(black_box(&mut scheduler), black_box(critical_tasks.clone()));
            black_box(scheduler)
        })
    });

    // Mixed priorities (realistic case)
    let mixed_tasks = generate_tasks(scale, num_stations);
    group.bench_function("mixed_priority", |b| {
        b.iter(|| {
            let mut scheduler = Scheduler::new(10);
            for i in 0..num_stations {
                scheduler.register_station(format!("station-{}", i), 10);
            }
            simulate_workload(black_box(&mut scheduler), black_box(mixed_tasks.clone()));
            black_box(scheduler)
        })
    });

    group.finish();
}

/// Benchmark fair scheduling across stations
fn bench_fair_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("fair_scheduling");

    let scale = 10_000;

    // Test with varying number of stations
    let station_counts = [2, 5, 10, 20, 50];

    for &num_stations in station_counts.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_stations),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let mut scheduler = Scheduler::new(10);
                    for i in 0..num_stations {
                        scheduler.register_station(format!("station-{}", i), 10);
                    }
                    simulate_workload(black_box(&mut scheduler), black_box(tasks.clone()));
                    black_box(scheduler)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark WIP limit enforcement overhead
fn bench_wip_limits(c: &mut Criterion) {
    let mut group = c.benchmark_group("wip_limits");

    let scale = 10_000;
    let num_stations = 10;
    let tasks = generate_tasks(scale, num_stations);

    // Test with different WIP limits
    let wip_limits = [1, 5, 10, 20, 100];

    for &wip_limit in wip_limits.iter() {
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(wip_limit),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let mut scheduler = Scheduler::new(wip_limit);
                    for i in 0..num_stations {
                        scheduler.register_station(format!("station-{}", i), wip_limit);
                    }
                    simulate_workload(black_box(&mut scheduler), black_box(tasks.clone()));
                    black_box(scheduler)
                })
            },
        );
    }

    group.finish();
}

// ==================== Determinism Cost Analysis ====================

/// Benchmark full scheduler with BTreeMap (current implementation)
fn bench_scheduler_deterministic(c: &mut Criterion) {
    let mut group = c.benchmark_group("determinism_cost");

    let scales = [1_000, 10_000, 100_000];
    let num_stations = 10;

    for &scale in scales.iter() {
        let tasks = generate_tasks(scale, num_stations);
        group.throughput(Throughput::Elements(scale as u64));

        group.bench_with_input(
            BenchmarkId::new("btreemap_scheduler", scale),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let mut scheduler = Scheduler::new(10);
                    for i in 0..num_stations {
                        scheduler.register_station(format!("station-{}", i), 10);
                    }
                    simulate_workload(black_box(&mut scheduler), black_box(tasks.clone()));
                    black_box(scheduler)
                })
            },
        );
    }

    group.finish();
}

// ==================== Criterion Configuration ====================

criterion_group!(
    scheduler_benches,
    bench_scheduler_submit,
    bench_scheduler_next,
    bench_scheduler_workload,
    bench_stable_sort,
    bench_unstable_sort,
    bench_btreemap_insertion,
    bench_hashmap_insertion,
    bench_btreemap_iteration,
    bench_hashmap_iteration,
    bench_btreemap_lookup,
    bench_hashmap_lookup,
    bench_priority_contention,
    bench_fair_scheduling,
    bench_wip_limits,
    bench_scheduler_deterministic
);

criterion_main!(scheduler_benches);
