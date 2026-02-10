//! Sync detector for ontology vs generated code drift
//!
//! Compares RDF ontology definitions with generated Rust code to detect:
//! - Added types (in ontology, not in code)
//! - Removed types (in code, not in ontology)
//! - Modified types (field additions, removals, type changes)

pub mod code_reader;
pub mod differ;
pub mod forward_sync;
pub mod generate;
pub mod migrate;
pub mod ontology_reader;
pub mod reader;
pub mod reporter;
pub mod resolver;
pub mod reverse_sync;
pub mod sync;
pub mod types;

pub use code_reader::{CodeReaderError, parse_code, parse_file};
pub use differ::detect_diffs;
pub use forward_sync::{ForwardSyncError, apply_forward_sync};
pub use generate::{
    GenerateError, GenerationConfig, GenerationResult, GgenConfig, OntologyConfig, ProjectConfig,
    RuleConfig, generate, load_config,
};
pub use migrate::{
    BreakingChange, DatabaseBackend, Migration, MigrationError, apply_migrations,
    detect_breaking_changes, generate_migrations,
};
pub use ontology_reader::{OntologyError, parse_ttl_directory, parse_ttl_file};
pub use reader::{ReaderError, read_generated_code, read_ontology};
pub use reporter::{report_sync_json, report_sync_status, write_sync_json};
pub use resolver::{ResolutionStrategy, ResolverConfig, SyncAction, resolve_conflicts};
pub use reverse_sync::{ReverseSyncError, apply_reverse_sync};
pub use sync::{SyncDirection, SyncError, sync};
pub use types::{CodeNode, FieldChange, FieldDef, OntologyNode, SyncDiff};
