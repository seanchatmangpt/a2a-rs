//! Storage adapters for CONSTRUCT components.
//!
//! This module provides persistent storage implementations for receipts,
//! artifacts, ontology state, and other CONSTRUCT data structures.

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
pub mod receipt_store;

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
pub use receipt_store::{ReceiptStore, ReceiptStoreError};

#[cfg(feature = "sqlx-storage")]
pub mod ontology_store;

#[cfg(feature = "sqlx-storage")]
pub use ontology_store::{AsyncOntologyStorage, SqlxOntologyStore};
