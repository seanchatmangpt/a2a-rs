//! Cryptographic refusal engine implementation
//!
//! Generates inadmissible-before receipts with cryptographic proof.

use async_trait::async_trait;

use crate::{
    domain::{AuthErrorCode, RefusalReason, RefusalReceipt, TypeCheckErrorCode},
    port::RefusalEngine,
};

/// Cryptographic refusal engine
///
/// Generates refusal receipts with SHA-256 proof hashes for all inadmissible packets.
/// Ensures deterministic, non-repudiable refusal decisions.
///
/// # Example
/// ```no_run
/// use osiris_edge::adapter::CryptoRefusalEngine;
/// use osiris_edge::port::RefusalEngine;
/// use osiris_edge::domain::AuthErrorCode;
///
/// # async fn example() {
/// let engine = CryptoRefusalEngine::new("gateway-1");
///
/// // Refuse a packet due to WIP capacity
/// let receipt = engine.refuse_wip_exceeded("pkt-123", 10, 10).await;
/// println!("Refused: {}", receipt.summary());
/// assert!(receipt.verify_proof());
///
/// // Refuse a packet due to auth failure
/// let receipt = engine.refuse_auth_failed(
///     "pkt-456",
///     AuthErrorCode::InvalidSignature,
///     "JWT signature verification failed"
/// ).await;
/// assert!(receipt.verify_proof());
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CryptoRefusalEngine {
    /// Gateway issuer identity
    issuer: String,
}

impl CryptoRefusalEngine {
    /// Create a new cryptographic refusal engine
    ///
    /// # Arguments
    /// * `issuer` - Identity of the gateway issuing refusal receipts
    #[must_use]
    pub fn new(issuer: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
        }
    }

    /// Create a refusal receipt with optional retry-after hint
    fn create_receipt_with_retry(
        &self,
        packet_id: &str,
        reason: RefusalReason,
        retry_after: Option<String>,
    ) -> RefusalReceipt {
        match retry_after {
            Some(retry) => RefusalReceipt::with_retry_after(packet_id, reason, &self.issuer, retry),
            None => RefusalReceipt::new(packet_id, reason, &self.issuer),
        }
    }
}

#[async_trait]
impl RefusalEngine for CryptoRefusalEngine {
    async fn refuse_wip_exceeded(
        &self,
        packet_id: &str,
        current: usize,
        limit: usize,
    ) -> RefusalReceipt {
        let reason = RefusalReason::wip_cap_exceeded(current, limit);

        // Suggest retry after 30 seconds for WIP capacity issues
        self.create_receipt_with_retry(packet_id, reason, Some("PT30S".to_string()))
    }

    async fn refuse_auth_failed(
        &self,
        packet_id: &str,
        error_code: AuthErrorCode,
        message: &str,
    ) -> RefusalReceipt {
        let reason = RefusalReason::auth_failed(error_code, message);

        // No retry-after for auth failures (client must fix credentials)
        self.create_receipt_with_retry(packet_id, reason, None)
    }

    async fn refuse_guard_failed(
        &self,
        packet_id: &str,
        guard_id: &str,
        condition: &str,
        message: &str,
    ) -> RefusalReceipt {
        let reason = RefusalReason::guard_failed(guard_id, condition, message);

        // Suggest retry after 60 seconds for guard failures (may need precondition)
        self.create_receipt_with_retry(packet_id, reason, Some("PT60S".to_string()))
    }

    async fn refuse_type_check_failed(
        &self,
        packet_id: &str,
        error_code: TypeCheckErrorCode,
        attempted_type: &str,
        message: &str,
    ) -> RefusalReceipt {
        let reason = RefusalReason::type_check_failed(error_code, attempted_type, message);

        // No retry-after for type check failures (client must fix packet structure)
        self.create_receipt_with_retry(packet_id, reason, None)
    }

    async fn refuse_type_check_failed_with_errors(
        &self,
        packet_id: &str,
        error_code: TypeCheckErrorCode,
        attempted_type: &str,
        message: &str,
        errors: Vec<String>,
    ) -> RefusalReceipt {
        let reason = RefusalReason::type_check_failed_with_errors(
            error_code,
            attempted_type,
            message,
            errors,
        );

        // No retry-after for type check failures (client must fix packet structure)
        self.create_receipt_with_retry(packet_id, reason, None)
    }

