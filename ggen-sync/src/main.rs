mod cli;

use clap::Parser;
use cli::{Cli, Commands};
use ggen_sync::sync;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
enum MainError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ontology path does not exist: {0}")]
    OntologyNotFound(String),

    #[error("Generated path does not exist: {0}")]
    GeneratedNotFound(String),

    #[error("Sync error: {0}")]
    Sync(#[from] ggen_sync::SyncError),
}

type Result<T> = std::result::Result<T, MainError>;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Sync {
            ontology,
            generated,
            direction,
        } => sync_command(&ontology, &generated, direction),
    }
}

fn sync_command(ontology: &Path, generated: &Path, direction: cli::SyncDirection) -> Result<()> {
    // Validate paths exist
    if !ontology.exists() {
        return Err(MainError::OntologyNotFound(ontology.display().to_string()));
    }

    if !generated.exists() {
        return Err(MainError::GeneratedNotFound(
            generated.display().to_string(),
        ));
    }

    // Convert CLI enum to library enum
    let lib_direction = match direction {
        cli::SyncDirection::Forward => ggen_sync::SyncDirection::Forward,
        cli::SyncDirection::Reverse => ggen_sync::SyncDirection::Reverse,
        cli::SyncDirection::Both => ggen_sync::SyncDirection::Both,
    };

    // Call the main sync orchestration function
    sync(ontology, generated, lib_direction)?;

    println!("\nSync completed successfully");
    Ok(())
}
