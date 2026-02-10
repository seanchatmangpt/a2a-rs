//! Persistent receipt storage using SQLx.
//!
//! This module provides database-backed storage for receipt chains, enabling
//! persistent audit trails across restarts and replay verification from storage.

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use async_trait::async_trait;
#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use chrono::{DateTime, Utc};
#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use serde::{Deserialize, Serialize};
#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use sqlx::{Row, SqlitePool};
#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use thiserror::Error;

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use crate::construct::receipts::{Receipt, ReceiptChain, ReceiptError};

/// Errors that can occur during receipt storage operations
#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ReceiptStoreError {
    /// Database connection error
    #[error("Database error: {0}")]
    Database(String),

    /// Receipt not found
    #[error("Receipt not found at sequence {0}")]
    NotFound(u64),

    /// Chain verification failed
    #[error("Chain verification failed: {0}")]
    VerificationFailed(String),

    /// Sequence number mismatch
    #[error("Sequence mismatch: expected {expected}, got {actual}")]
    SequenceMismatch { expected: u64, actual: u64 },

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid replay point
    #[error("Invalid replay point: {0}")]
    InvalidReplayPoint(String),
}

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
impl From<sqlx::Error> for ReceiptStoreError {
    fn from(err: sqlx::Error) -> Self {
        ReceiptStoreError::Database(err.to_string())
    }
}

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
impl From<serde_json::Error> for ReceiptStoreError {
    fn from(err: serde_json::Error) -> Self {
        ReceiptStoreError::Serialization(err.to_string())
    }
}

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
impl From<ReceiptError> for ReceiptStoreError {
    fn from(err: ReceiptError) -> Self {
        ReceiptStoreError::VerificationFailed(err.to_string())
    }
}

/// SQLx-based persistent receipt storage.
///
/// Stores receipts in a database table with the following schema:
/// - sequence: INTEGER PRIMARY KEY
/// - timestamp: TEXT (ISO 8601 format)
/// - observation_hash: TEXT
/// - action_hash: TEXT
/// - delta_hash: TEXT
/// - receipt_hash: TEXT
/// - previous_hash: TEXT (nullable)
/// - signature: TEXT (nullable)
/// - public_key: TEXT (nullable)
/// - metadata: TEXT (nullable, JSON)
#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
pub struct ReceiptStore {
    pool: SqlitePool,
}

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
impl ReceiptStore {
    /// Creates a new receipt store with the given database pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Creates a new receipt store from a database URL.
    pub async fn from_url(database_url: &str) -> Result<Self, ReceiptStoreError> {
        let pool = SqlitePool::connect(database_url).await?;
        let store = Self::new(pool);
        store.run_migrations().await?;
        Ok(store)
    }

