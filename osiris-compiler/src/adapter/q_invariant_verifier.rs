//! Q invariant verifier adapter implementation.
//!
//! This adapter implements the InvariantVerifier port with support for:
//! - Predicate evaluation over state snapshots
//! - preserve(Q) verification across state transitions
//! - Commit blocking on invariant violations
//! - Refusal receipt emission

use crate::domain::{
    Commit, CommitVerificationResult, ComparisonOperator, InvariantCheckResult, InvariantPredicate,
    InvariantSeverity, PreservationResult, QInvariant, RefusalReason, RefusalReceipt,
    StateSnapshot,
};
use crate::port::{InvariantVerificationError, InvariantVerifier};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Q invariant verifier implementation.
///
/// This adapter maintains a registry of invariants and provides methods
/// to verify them against state snapshots and commits.
#[derive(Debug, Clone)]
pub struct QInvariantVerifier {
    /// Registered invariants
    invariants: Arc<RwLock<HashMap<String, QInvariant>>>,
}

impl QInvariantVerifier {
    /// Creates a new Q invariant verifier.
    pub fn new() -> Self {
        Self {
            invariants: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Evaluates a predicate against a state snapshot.
    fn evaluate_predicate(
        &self,
        predicate: &InvariantPredicate,
        state: &StateSnapshot,
    ) -> Result<bool, InvariantVerificationError> {
        match predicate {
            InvariantPredicate::StateEquals { field, expected } => {
                let actual = state
                    .state
                    .get(field)
                    .ok_or_else(|| InvariantVerificationError::MissingStateField(field.clone()))?;
                Ok(actual == expected)
            }

            InvariantPredicate::StateComparison {
                field,
                operator,
                value,
            } => {
                let actual = state
                    .state
                    .get(field)
                    .ok_or_else(|| InvariantVerificationError::MissingStateField(field.clone()))?;
                self.compare_values(actual, operator, value)
            }

            InvariantPredicate::And { predicates } => {
                for pred in predicates {
                    if !self.evaluate_predicate(pred, state)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }

            InvariantPredicate::Or { predicates } => {
                for pred in predicates {
                    if self.evaluate_predicate(pred, state)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            InvariantPredicate::Not { predicate } => {
                Ok(!self.evaluate_predicate(predicate, state)?)
            }

            InvariantPredicate::Relational {
                left_field,
                operator,
                right_field,
            } => {
                let left = state.state.get(left_field).ok_or_else(|| {
                    InvariantVerificationError::MissingStateField(left_field.clone())
                })?;
                let right = state.state.get(right_field).ok_or_else(|| {
                    InvariantVerificationError::MissingStateField(right_field.clone())
                })?;
                self.compare_values(left, operator, right)
            }

            InvariantPredicate::TypeInvariant { schema } => {
                // For now, we do basic JSON schema validation
                // In a full implementation, use a JSON schema validator library
                self.validate_type_schema(&state.state, schema)
            }

            InvariantPredicate::Custom {
                expression,
                parameters: _,
            } => {
                // Custom expressions would require a runtime evaluator
                // For now, return an error indicating this needs implementation
                Err(InvariantVerificationError::CustomExpressionError(format!(
                    "Custom expression not yet implemented: {}",
                    expression
                )))
            }
        }
    }

    /// Compares two JSON values using a comparison operator.
    fn compare_values(
        &self,
        left: &serde_json::Value,
        operator: &ComparisonOperator,
        right: &serde_json::Value,
    ) -> Result<bool, InvariantVerificationError> {
        use serde_json::Value;

        match operator {
            ComparisonOperator::Eq => Ok(left == right),
            ComparisonOperator::Ne => Ok(left != right),

            ComparisonOperator::Lt => match (left, right) {
                (Value::Number(l), Value::Number(r)) => {
                    Ok(l.as_f64().unwrap_or(0.0) < r.as_f64().unwrap_or(0.0))
                }
                _ => Err(InvariantVerificationError::TypeMismatch {
                    expected: "number".to_string(),
                    actual: format!("{:?}", left),
                }),
            },

            ComparisonOperator::Le => match (left, right) {
                (Value::Number(l), Value::Number(r)) => {
                    Ok(l.as_f64().unwrap_or(0.0) <= r.as_f64().unwrap_or(0.0))
                }
                _ => Err(InvariantVerificationError::TypeMismatch {
                    expected: "number".to_string(),
                    actual: format!("{:?}", left),
                }),
            },

            ComparisonOperator::Gt => match (left, right) {
                (Value::Number(l), Value::Number(r)) => {
                    Ok(l.as_f64().unwrap_or(0.0) > r.as_f64().unwrap_or(0.0))
                }
                _ => Err(InvariantVerificationError::TypeMismatch {
                    expected: "number".to_string(),
                    actual: format!("{:?}", left),
                }),
            },

            ComparisonOperator::Ge => match (left, right) {
                (Value::Number(l), Value::Number(r)) => {
                    Ok(l.as_f64().unwrap_or(0.0) >= r.as_f64().unwrap_or(0.0))
                }
                _ => Err(InvariantVerificationError::TypeMismatch {
                    expected: "number".to_string(),
                    actual: format!("{:?}", left),
                }),
            },

            ComparisonOperator::Contains => match left {
                Value::Array(arr) => Ok(arr.contains(right)),
                Value::String(s) => match right {
                    Value::String(substr) => Ok(s.contains(substr.as_str())),
                    _ => Ok(false),
                },
                _ => Err(InvariantVerificationError::TypeMismatch {
                    expected: "array or string".to_string(),
                    actual: format!("{:?}", left),
                }),
            },

            ComparisonOperator::Matches => {
                // Regex matching would require regex crate
                Err(InvariantVerificationError::CustomExpressionError(
                    "Regex matching not yet implemented".to_string(),
                ))
            }
        }
    }

    /// Validates state against a type schema.
    fn validate_type_schema(
        &self,
        state: &HashMap<String, serde_json::Value>,
        _schema: &serde_json::Value,
    ) -> Result<bool, InvariantVerificationError> {
        // Basic validation - in a full implementation, use jsonschema crate
        // For now, just check that state is not empty
        Ok(!state.is_empty())
    }

    /// Generates a unique receipt ID.
    fn generate_receipt_id() -> String {
        #[cfg(feature = "timestamps")]
        {
            format!("rcpt-{}", chrono::Utc::now().timestamp_millis())
        }
        #[cfg(not(feature = "timestamps"))]
        {
            format!(
                "rcpt-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            )
        }
    }
}

impl Default for QInvariantVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InvariantVerifier for QInvariantVerifier {
    async fn register_invariant(
        &mut self,
        invariant: QInvariant,
    ) -> Result<(), InvariantVerificationError> {
        let mut invariants = self.invariants.write().await;
        invariants.insert(invariant.id.clone(), invariant);
        Ok(())
    }

    async fn unregister_invariant(
        &mut self,
        invariant_id: &str,
    ) -> Result<(), InvariantVerificationError> {
        let mut invariants = self.invariants.write().await;
        invariants.remove(invariant_id).ok_or_else(|| {
            InvariantVerificationError::InvariantNotFound(invariant_id.to_string())
        })?;
        Ok(())
    }

    async fn get_invariant(
        &self,
        invariant_id: &str,
    ) -> Result<QInvariant, InvariantVerificationError> {
        let invariants = self.invariants.read().await;
        invariants
            .get(invariant_id)
            .cloned()
            .ok_or_else(|| InvariantVerificationError::InvariantNotFound(invariant_id.to_string()))
    }

    async fn list_invariants(&self) -> Vec<QInvariant> {
        let invariants = self.invariants.read().await;
        invariants.values().cloned().collect()
    }

    async fn check_invariant(
        &self,
        invariant_id: &str,
        state: &StateSnapshot,
    ) -> Result<InvariantCheckResult, InvariantVerificationError> {
        let invariant = self.get_invariant(invariant_id).await?;

        if !invariant.enabled {
            // Disabled invariants are considered satisfied
            return Ok(InvariantCheckResult::Satisfied {
                invariant_id: invariant_id.to_string(),
                snapshot_id: state.snapshot_id.clone(),
            });
        }

        match self.evaluate_predicate(&invariant.predicate, state) {
            Ok(true) => Ok(InvariantCheckResult::Satisfied {
                invariant_id: invariant_id.to_string(),
                snapshot_id: state.snapshot_id.clone(),
            }),
            Ok(false) => Ok(InvariantCheckResult::Violated {
                invariant_id: invariant_id.to_string(),
                snapshot_id: state.snapshot_id.clone(),
                reason: format!("Invariant '{}' violated", invariant.name),
                context: HashMap::new(),
            }),
            Err(e) => Ok(InvariantCheckResult::VerificationFailed {
                invariant_id: invariant_id.to_string(),
                snapshot_id: state.snapshot_id.clone(),
                error: e.to_string(),
            }),
        }
    }

    async fn verify_preservation(
        &self,
        invariant_id: &str,
        pre_state: &StateSnapshot,
        post_state: &StateSnapshot,
    ) -> Result<PreservationResult, InvariantVerificationError> {
        let pre_result = self.check_invariant(invariant_id, pre_state).await?;
        let post_result = self.check_invariant(invariant_id, post_state).await?;

        let preserved = matches!(
            (&pre_result, &post_result),
            (
                InvariantCheckResult::Satisfied { .. },
                InvariantCheckResult::Satisfied { .. }
            )
        );

        Ok(PreservationResult {
            invariant_id: invariant_id.to_string(),
            pre_state: pre_result,
            post_state: post_result,
            preserved,
        })
    }

    async fn verify_commit(
        &self,
        commit: &Commit,
    ) -> Result<CommitVerificationResult, InvariantVerificationError> {
        let invariants = self.list_invariants().await;
        let mut results = Vec::new();

        for invariant in invariants {
            if !invariant.enabled {
                continue;
            }

            let preservation = self
                .verify_preservation(&invariant.id, &commit.pre_state, &commit.post_state)
                .await?;

            // Only block on Critical and Error severity violations
            if !preservation.preserved && preservation.should_block(invariant.severity) {
                results.push(preservation);
            } else {
                // Warning-level violations don't block but are still recorded
                results.push(preservation);
            }
        }

        Ok(CommitVerificationResult::new(
            commit.commit_id.clone(),
            results,
        ))
    }

    async fn block_commit(
        &self,
        commit: &Commit,
        verification_result: &CommitVerificationResult,
    ) -> Result<RefusalReceipt, InvariantVerificationError> {
        if verification_result.is_allowed() {
            return Err(InvariantVerificationError::InternalError(
                "Cannot block commit that passed verification".to_string(),
            ));
        }

        let reason = RefusalReason::InvariantViolation {
            invariant_ids: verification_result.blocking_violations.clone(),
            message: format!(
                "Commit blocked due to {} invariant violation(s)",
                verification_result.blocking_violations.len()
            ),
        };

        let mut context = HashMap::new();
        context.insert("commit_id".to_string(), serde_json::json!(commit.commit_id));
        context.insert(
            "pre_state_id".to_string(),
            serde_json::json!(commit.pre_state.snapshot_id),
        );
        context.insert(
            "post_state_id".to_string(),
            serde_json::json!(commit.post_state.snapshot_id),
        );

        let receipt = RefusalReceipt {
            receipt_id: Self::generate_receipt_id(),
            packet_id: commit.commit_id.clone(),
            reason,
            #[cfg(feature = "timestamps")]
            timestamp: chrono::Utc::now(),
            #[cfg(not(feature = "timestamps"))]
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string(),
            signature: None,
            context,
        };

        Ok(receipt)
    }

    async fn set_invariant_enabled(
        &mut self,
        invariant_id: &str,
        enabled: bool,
    ) -> Result<(), InvariantVerificationError> {
        let mut invariants = self.invariants.write().await;
        let invariant = invariants.get_mut(invariant_id).ok_or_else(|| {
            InvariantVerificationError::InvariantNotFound(invariant_id.to_string())
        })?;
        invariant.enabled = enabled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state(status: &str, count: i64) -> StateSnapshot {
        let mut state = HashMap::new();
        state.insert("status".to_string(), serde_json::json!(status));
        state.insert("count".to_string(), serde_json::json!(count));

        StateSnapshot {
            snapshot_id: "snap-1".to_string(),
            state,
            #[cfg(feature = "timestamps")]
            timestamp: chrono::Utc::now(),
            #[cfg(not(feature = "timestamps"))]
            timestamp: "2026-02-09T12:00:00Z".to_string(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_register_and_get_invariant() {
        let mut verifier = QInvariantVerifier::new();

        let invariant = QInvariant {
            id: "inv-1".to_string(),
            name: "Status must be active".to_string(),
            description: None,
            predicate: InvariantPredicate::StateEquals {
                field: "status".to_string(),
                expected: serde_json::json!("active"),
            },
            severity: InvariantSeverity::Error,
            enabled: true,
        };

        verifier
            .register_invariant(invariant.clone())
            .await
            .unwrap();

        let retrieved = verifier.get_invariant("inv-1").await.unwrap();
        assert_eq!(retrieved.id, "inv-1");
        assert_eq!(retrieved.name, "Status must be active");
    }

    #[tokio::test]
    async fn test_check_invariant_satisfied() {
        let mut verifier = QInvariantVerifier::new();

        let invariant = QInvariant {
            id: "inv-1".to_string(),
            name: "Status must be active".to_string(),
            description: None,
            predicate: InvariantPredicate::StateEquals {
                field: "status".to_string(),
                expected: serde_json::json!("active"),
            },
            severity: InvariantSeverity::Error,
            enabled: true,
        };

        verifier.register_invariant(invariant).await.unwrap();

        let state = create_test_state("active", 5);
        let result = verifier.check_invariant("inv-1", &state).await.unwrap();

        assert!(matches!(result, InvariantCheckResult::Satisfied { .. }));
    }

    #[tokio::test]
    async fn test_check_invariant_violated() {
        let mut verifier = QInvariantVerifier::new();

        let invariant = QInvariant {
            id: "inv-1".to_string(),
            name: "Status must be active".to_string(),
            description: None,
            predicate: InvariantPredicate::StateEquals {
                field: "status".to_string(),
                expected: serde_json::json!("active"),
            },
            severity: InvariantSeverity::Error,
            enabled: true,
        };

        verifier.register_invariant(invariant).await.unwrap();

        let state = create_test_state("inactive", 5);
        let result = verifier.check_invariant("inv-1", &state).await.unwrap();

        assert!(matches!(result, InvariantCheckResult::Violated { .. }));
    }

    #[tokio::test]
    async fn test_verify_preservation() {
        let mut verifier = QInvariantVerifier::new();

        let invariant = QInvariant {
            id: "inv-1".to_string(),
            name: "Count must be positive".to_string(),
            description: None,
            predicate: InvariantPredicate::StateComparison {
                field: "count".to_string(),
                operator: ComparisonOperator::Gt,
                value: serde_json::json!(0),
            },
            severity: InvariantSeverity::Error,
            enabled: true,
        };

        verifier.register_invariant(invariant).await.unwrap();

        let pre_state = create_test_state("active", 5);
        let post_state = create_test_state("active", 10);

        let result = verifier
            .verify_preservation("inv-1", &pre_state, &post_state)
            .await
            .unwrap();

        assert!(result.preserved);
    }

    #[tokio::test]
    async fn test_verify_commit_blocked() {
        let mut verifier = QInvariantVerifier::new();

        let invariant = QInvariant {
            id: "inv-1".to_string(),
            name: "Count must be positive".to_string(),
            description: None,
            predicate: InvariantPredicate::StateComparison {
                field: "count".to_string(),
                operator: ComparisonOperator::Gt,
                value: serde_json::json!(0),
            },
            severity: InvariantSeverity::Error,
            enabled: true,
        };

        verifier.register_invariant(invariant).await.unwrap();

        let commit = Commit {
            commit_id: "commit-1".to_string(),
            pre_state: create_test_state("active", 5),
            post_state: create_test_state("active", -1), // Violates invariant
            description: None,
            metadata: HashMap::new(),
        };

        let result = verifier.verify_commit(&commit).await.unwrap();

        assert!(result.is_blocked());
        assert!(!result.blocking_violations.is_empty());
    }

    #[tokio::test]
    async fn test_block_commit_with_receipt() {
        let mut verifier = QInvariantVerifier::new();

        let invariant = QInvariant {
            id: "inv-1".to_string(),
            name: "Count must be positive".to_string(),
            description: None,
            predicate: InvariantPredicate::StateComparison {
                field: "count".to_string(),
                operator: ComparisonOperator::Gt,
                value: serde_json::json!(0),
            },
            severity: InvariantSeverity::Critical,
            enabled: true,
        };

        verifier.register_invariant(invariant).await.unwrap();

        let commit = Commit {
            commit_id: "commit-1".to_string(),
            pre_state: create_test_state("active", 5),
            post_state: create_test_state("active", -1),
            description: None,
            metadata: HashMap::new(),
        };

        let verification_result = verifier.verify_commit(&commit).await.unwrap();
        let receipt = verifier
            .block_commit(&commit, &verification_result)
            .await
            .unwrap();

        assert_eq!(receipt.packet_id, "commit-1");
        assert!(matches!(
            receipt.reason,
            RefusalReason::InvariantViolation { .. }
        ));
    }
}
