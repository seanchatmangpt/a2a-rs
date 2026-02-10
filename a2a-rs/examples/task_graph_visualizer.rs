//! Task Graph Visualizer Example
//!
//! Demonstrates how to create a task graph and export it to DOT format
//! for visualization with Graphviz.
//!
//! Run with: cargo run --example task_graph_visualizer
//!
//! To generate a PNG image (requires graphviz installed):
//! cargo run --example task_graph_visualizer > graph.dot
//! dot -Tpng graph.dot -o graph.png

use a2a_rs::construct::{TaskGraph, TaskGraphVisualizer};
use a2a_rs::domain::{Task, TaskState};

fn main() {
    // Create a new task graph
    let mut graph = TaskGraph::new();

    // Create a workflow: data preparation -> model training -> evaluation
    let mut prep = Task::new("data-prep".to_string(), "ml-workflow".to_string());
    prep.status.state = TaskState::Completed;

    let mut train = Task::new("model-training".to_string(), "ml-workflow".to_string());
    train.status.state = TaskState::Working;

    let mut eval = Task::new("evaluation".to_string(), "ml-workflow".to_string());
    eval.status.state = TaskState::Submitted;

    // Add tasks to graph
    graph.add_task(prep).expect("Failed to add prep task");
    graph
        .add_task(train.clone())
        .expect("Failed to add train task");
    graph.add_task(eval).expect("Failed to add eval task");

    // Define dependencies
    graph
        .add_dependency("model-training", "data-prep")
        .expect("Failed to add dependency");
    graph
        .add_dependency("evaluation", "model-training")
        .expect("Failed to add dependency");

    // Generate DOT format
    let dot = graph.to_dot();

    // Print to stdout (can be redirected to file)
    println!("{}", dot);

    // Also demonstrate accessing blocking information
    eprintln!("\n=== Task Graph Analysis ===");
    eprintln!("Total tasks: {}", graph.task_count());
    eprintln!("Total dependencies: {}", graph.edge_count());

    // Check which tasks are ready to execute
    let ready = graph.ready_tasks();
    if ready.is_empty() {
        eprintln!("No tasks ready to execute");
    } else {
        eprintln!("Tasks ready to execute: {:?}", ready);
    }

    // Check prerequisites for evaluation task
    match graph.unmet_prerequisites("evaluation") {
        Ok(unmet) => {
            if unmet.is_empty() {
                eprintln!("evaluation: All prerequisites met");
            } else {
                eprintln!("evaluation blocked by: {:?}", unmet);
            }
        }
        Err(e) => eprintln!("Error checking prerequisites: {}", e),
    }

    eprintln!("\n=== Output Instructions ===");
    eprintln!("To save the graph to a file:");
    eprintln!("  cargo run --example task_graph_visualizer > graph.dot");
    eprintln!("\nTo render as PNG (requires graphviz):");
    eprintln!("  dot -Tpng graph.dot -o graph.png");
    eprintln!("\nTo render as SVG:");
    eprintln!("  dot -Tsvg graph.dot -o graph.svg");
}
