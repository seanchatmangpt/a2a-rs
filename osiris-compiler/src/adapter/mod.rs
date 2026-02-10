//! Adapter implementations for the Osiris compiler.
//!
//! Adapters implement the port traits with concrete technologies
//! (databases, external services, etc.).

pub mod a2a_orchestrator;
pub mod audit_logger;
#[cfg(feature = "backup")]
pub mod backup;
pub mod bpmn_compiler;
pub mod circuit_breaker;
#[cfg(feature = "cloud-tasks")]
pub mod cloud_tasks_queue;
pub mod construct8_writer;
#[cfg(feature = "firestore")]
pub mod firestore_state_store;
#[cfg(feature = "gcs")]
pub mod gcs_receipt_storage;
#[cfg(feature = "grpc")]
pub mod grpc_transport;
pub mod h_guard_evaluator;
pub mod in_memory_merkle_storage;
pub mod in_memory_writer;
pub mod kms_signer;
pub mod lambda_orderer;
pub mod persistent_merkle_storage;
pub mod q_invariant_verifier;
pub mod receipt_builder;
pub mod receipt_storage;
#[cfg(feature = "secret-manager")]
pub mod secret_manager;
pub mod sigma_type_checker;
#[cfg(feature = "spanner")]
pub mod spanner_state_store;
pub mod workflow_kernel;
#[cfg(feature = "firestore")]
pub mod workflow_persistence;
#[cfg(feature = "workspace-publisher")]
pub mod workspace_publisher;

pub use a2a_orchestrator::RemoteA2AOrchestratorAdapter;
pub use audit_logger::{CloudLoggingAuditLogger, CloudLoggingConfig};
#[cfg(feature = "backup")]
pub use backup::GcsBackupManager;
pub use bpmn_compiler::BpmnCompiler;
pub use circuit_breaker::StandardCircuitBreaker;
#[cfg(feature = "cloud-tasks")]
pub use cloud_tasks_queue::CloudTasksQueue;
pub use construct8_writer::Construct8Writer;
#[cfg(feature = "firestore")]
pub use firestore_state_store::FirestoreStateStore;
#[cfg(feature = "gcs")]
pub use gcs_receipt_storage::{GcsConfig, GcsReceiptStorage};
#[cfg(feature = "grpc")]
pub use grpc_transport::GrpcTransport;
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
#[cfg(feature = "secret-manager")]
pub use secret_manager::{GoogleSecretManager, GoogleSecretManagerConfig};
pub use sigma_type_checker::{SigmaTypeChecker, TypeCheckError};
#[cfg(feature = "spanner")]
pub use spanner_state_store::{SpannerConfig, SpannerStateStore};
pub use workflow_kernel::InMemoryWorkflowKernel;
#[cfg(feature = "firestore")]
pub use workflow_persistence::{FirestoreConfig, FirestoreWorkflowStore};
#[cfg(feature = "workspace-publisher")]
pub use workspace_publisher::{GoogleWorkspacePublisher, WorkspacePublisherConfig};