    /// Runs database migrations to create the receipts table.
    async fn run_migrations(&self) -> Result<(), ReceiptStoreError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS receipts (
                sequence INTEGER PRIMARY KEY,
                timestamp TEXT NOT NULL,
                observation_hash TEXT NOT NULL,
                action_hash TEXT NOT NULL,
                delta_hash TEXT NOT NULL,
                receipt_hash TEXT NOT NULL,
                previous_hash TEXT,
                signature TEXT,
                public_key TEXT,
                metadata TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create index on receipt_hash for faster lookups
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_receipt_hash ON receipts(receipt_hash)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Appends a receipt to the store.
    ///
    /// The receipt's sequence number must be the next in the chain.
    /// This method verifies the sequence and previous_hash before storing.
    pub async fn append(&self, receipt: &Receipt) -> Result<(), ReceiptStoreError> {
        // Verify sequence number
        let expected_sequence = self.get_chain_length().await?;
        if receipt.sequence != expected_sequence {
            return Err(ReceiptStoreError::SequenceMismatch {
                expected: expected_sequence,
                actual: receipt.sequence,
            });
        }

        // Verify previous hash if not genesis
        if receipt.sequence > 0 {
            let prev_receipt = self.get_receipt(receipt.sequence - 1).await?;
            match &receipt.previous_hash {
                Some(prev_hash) if prev_hash == &prev_receipt.receipt_hash => {}
                Some(prev_hash) => {
                    return Err(ReceiptStoreError::VerificationFailed(format!(
                        "Previous hash mismatch: expected {}, got {}",
                        prev_receipt.receipt_hash, prev_hash
                    )));
                }
                None => {
                    return Err(ReceiptStoreError::VerificationFailed(
                        "Missing previous hash for non-genesis receipt".to_string(),
                    ));
                }
            }
        }

        // Serialize metadata if present
        let metadata_json = receipt
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        // Insert receipt
        sqlx::query(
            r#"
            INSERT INTO receipts (
                sequence, timestamp, observation_hash, action_hash, delta_hash,
                receipt_hash, previous_hash, signature, public_key, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(receipt.sequence as i64)
        .bind(receipt.timestamp.to_rfc3339())
        .bind(&receipt.observation_hash)
        .bind(&receipt.action_hash)
        .bind(&receipt.delta_hash)
        .bind(&receipt.receipt_hash)
        .bind(&receipt.previous_hash)
        .bind({
            #[cfg(feature = "receipts-signing")]
            {
                &receipt.signature
            }
            #[cfg(not(feature = "receipts-signing"))]
            {
                None::<String>
            }
        })
        .bind({
            #[cfg(feature = "receipts-signing")]
            {
                &receipt.public_key
            }
            #[cfg(not(feature = "receipts-signing"))]
            {
                None::<String>
            }
        })
        .bind(metadata_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets a single receipt by sequence number.
    pub async fn get_receipt(&self, sequence: u64) -> Result<Receipt, ReceiptStoreError> {
        let row = sqlx::query(
            r#"
            SELECT sequence, timestamp, observation_hash, action_hash, delta_hash,
                   receipt_hash, previous_hash, signature, public_key, metadata
            FROM receipts
            WHERE sequence = ?
            "#,
        )
        .bind(sequence as i64)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ReceiptStoreError::NotFound(sequence))?;

        self.row_to_receipt(&row)
    }

    /// Gets the entire receipt chain from storage.
    pub async fn get_chain(&self) -> Result<ReceiptChain, ReceiptStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, timestamp, observation_hash, action_hash, delta_hash,
                   receipt_hash, previous_hash, signature, public_key, metadata
            FROM receipts
            ORDER BY sequence ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let receipts: Result<Vec<Receipt>, _> =
            rows.iter().map(|row| self.row_to_receipt(row)).collect();

        Ok(ReceiptChain {
            receipts: receipts?,
            metadata: None,
        })
    }

    /// Gets a partial chain from a starting sequence number.
    pub async fn get_chain_from(
        &self,
        start_sequence: u64,
    ) -> Result<ReceiptChain, ReceiptStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, timestamp, observation_hash, action_hash, delta_hash,
                   receipt_hash, previous_hash, signature, public_key, metadata
            FROM receipts
            WHERE sequence >= ?
            ORDER BY sequence ASC
            "#,
        )
        .bind(start_sequence as i64)
        .fetch_all(&self.pool)
        .await?;

        let receipts: Result<Vec<Receipt>, _> =
            rows.iter().map(|row| self.row_to_receipt(row)).collect();

        Ok(ReceiptChain {
            receipts: receipts?,
            metadata: None,
        })
    }

    /// Verifies the integrity of the stored chain.
    ///
    /// This loads the entire chain and runs verification.
    pub async fn verify_chain(&self) -> Result<(), ReceiptStoreError> {
        let chain = self.get_chain().await?;
        chain.verify_integrity()?;
        Ok(())
    }

    /// Replays operations from a specific sequence number.
    ///
    /// Returns a chain containing all receipts from the replay point onward.
    /// This is useful for rebuilding state after a specific point in history.
    pub async fn replay_from(
        &self,
        start_sequence: u64,
    ) -> Result<ReceiptChain, ReceiptStoreError> {
        // Verify the start sequence exists
        let chain_length = self.get_chain_length().await?;
        if start_sequence >= chain_length {
            return Err(ReceiptStoreError::InvalidReplayPoint(format!(
                "Sequence {} is beyond chain length {}",
                start_sequence, chain_length
            )));
        }

        // Get the partial chain
        let replay_chain = self.get_chain_from(start_sequence).await?;

        // Verify integrity of the replay chain
        replay_chain.verify_integrity()?;

        Ok(replay_chain)
    }

    /// Gets the current length of the receipt chain.
    pub async fn get_chain_length(&self) -> Result<u64, ReceiptStoreError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM receipts")
            .fetch_one(&self.pool)
            .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count as u64)
    }

    /// Gets the latest receipt in the chain.
    pub async fn get_latest(&self) -> Result<Option<Receipt>, ReceiptStoreError> {
        let length = self.get_chain_length().await?;
        if length == 0 {
            return Ok(None);
        }
        Ok(Some(self.get_receipt(length - 1).await?))
    }

    /// Clears all receipts from storage (useful for testing).
    #[cfg(test)]
    pub async fn clear(&self) -> Result<(), ReceiptStoreError> {
        sqlx::query("DELETE FROM receipts")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Converts a database row to a Receipt.
    fn row_to_receipt(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Receipt, ReceiptStoreError> {
        let sequence: i64 = row.try_get("sequence")?;
        let timestamp_str: String = row.try_get("timestamp")?;
        let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(|e| ReceiptStoreError::Serialization(e.to_string()))?
            .with_timezone(&Utc);

        let metadata_str: Option<String> = row.try_get("metadata")?;
        let metadata = metadata_str.map(|s| serde_json::from_str(&s)).transpose()?;

        Ok(Receipt {
            sequence: sequence as u64,
            timestamp,
            observation_hash: row.try_get("observation_hash")?,
            action_hash: row.try_get("action_hash")?,
            delta_hash: row.try_get("delta_hash")?,
            receipt_hash: row.try_get("receipt_hash")?,
            previous_hash: row.try_get("previous_hash")?,
            #[cfg(feature = "receipts-signing")]
            signature: row.try_get("signature")?,
            #[cfg(feature = "receipts-signing")]
            public_key: row.try_get("public_key")?,
            metadata,
        })
    }
}

#[cfg(all(test, feature = "sqlx-storage", feature = "receipts"))]
mod tests {
    use super::*;

    async fn create_test_store() -> ReceiptStore {
        ReceiptStore::from_url("sqlite::memory:")
            .await
            .expect("Failed to create test store")
    }

    #[tokio::test]
    async fn test_store_creation() {
        let store = create_test_store().await;
        let length = store.get_chain_length().await.unwrap();
        assert_eq!(length, 0);
    }

    #[tokio::test]
    async fn test_append_receipt() {
        let store = create_test_store().await;

        let receipt = Receipt::new(b"observation", b"action", b"delta");
        store.append(&receipt).await.unwrap();

        let length = store.get_chain_length().await.unwrap();
        assert_eq!(length, 1);
    }

    #[tokio::test]
    async fn test_append_chain() {
        let store = create_test_store().await;

        // Create a chain in memory
        let mut chain = ReceiptChain::new();
        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");
        chain.add_transition(b"obs3", b"act3", b"delta3");

        // Append each receipt
        for receipt in &chain.receipts {
            store.append(receipt).await.unwrap();
        }

        let length = store.get_chain_length().await.unwrap();
        assert_eq!(length, 3);
    }

    #[tokio::test]
    async fn test_get_receipt() {
        let store = create_test_store().await;

        let receipt = Receipt::new(b"observation", b"action", b"delta");
        store.append(&receipt).await.unwrap();

        let retrieved = store.get_receipt(0).await.unwrap();
        assert_eq!(retrieved.observation_hash, receipt.observation_hash);
        assert_eq!(retrieved.action_hash, receipt.action_hash);
        assert_eq!(retrieved.delta_hash, receipt.delta_hash);
    }

    #[tokio::test]
    async fn test_get_chain() {
        let store = create_test_store().await;

        // Create and store a chain
        let mut chain = ReceiptChain::new();
        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        for receipt in &chain.receipts {
            store.append(receipt).await.unwrap();
        }

        // Retrieve the chain
        let retrieved_chain = store.get_chain().await.unwrap();
        assert_eq!(retrieved_chain.len(), 2);
        assert!(retrieved_chain.verify_integrity().is_ok());
    }

    #[tokio::test]
    async fn test_verify_chain() {
        let store = create_test_store().await;

        // Create and store a valid chain
        let mut chain = ReceiptChain::new();
        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        for receipt in &chain.receipts {
            store.append(receipt).await.unwrap();
        }

        // Verification should pass
        assert!(store.verify_chain().await.is_ok());
    }

    #[tokio::test]
    async fn test_replay_from() {
        let store = create_test_store().await;

        // Create a chain
        let mut chain = ReceiptChain::new();
        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");
        chain.add_transition(b"obs3", b"act3", b"delta3");

        for receipt in &chain.receipts {
            store.append(receipt).await.unwrap();
        }

        // Replay from sequence 1
        let replay_chain = store.replay_from(1).await.unwrap();
        assert_eq!(replay_chain.len(), 2);
        assert_eq!(replay_chain.get(0).unwrap().sequence, 1);
        assert!(replay_chain.verify_integrity().is_ok());
    }

    #[tokio::test]
    async fn test_get_latest() {
        let store = create_test_store().await;

        // No receipts yet
        assert!(store.get_latest().await.unwrap().is_none());

        // Add receipts
        let mut chain = ReceiptChain::new();
        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        for receipt in &chain.receipts {
            store.append(receipt).await.unwrap();
        }

        // Get latest
        let latest = store.get_latest().await.unwrap().unwrap();
        assert_eq!(latest.sequence, 1);
    }

    #[tokio::test]
    async fn test_sequence_validation() {
        let store = create_test_store().await;

        // Try to append receipt with wrong sequence
        let mut receipt = Receipt::new(b"observation", b"action", b"delta");
        receipt.sequence = 5; // Wrong sequence

        let result = store.append(&receipt).await;
        assert!(matches!(
            result,
            Err(ReceiptStoreError::SequenceMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn test_previous_hash_validation() {
        let store = create_test_store().await;

        // Add genesis receipt
        let receipt1 = Receipt::new(b"obs1", b"act1", b"delta1");
        store.append(&receipt1).await.unwrap();

        // Try to append receipt with wrong previous hash
        let mut receipt2 = Receipt::new(b"obs2", b"act2", b"delta2");
        receipt2.sequence = 1;
        receipt2.previous_hash = Some("wrong_hash".to_string());

        let result = store.append(&receipt2).await;
        assert!(matches!(
            result,
            Err(ReceiptStoreError::VerificationFailed(_))
        ));
    }

    #[cfg(feature = "receipts-signing")]
    #[tokio::test]
    async fn test_signed_receipts() {
        use ed25519_dalek::SigningKey;
        use rand::{RngCore, rngs::OsRng};

        let store = create_test_store().await;

        let mut rng = OsRng;
        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        // Create signed chain
        let mut chain = ReceiptChain::new();
        chain.add_signed_transition(b"obs1", b"act1", b"delta1", &signing_key);
        chain.add_signed_transition(b"obs2", b"act2", b"delta2", &signing_key);

        // Store receipts
        for receipt in &chain.receipts {
            store.append(receipt).await.unwrap();
        }

        // Retrieve and verify
        let retrieved_chain = store.get_chain().await.unwrap();
        assert!(retrieved_chain.verify_integrity().is_ok());

        // Check signatures are present
        for receipt in retrieved_chain.iter() {
            assert!(receipt.signature.is_some());
            assert!(receipt.public_key.is_some());
        }
    }
}
