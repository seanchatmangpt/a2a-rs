//! Adapter implementations for the Osiris compiler.
//!
//! Adapters implement the port traits with concrete technologies
//! (databases, external services, etc.).

pub mod a2a_orchestrator;
pub mod bpmn_compiler;
pub mod construct8_writer;
#[cfg(feature = "firestore")]
pub mod firestore_state_store;
#[cfg(feature = "gcs")]
pub mod gcs_receipt_storage;
pub mod h_guard_evaluator;
pub mod in_memory_merkle_storage;
pub mod in_memory_writer;
pub mod kms_signer;
pub mod lambda_orderer;
pub mod persistent_merkle_storage;
pub mod q_invariant_verifier;
pub mod receipt_builder;
pub mod receipt_storage;
pub mod sigma_type_checker;
pub mod workflow_kernel;
#[cfg(feature = "workspace-publisher")]
pub mod workspace_publisher;

pub use a2a_orchestrator::RemoteA2AOrchestratorAdapter;
pub use bpmn_compiler::BpmnCompiler;
pub use construct8_writer::Construct8Writer;
#[cfg(feature = "firestore")]
pub use firestore_state_store::FirestoreStateStore;
#[cfg(feature = "gcs")]
pub use gcs_receipt_storage::{GcsConfig, GcsReceiptStorage};
pub use h_guard_evaluator::{EvaluationContext, GuardEvaluationError, HGuardEvaluatorAdapter};
pub use in_memory_merkle_storage::InMemoryMerkleStorage;
pub use in_memory_writer::InMemoryWriter;
#[cfg(feature = "kms")]
pub use kms_signer::{KmsConfig, KmsSigner};
pub use lambda_orderer::{LambdaOrderer, LambdaOrdererConfig};
pub use persistent_merkle_storage::{InMemoryBackend, PersistentMerkleStorage};
pub use q_invariant_verifier::QInvariantVerifier;
pub use receipt_builder::{LocalSigner, Signer, StandardReceiptBuilder};
pub use receipt_storage::InMemoryReceiptStorage;
#[cfg(feature = "storage")]
pub use receipt_storage::{CloudStorageConfig, CloudStorageReceiptStorage};
pub use sigma_type_checker::{SigmaTypeChecker, TypeCheckError};
pub use workflow_kernel::InMemoryWorkflowKernel;
#[cfg(feature = "workspace-publisher")]
pub use workspace_publisher::{GoogleWorkspacePublisher, WorkspacePublisherConfig};
