//! Main synchronization orchestration logic
//!
//! Wires together: reader → differ → resolver → forward/reverse sync → reporter

use crate::differ::detect_diffs;
use crate::reader::{read_generated_code, read_ontology};
use crate::reporter::report_sync_status;
use crate::resolver::{ResolutionStrategy, ResolverConfig, SyncAction, resolve_conflicts};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Reader error: {0}")]
    Reader(#[from] crate::reader::ReaderError),

    #[error("Resolver error: {0}")]
    Resolver(#[from] crate::resolver::ResolverError),

    #[error("Sync failed: {0}")]
    Failed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Sync from ontology to generated code (forward)
    Forward,
    /// Sync from generated code to ontology (reverse)
    Reverse,
    /// Sync in both directions
    Both,
}

/// Main sync function - orchestrates the entire pipeline
pub fn sync(ontology_path: &Path, generated_path: &Path, direction: SyncDirection) -> Result<()> {
    println!("Starting sync:");
    println!("  Ontology:  {}", ontology_path.display());
    println!("  Generated: {}", generated_path.display());
    println!("  Direction: {:?}", direction);
    println!();

    // Step 1: Read ontology and generated code
    println!("[1/5] Reading ontology...");
    let ontology = read_ontology(ontology_path)?;
    println!("      Found {} types in ontology", ontology.len());

    println!("[2/5] Reading generated code...");
    let code = read_generated_code(generated_path)?;
    println!("      Found {} types in generated code", code.len());

    // Step 2: Detect differences
    println!("[3/5] Detecting differences...");
    let diffs = detect_diffs(&ontology, &code);
    println!("      Detected {} differences", diffs.len());

    if diffs.is_empty() {
        println!("\nNo differences detected. Ontology and code are in sync.");
        return Ok(());
    }

    // Step 3 & 4: Resolve conflicts and execute sync based on direction
    match direction {
        SyncDirection::Forward => {
            println!("[4/5] Resolving conflicts (forward: ontology → code)...");
            let config = ResolverConfig::auto(ResolutionStrategy::TakeOntology);
            let actions = resolve_conflicts(diffs.clone(), &config)?;
            println!("      Planned {} actions", actions.len());

            println!("[5/5] Executing forward sync...");
            execute_sync(&actions)?;
        }
        SyncDirection::Reverse => {
            println!("[4/5] Resolving conflicts (reverse: code → ontology)...");
            let config = ResolverConfig::auto(ResolutionStrategy::TakeCode);
            let actions = resolve_conflicts(diffs.clone(), &config)?;
            println!("      Planned {} actions", actions.len());

            println!("[5/5] Executing reverse sync...");
            execute_sync(&actions)?;
        }
        SyncDirection::Both => {
            println!("[4/5] Resolving conflicts (bidirectional)...");
            // For bidirectional, we need manual resolution
            let config = ResolverConfig::auto(ResolutionStrategy::Merge);
            let actions = resolve_conflicts(diffs.clone(), &config)?;
            println!(
                "      Planned {} actions (manual review required)",
                actions.len()
            );

            println!("[5/5] Generating sync plan (no automatic execution)...");
            // Don't execute, just report what needs to be done
        }
    }

    // Step 5: Generate report
    println!("\n=== Sync Report ===");
    report_sync_status(&diffs);

    Ok(())
}

/// Execute sync actions
fn execute_sync(actions: &[SyncAction]) -> Result<()> {
    for action in actions {
        match action {
            SyncAction::GenerateCode { type_name } => {
                println!("      → Generate code for: {}", type_name);
                // TODO: Call code generation
            }
            SyncAction::UpdateCode { type_name } => {
                println!("      → Update code for: {}", type_name);
                // TODO: Update existing code
            }
            SyncAction::UpdateOntology { type_name } => {
                println!("      → Update ontology for: {}", type_name);
                // TODO: Update RDF ontology
            }
            SyncAction::Skip { type_name } => {
                println!("      ⊘ Skipping: {}", type_name);
            }
        }
    }

    Ok(())
}
