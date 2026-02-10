//! Observability Integration Demo
//!
//! This example demonstrates the comprehensive observability features
//! of the CONSTRUCT runtime, including:
//! - Metrics tracking for operations, guards, and invariants
//! - Distributed tracing with span context
//! - Timing instrumentation for performance analysis
//! - Instrumented wrappers for guards and invariants
//!
//! Run with:
//! ```bash
//! RUST_LOG=debug cargo run --example observability_demo --features tracing
//! ```

use a2a_rs::construct::{
    guards::{Guard, RangeGuard, RefusalCode, TypeGuard},
    invariants::{Invariant, InvariantRegistry, TaskStateInvariant},
    observability::{
        InstrumentedGuard, InstrumentedInvariant, ObservabilityContext, RuntimeMetrics,
    },
    runtime::{Operation, Runtime},
};
use a2a_rs::domain::{Task, TaskStatus};

fn main() {
    // Initialize tracing subscriber for console output
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("observability_demo=debug".parse().unwrap())
                .add_directive("a2a_rs=debug".parse().unwrap()),
        )
        .init();

    println!("=== CONSTRUCT Observability Demo ===\n");

    // 1. Create metrics registry
    println!("1. Creating metrics registry...");
    let metrics = RuntimeMetrics::new();

    // 2. Create observability context
    println!("2. Creating observability context...");
    let execution_id = uuid::Uuid::new_v4().to_string();
    let ctx = ObservabilityContext::with_metrics(execution_id.clone(), 1, metrics.clone());
    println!("   Execution ID: {}", ctx.execution_id);
    println!("   Policy Epoch: {}\n", ctx.policy_epoch);

    // 3. Demonstrate guard instrumentation
    println!("3. Testing instrumented guards...");

    // Create a type guard and wrap it with instrumentation
    let type_guard = TypeGuard::new("number".to_string());
    let instrumented_guard = InstrumentedGuard::new(type_guard, metrics.clone());

    // Test valid input
    let valid_input = serde_json::json!(42);
    match instrumented_guard.check(&valid_input, "test_field", ctx.policy_epoch) {
        Ok(_) => println!("   ✓ Valid input admitted"),
        Err(e) => println!("   ✗ Unexpected rejection: {}", e),
    }

    // Test invalid input
    let invalid_input = serde_json::json!("not a number");
    match instrumented_guard.check(&invalid_input, "test_field", ctx.policy_epoch) {
        Ok(_) => println!("   ✗ Invalid input should have been rejected"),
        Err(receipt) => {
            println!("   ✓ Invalid input rejected");
            println!("      Code: {:?}", receipt.code);
            println!("      Reason: {}", receipt.reason);
        }
    }
    println!();

    // 4. Demonstrate range guard with metrics
    println!("4. Testing range guard with metrics...");
    let range_guard = RangeGuard::new(Some(0.0), Some(100.0));
    let instrumented_range = InstrumentedGuard::new(range_guard, metrics.clone());

    let test_values = vec![
        (serde_json::json!(50), true),
        (serde_json::json!(150), false),
        (serde_json::json!(-10), false),
    ];

    for (value, should_pass) in test_values {
        let result = instrumented_range.check(&value, "range_test", ctx.policy_epoch);
        match (result.is_ok(), should_pass) {
            (true, true) => println!("   ✓ {} admitted (expected)", value),
            (false, false) => println!("   ✓ {} rejected (expected)", value),
            (true, false) => println!("   ✗ {} admitted (unexpected)", value),
            (false, true) => println!("   ✗ {} rejected (unexpected)", value),
        }
    }
    println!();

    // 5. Demonstrate invariant instrumentation
    println!("5. Testing instrumented invariants...");

    let task_invariant = TaskStateInvariant::new();
    let instrumented_invariant = InstrumentedInvariant::new(task_invariant, metrics.clone());

    // Create a valid task
    let valid_task = Task::builder()
        .id("task-1".to_string())
        .context_id("ctx-1".to_string())
        .status(TaskStatus::default())
        .build();

    match instrumented_invariant.check(&valid_task) {
        Ok(_) => println!("   ✓ Valid task passed invariant check"),
        Err(e) => println!("   ✗ Unexpected invariant violation: {}", e),
    }
    println!();

    // 6. Demonstrate scheduler metrics
    println!("6. Simulating scheduler operations...");
    metrics.record_task_submitted("task-1", "normal");
    metrics.record_scheduler_selection("task-1");
    metrics.record_task_completed("task-1");

    metrics.record_task_submitted("task-2", "high");
    metrics.record_task_cancelled("task-2");
    println!("   ✓ Recorded scheduler operations\n");

    // 7. Display metrics snapshot
    println!("7. Metrics Snapshot:");
    let snapshot = metrics.snapshot();
    println!("   Guard admissions:        {}", snapshot.guard_admissions);
    println!("   Guard rejections:        {}", snapshot.guard_rejections);
    println!(
        "   Guard rejection rate:    {:.2}%",
        snapshot.guard_rejection_rate() * 100.0
    );
    println!(
        "   Invariant checks passed: {}",
        snapshot.invariant_checks_passed
    );
    println!(
        "   Invariant checks failed: {}",
        snapshot.invariant_checks_failed
    );
    println!("   Tasks submitted:         {}", snapshot.tasks_submitted);
    println!("   Tasks completed:         {}", snapshot.tasks_completed);
    println!("   Tasks cancelled:         {}", snapshot.tasks_cancelled);
    println!(
        "   Scheduler selections:    {}",
        snapshot.scheduler_selections
    );
    println!("   Total errors:            {}", snapshot.total_errors());
    println!(
        "   Task completion rate:    {:.2}%\n",
        snapshot.task_completion_rate() * 100.0
    );

    // 8. Demonstrate runtime execution timing
    println!("8. Creating operation timing...");
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut timing = ctx.create_timing("create_task".to_string());
    timing.add_stage("type_check".to_string(), 10);
    timing.add_stage("admission_guard".to_string(), 20);
    timing.add_stage("transformations".to_string(), 30);
    timing.add_stage("invariants".to_string(), 15);
    timing.add_stage("delta_execution".to_string(), 25);

    println!("   Operation: {}", timing.operation);
    println!("   Duration: {}ms", timing.duration_ms);
    println!("   Stages:");
    for (stage, duration_ms) in &timing.stages {
        println!("     - {}: {}ms", stage, duration_ms);
    }

    if let Some((slowest, duration)) = timing.slowest_stage() {
        println!("   Slowest stage: {} ({}ms)", slowest, duration);
    }
    println!();

    // 9. Demonstrate full runtime execution with observability
    println!("9. Running full runtime execution with observability...");
    let mut runtime = Runtime::default_runtime().with_policy_epoch(ctx.policy_epoch);

    let task = Task::builder()
        .id("task-obs-1".to_string())
        .context_id("ctx-obs-1".to_string())
        .status(TaskStatus::default())
        .build();

    let operation = Operation::CreateTask {
        task,
        initial_message: None,
        priority: None,
    };

    match runtime.handle(operation) {
        Ok(output) => {
            println!("   ✓ Operation executed successfully");
            println!("      Execution ID: {}", output.receipt.execution_id);
            println!("      Duration: {}ms", output.receipt.duration_ms);
            println!("      Stages: {:?}", output.receipt.stages_completed);
            println!("      Success: {}", output.receipt.success);
            println!("      Events: {}", output.events.len());
        }
        Err(e) => println!("   ✗ Operation failed: {}", e),
    }
    println!();

    println!("=== Demo Complete ===");
    println!("\nKey Takeaways:");
    println!("• Metrics provide low-overhead counters for all operations");
    println!("• Instrumented wrappers add observability to guards and invariants");
    println!("• Timing information enables performance analysis");
    println!("• ObservabilityContext provides correlation across distributed traces");
    println!("• All instrumentation is feature-gated for zero-cost when disabled");
}
