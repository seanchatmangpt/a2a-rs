//! Adapter implementations for the Osiris compiler.
//!
//! Adapters implement the port traits with concrete technologies
//! (databases, external services, etc.).

pub mod construct8_writer;
pub mod h_guard_evaluator;
pub mod in_memory_writer;
pub mod kms_signer;
pub mod lambda_orderer;
pub mod q_invariant_verifier;
pub mod receipt_builder;
pub mod receipt_storage;
pub mod sigma_type_checker;
pub mod workflow_kernel;

pub use construct8_writer::Construct8Writer;
pub use h_guard_evaluator::{EvaluationContext, GuardEvaluationError, HGuardEvaluatorAdapter};
pub use in_memory_writer::InMemoryWriter;
#[cfg(feature = "kms")]
pub use kms_signer::{KmsConfig, KmsSigner};
pub use lambda_orderer::{LambdaOrderer, LambdaOrdererConfig};
pub use q_invariant_verifier::QInvariantVerifier;
pub use receipt_builder::{LocalSigner, Signer, StandardReceiptBuilder};
pub use receipt_storage::InMemoryReceiptStorage;
#[cfg(feature = "storage")]
pub use receipt_storage::{CloudStorageConfig, CloudStorageReceiptStorage};
pub use sigma_type_checker::{SigmaTypeChecker, TypeCheckError};
pub use workflow_kernel::InMemoryWorkflowKernel;
