//! Demonstration of the immutable artifact storage system.
//!
//! This example shows how to use the ArtifactStore to manage artifacts
//! with content-addressed storage, append-only semantics, and commit operations.

use a2a_rs::construct::{ArtifactStore, InMemoryArtifactStore};
use a2a_rs::domain::{Artifact, Part};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Artifact Storage Demo ===\n");

    // Create the artifact store
    let store = InMemoryArtifactStore::new();
    println!("Created in-memory artifact store\n");

    // Create some test artifacts
    let artifact1 = Artifact {
        artifact_id: "report-001".to_string(),
        name: Some("Analysis Report".to_string()),
        description: Some("Initial analysis of the data".to_string()),
        parts: vec![Part::text("This is the analysis content...".to_string())],
        metadata: None,
        extensions: None,
    };

    let artifact2 = Artifact {
        artifact_id: "chart-001".to_string(),
        name: Some("Data Chart".to_string()),
        description: Some("Visualization of results".to_string()),
        parts: vec![Part::text("Chart data goes here...".to_string())],
        metadata: None,
        extensions: None,
    };

    // Append artifacts to task
    println!("Appending artifacts to task 'task-123'...");
    let hash1 = store.append("task-123", artifact1.clone())?;
    println!(
        "  - Stored artifact 'report-001' with hash: {}",
        hash1.as_str()
    );

    let hash2 = store.append("task-123", artifact2.clone())?;
    println!(
        "  - Stored artifact 'chart-001' with hash: {}\n",
        hash2.as_str()
    );

    // List artifacts for the task
    println!("Artifacts for task 'task-123':");
    let artifacts = store.list_by_task("task-123")?;
    for (i, artifact) in artifacts.iter().enumerate() {
        println!(
            "  {}. {} - committed: {}",
            i + 1,
            artifact.artifact.artifact_id,
            artifact.committed
        );
    }
    println!();

    // Commit individual artifact
    println!("Committing artifact 'report-001'...");
    store.commit("task-123", "report-001")?;

    let stored = store.get_by_id("task-123", "report-001")?;
    println!("  - Artifact committed: {}\n", stored.committed);

    // Commit entire task
    println!("Committing all artifacts for task 'task-123'...");
    store.commit_task("task-123")?;

    let task_artifacts = store.get_task_artifacts("task-123")?;
    println!("  - Task finalized: {}", task_artifacts.finalized);
    println!(
        "  - All artifacts committed: {}\n",
        task_artifacts.artifacts.iter().all(|a| a.committed)
    );

    // Try to append after finalization (should fail)
    println!("Attempting to append artifact after task finalization...");
    let artifact3 = Artifact {
        artifact_id: "extra-001".to_string(),
        name: Some("Extra Artifact".to_string()),
        description: None,
        parts: vec![Part::text("This should fail...".to_string())],
        metadata: None,
        extensions: None,
    };

    match store.append("task-123", artifact3) {
        Ok(_) => println!("  - ERROR: Should have failed!"),
        Err(e) => println!("  - Expected error: {}\n", e),
    }

    // Retrieve by content hash
    println!("Retrieving artifact by content hash...");
    let retrieved = store.get_by_hash(&hash1)?;
    println!("  - Retrieved: {}", retrieved.artifact.artifact_id);
    println!(
        "  - Content matches: {}",
        retrieved.artifact.artifact_id == artifact1.artifact_id
    );

    println!("\n=== Demo Complete ===");
    Ok(())
}