    fn issuer(&self) -> &str {
        &self.issuer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_refuse_wip_exceeded() {
        let engine = CryptoRefusalEngine::new("test-gateway");

        let receipt = engine.refuse_wip_exceeded("pkt-123", 5, 5).await;

        assert_eq!(receipt.packet_id, "pkt-123");
        assert_eq!(receipt.issuer, "test-gateway");
        assert!(receipt.verify_proof());
        assert!(receipt.summary().contains("WIP capacity exceeded"));
        assert_eq!(receipt.retry_after, Some("PT30S".to_string()));
    }

    #[tokio::test]
    async fn test_refuse_auth_failed() {
        let engine = CryptoRefusalEngine::new("test-gateway");

        let receipt = engine
            .refuse_auth_failed(
                "pkt-456",
                AuthErrorCode::InvalidSignature,
                "JWT signature verification failed",
            )
            .await;

        assert_eq!(receipt.packet_id, "pkt-456");
        assert!(receipt.verify_proof());
        assert!(receipt.summary().contains("Authentication failed"));
        assert_eq!(receipt.retry_after, None);
    }

    #[tokio::test]
    async fn test_refuse_guard_failed() {
        let engine = CryptoRefusalEngine::new("test-gateway");

        let receipt = engine
            .refuse_guard_failed(
                "pkt-789",
                "precondition-auth",
                "RequiresPrior(AuthToken)",
                "Must authenticate before submitting data",
            )
            .await;

        assert_eq!(receipt.packet_id, "pkt-789");
        assert!(receipt.verify_proof());
        assert!(
            receipt
                .summary()
                .contains("Guard 'precondition-auth' failed")
        );
        assert_eq!(receipt.retry_after, Some("PT60S".to_string()));
    }

    #[tokio::test]
    async fn test_refuse_type_check_failed() {
        let engine = CryptoRefusalEngine::new("test-gateway");

        let receipt = engine
            .refuse_type_check_failed(
                "pkt-999",
                TypeCheckErrorCode::TypeNotInSigma,
                "UnknownPacket",
                "Packet type not in closed type system Σ",
            )
            .await;

        assert_eq!(receipt.packet_id, "pkt-999");
        assert!(receipt.verify_proof());
        assert!(receipt.summary().contains("Type check failed"));
        assert_eq!(receipt.retry_after, None);
    }

    #[tokio::test]
    async fn test_refuse_type_check_failed_with_errors() {
        let engine = CryptoRefusalEngine::new("test-gateway");

        let errors = vec![
            "Missing required field: 'from'".to_string(),
            "Field 'to' must be non-empty".to_string(),
        ];

        let receipt = engine
            .refuse_type_check_failed_with_errors(
                "pkt-111",
                TypeCheckErrorCode::SchemaViolation,
                "EmailPacket",
                "Schema validation failed",
                errors.clone(),
            )
            .await;

        assert_eq!(receipt.packet_id, "pkt-111");
        assert!(receipt.verify_proof());

        if let RefusalReason::TypeCheckFailed {
            errors: ref_errors, ..
        } = &receipt.reason
        {
            assert_eq!(ref_errors.len(), 2);
            assert_eq!(ref_errors, &errors);
        } else {
            panic!("Expected TypeCheckFailed variant");
        }
    }

    #[tokio::test]
    async fn test_issuer() {
        let engine = CryptoRefusalEngine::new("my-gateway");
        assert_eq!(engine.issuer(), "my-gateway");
    }

    #[tokio::test]
    async fn test_refuse_from_wip_error() {
        use crate::domain::WipError;

        let engine = CryptoRefusalEngine::new("test-gateway");

        // Test WipLimitReached
        let wip_error = WipError::WipLimitReached {
            current: 10,
            limit: 10,
        };
        let receipt = engine.refuse_from_wip_error("pkt-wip", &wip_error).await;
        assert!(receipt.verify_proof());
        assert!(receipt.summary().contains("WIP capacity exceeded"));

        // Test GateClosed
        let gate_closed = WipError::GateClosed;
        let receipt = engine
            .refuse_from_wip_error("pkt-closed", &gate_closed)
            .await;
        assert!(receipt.verify_proof());
        assert!(receipt.summary().contains("Authentication failed"));
    }

    #[tokio::test]
    async fn test_receipt_uniqueness() {
        let engine = CryptoRefusalEngine::new("test-gateway");

        // Generate two receipts for the same packet with same reason
        let receipt1 = engine.refuse_wip_exceeded("pkt-same", 5, 5).await;
        let receipt2 = engine.refuse_wip_exceeded("pkt-same", 5, 5).await;

        // Receipts should have different IDs and timestamps
        assert_ne!(receipt1.receipt_id, receipt2.receipt_id);
        assert_ne!(receipt1.timestamp, receipt2.timestamp);

        // But both should verify
        assert!(receipt1.verify_proof());
        assert!(receipt2.verify_proof());
    }

    #[tokio::test]
    async fn test_clone() {
        let engine1 = CryptoRefusalEngine::new("gateway-1");
        let engine2 = engine1.clone();

        assert_eq!(engine1.issuer(), engine2.issuer());

        let receipt1 = engine1.refuse_wip_exceeded("pkt-1", 5, 5).await;
        let receipt2 = engine2.refuse_wip_exceeded("pkt-2", 5, 5).await;

        assert_eq!(receipt1.issuer, receipt2.issuer);
    }
}
