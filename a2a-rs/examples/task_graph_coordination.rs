//! Example demonstrating task graph coordination topology
//!
//! This example shows how to use TaskGraph for managing task dependencies,
//! including prerequisite checking, join semantics, and cancellation propagation.

use a2a_rs::construct::{CoordinationError, TaskGraph};
use a2a_rs::domain::{Artifact, Part, Task, TaskState};

fn main() -> Result<(), CoordinationError> {
    println!("=== Task Graph Coordination Example ===\n");

    // Create a task graph
    let mut graph = TaskGraph::new();
    println!("Created empty task graph");

    // Create tasks
    let task_a = Task::new("fetch-data".to_string(), "ctx-1".to_string());
    let task_b = Task::new("process-data".to_string(), "ctx-1".to_string());
    let task_c = Task::new("validate-data".to_string(), "ctx-1".to_string());
    let task_d = Task::new("save-results".to_string(), "ctx-1".to_string());

    // Add tasks to graph
    graph.add_task(task_a)?;
    graph.add_task(task_b)?;
    graph.add_task(task_c)?;
    graph.add_task(task_d)?;
    println!("Added 4 tasks to graph\n");

    // Define dependencies:
    // task_b depends on task_a (sequential)
    // task_c depends on task_a (fan-out)
    // task_d depends on both task_b and task_c (join)
    graph.add_dependency("process-data", "fetch-data")?;
    graph.add_dependency("validate-data", "fetch-data")?;
    graph.add_dependency("save-results", "process-data")?;
    graph.add_dependency("save-results", "validate-data")?;

    println!("Dependency graph:");
    println!("  fetch-data (root)");
    println!("    -> process-data");
    println!("    -> validate-data");
    println!("  process-data & validate-data (join)");
    println!("    -> save-results (leaf)\n");

    // Check which tasks are ready to execute
    let ready = graph.ready_tasks();
    println!("Tasks ready to execute: {:?}", ready);
    println!("Expected: only fetch-data (no prerequisites)\n");

    // Complete fetch-data and add an artifact
    if let Some(task) = graph.get_task_mut("fetch-data") {
        task.status.state = TaskState::Completed;
        let artifact = Artifact {
            artifact_id: "data-001".to_string(),
            name: Some("raw_data.json".to_string()),
            description: Some("Raw data fetched from API".to_string()),
            parts: vec![Part::text("{\"count\": 42}".to_string())],
            metadata: None,
            extensions: None,
        };
        task.add_artifact(artifact);
        println!("Completed fetch-data with artifact");
    }

    // Check ready tasks again
    let ready = graph.ready_tasks();
    println!("Tasks ready to execute: {:?}", ready);
    println!("Expected: process-data and validate-data (fetch-data completed)\n");

    // Complete process-data
    if let Some(task) = graph.get_task_mut("process-data") {
        task.status.state = TaskState::Completed;
        println!("Completed process-data");
    }

    // Check save-results prerequisites
    let can_run = graph.prerequisites_met("save-results")?;
    println!(
        "Can save-results run? {} (validate-data still pending)",
        can_run
    );

    // Complete validate-data (join point)
    if let Some(task) = graph.get_task_mut("validate-data") {
        task.status.state = TaskState::Completed;
        println!("Completed validate-data");
    }

    // Check save-results prerequisites again
    let can_run = graph.prerequisites_met("save-results")?;
    println!(
        "Can save-results run? {} (all prerequisites met)\n",
        can_run
    );

    // Get all prerequisite artifacts for save-results
    let artifacts = graph.get_all_prerequisite_artifacts("save-results")?;
    println!(
        "Artifacts available to save-results: {} artifact(s)",
        artifacts.len()
    );
    for artifact in &artifacts {
        println!("  - {}", artifact.artifact_id);
    }
    println!();

    // Demonstrate cancellation propagation
    println!("=== Cancellation Propagation Demo ===\n");

    let mut graph2 = TaskGraph::new();
    let mut task_x = Task::new("task-x".to_string(), "ctx-2".to_string());
    task_x.status.state = TaskState::Working;
    let task_y = Task::new("task-y".to_string(), "ctx-2".to_string());
    let task_z = Task::new("task-z".to_string(), "ctx-2".to_string());

    graph2.add_task(task_x)?;
    graph2.add_task(task_y)?;
    graph2.add_task(task_z)?;
    graph2.add_dependency("task-y", "task-x")?;
    graph2.add_dependency("task-z", "task-y")?;

    println!("Created chain: task-x -> task-y -> task-z");
    println!("Canceling task-x...");

    let canceled = graph2.propagate_cancellation("task-x")?;
    println!("Canceled tasks: {:?}", canceled);
    println!("Expected: all three tasks (cascading cancellation)\n");

    // Demonstrate termination detection
    println!("=== Termination Detection ===\n");
    println!("Graph 1 terminated? {}", graph.is_terminated());
    println!("Expected: false (save-results still in Submitted state)");

    if let Some(task) = graph.get_task_mut("save-results") {
        task.status.state = TaskState::Completed;
    }

    println!(
        "After completing save-results, terminated? {}",
        graph.is_terminated()
    );
    println!("Expected: true (all tasks in terminal states)\n");

    // Demonstrate topological sort
    println!("=== Topological Sort ===\n");
    let sorted = graph.topological_sort()?;
    println!("Task execution order: {:?}", sorted);
    println!("Note: fetch-data comes before its dependents\n");

    println!("=== Example Complete ===");

    Ok(())
}
