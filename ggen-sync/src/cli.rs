use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "ggen")]
#[command(about = "Code generation and synchronization tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Synchronize between ontology and generated code
    Sync {
        /// Path to ontology directory
        #[arg(long)]
        ontology: PathBuf,

        /// Path to generated code directory
        #[arg(long)]
        generated: PathBuf,

        /// Synchronization direction
        #[arg(long, value_enum)]
        direction: SyncDirection,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SyncDirection {
    /// Sync from ontology to generated code
    Forward,
    /// Sync from generated code to ontology
    Reverse,
    /// Sync in both directions
    Both,
}
