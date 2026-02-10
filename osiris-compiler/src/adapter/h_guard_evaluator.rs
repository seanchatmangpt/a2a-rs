//! HGuardEvaluator adapter.
//!
//! Implements the GuardEvaluator port trait for H-guards (inadmissible-before constraints).

use crate::domain::{GuardCondition, GuardEvaluationResult, HGuard, Packet};
use crate::port::GuardEvaluator;
use async_trait::async_trait;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error types for guard evaluation.
#[derive(Debug, thiserror::Error)]
pub enum GuardEvaluationError {
    #[error("Guard not found: {guard_id}")]
    GuardNotFound { guard_id: String },

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Evaluation error: {0}")]
    EvaluationError(String),

    #[error("Invalid condition: {0}")]
    InvalidCondition(String),
}

/// Context for evaluating guards.
///
/// Tracks state needed to evaluate temporal, state-based, and custom predicates.
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    /// Packets that have been processed (by ID)
    pub processed_packets: HashMap<String, Packet>,
    /// Current system state
    pub state: HashMap<String, serde_json::Value>,
    /// Custom predicate handlers
    pub custom_predicates: HashMap<String, bool>,
}

/// Adapter implementing the GuardEvaluator port.
///
/// This implementation:
/// - Maintains a registry of H-guards
/// - Evaluates temporal, state-based, and custom conditions
/// - Blocks packets that violate any guard
/// - Produces detailed violation reports
#[derive(Debug, Clone)]
pub struct HGuardEvaluatorAdapter {
    /// Registered H-guards indexed by ID
    guards: Arc<RwLock<HashMap<String, HGuard>>>,
    /// Evaluation context
    context: Arc<RwLock<EvaluationContext>>,
}

impl HGuardEvaluatorAdapter {
    /// Creates a new H-guard evaluator.
    pub fn new() -> Self {
        Self {
            guards: Arc::new(RwLock::new(HashMap::new())),
            context: Arc::new(RwLock::new(EvaluationContext::default())),
        }
    }

    /// Creates a new H-guard evaluator with custom context.
    pub fn with_context(context: EvaluationContext) -> Self {
        Self {
            guards: Arc::new(RwLock::new(HashMap::new())),
            context: Arc::new(RwLock::new(context)),
        }
    }

    /// Records that a packet has been successfully processed.
    ///
    /// This is used for RequiresPrior guard conditions.
    pub async fn record_packet(&self, packet: Packet) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut ctx = self.context.write().await;
        ctx.processed_packets.insert(packet.id.clone(), packet);
        Ok(())
    }

    /// Updates system state.
    ///
    /// This is used for StateRequirement guard conditions.
    pub async fn update_state(
        &self,
        key: String,
        value: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut ctx = self.context.write().await;
        ctx.state.insert(key, value);
        Ok(())
    }

    /// Registers a custom predicate result.
    ///
    /// This is used for Custom guard conditions.
    pub async fn set_predicate(
        &self,
        predicate: String,
        result: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut ctx = self.context.write().await;
        ctx.custom_predicates.insert(predicate, result);
        Ok(())
    }

    /// Evaluates a single guard condition.
    async fn evaluate_condition(
        &self,
        condition: &GuardCondition,
        _packet: &Packet,
    ) -> Result<bool, GuardEvaluationError> {
        let ctx = self.context.read().await;

        match condition {
            GuardCondition::RequiresPrior {
                packet_type,
                packet_id,
            } => {
                // Check if a prior packet of the required type has been processed
                if let Some(required_id) = packet_id {
                    // Specific packet required
                    Ok(ctx.processed_packets.contains_key(required_id))
                } else {
                    // Any packet of the required type
                    Ok(ctx
                        .processed_packets
                        .values()
                        .any(|p| &p.packet_type == packet_type))
                }
            }

            GuardCondition::TemporalDelay { not_before } => {
                #[cfg(feature = "timestamps")]
                {
                    if let Some(not_before_time) = not_before {
                        let now = chrono::Utc::now();
                        Ok(now >= *not_before_time)
                    } else {
                        Ok(true)
                    }
                }
                #[cfg(not(feature = "timestamps"))]
                {
                    // Without timestamp support, always consider temporal delays satisfied
                    let _ = not_before;
                    Ok(true)
                }
            }

            GuardCondition::StateRequirement {
                required_state,
                context: guard_context,
            } => {
                // Check if required state exists
                if !ctx.state.contains_key(required_state) {
                    return Ok(false);
                }

                // If there are context requirements, validate them
                if !guard_context.is_empty() {
                    let current_value = &ctx.state[required_state];
                    for (key, expected_value) in guard_context {
                        if let Some(obj) = current_value.as_object() {
                            if let Some(actual_value) = obj.get(key) {
                                if actual_value != expected_value {
                                    return Ok(false);
                                }
                            } else {
                                return Ok(false);
                            }
                        } else {
                            return Ok(false);
                        }
                    }
                }

                Ok(true)
            }

            GuardCondition::Custom {
                predicate,
                parameters: _,
            } => {
                // Look up custom predicate result
                Ok(ctx
                    .custom_predicates
                    .get(predicate)
                    .copied()
                    .unwrap_or(false))
            }
        }
    }
}

impl Default for HGuardEvaluatorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GuardEvaluator for HGuardEvaluatorAdapter {
    async fn register_guard(&mut self, guard: HGuard) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut guards = self.guards.write().await;
        guards.insert(guard.id.clone(), guard);
        Ok(())
    }

    async fn unregister_guard(
        &mut self,
        guard_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut guards = self.guards.write().await;
        guards.remove(guard_id);
        Ok(())
    }

    async fn evaluate(
        &self,
        packet: &Packet,
    ) -> Result<Vec<GuardEvaluationResult>, Box<dyn Error + Send + Sync>> {
        let guards = self.guards.read().await;

        let mut results = Vec::new();

        // Evaluate all guards that apply to this packet type
        for guard in guards.values() {
            if guard.packet_type == packet.packet_type {
                let result = self.evaluate_guard(&guard.id, packet).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    async fn evaluate_guard(
        &self,
        guard_id: &str,
        packet: &Packet,
    ) -> Result<GuardEvaluationResult, Box<dyn Error + Send + Sync>> {
        let guards = self.guards.read().await;

        let guard = guards
            .get(guard_id)
            .ok_or_else(|| GuardEvaluationError::GuardNotFound {
                guard_id: guard_id.to_string(),
            })?;

        // Evaluate the guard condition
        match self.evaluate_condition(&guard.condition, packet).await {
            Ok(true) => Ok(GuardEvaluationResult::Satisfied {
                guard_id: guard_id.to_string(),
            }),
            Ok(false) => {
                let reason = format!(
                    "H-guard '{}' violated: {}",
                    guard_id,
                    guard
                        .description
                        .as_deref()
                        .unwrap_or("condition not satisfied")
                );

                Ok(GuardEvaluationResult::Violated {
                    guard_id: guard_id.to_string(),
                    reason,
                    retry_after: None,
                })
            }
            Err(e) => Err(Box::new(e) as Box<dyn Error + Send + Sync>),
        }
    }

    async fn list_guards(&self) -> Result<Vec<HGuard>, Box<dyn Error + Send + Sync>> {
        let guards = self.guards.read().await;
        Ok(guards.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PacketType;

    #[tokio::test]
    async fn test_register_and_list_guards() {
        let mut evaluator = HGuardEvaluatorAdapter::new();

        let guard = HGuard {
            id: "guard-1".to_string(),
            packet_type: PacketType::new("test", "Type1", "1.0"),
            condition: GuardCondition::Custom {
                predicate: "always_true".to_string(),
                parameters: HashMap::new(),
            },
            description: Some("Test guard".to_string()),
        };

        evaluator.register_guard(guard.clone()).await.unwrap();

        let guards = evaluator.list_guards().await.unwrap();
        assert_eq!(guards.len(), 1);
        assert_eq!(guards[0].id, "guard-1");
    }

    #[tokio::test]
    async fn test_unregister_guard() {
        let mut evaluator = HGuardEvaluatorAdapter::new();

        let guard = HGuard {
            id: "guard-1".to_string(),
            packet_type: PacketType::new("test", "Type1", "1.0"),
            condition: GuardCondition::Custom {
                predicate: "test".to_string(),
                parameters: HashMap::new(),
            },
            description: None,
        };

        evaluator.register_guard(guard).await.unwrap();
        assert_eq!(evaluator.list_guards().await.unwrap().len(), 1);

        evaluator.unregister_guard("guard-1").await.unwrap();
        assert_eq!(evaluator.list_guards().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_custom_predicate_guard() {
        let mut evaluator = HGuardEvaluatorAdapter::new();

        // Set predicate to false
        evaluator
            .set_predicate("test_predicate".to_string(), false)
            .await
            .unwrap();

        let guard = HGuard {
            id: "guard-1".to_string(),
            packet_type: PacketType::new("test", "Type1", "1.0"),
            condition: GuardCondition::Custom {
                predicate: "test_predicate".to_string(),
                parameters: HashMap::new(),
            },
            description: Some("Custom predicate test".to_string()),
        };

        evaluator.register_guard(guard.clone()).await.unwrap();

        let packet = Packet {
            id: "pkt-1".to_string(),
            packet_type: PacketType::new("test", "Type1", "1.0"),
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        // Should be violated
        let result = evaluator.evaluate_guard("guard-1", &packet).await.unwrap();
        assert!(matches!(result, GuardEvaluationResult::Violated { .. }));

        // Set predicate to true
        evaluator
            .set_predicate("test_predicate".to_string(), true)
            .await
            .unwrap();

        // Should now be satisfied
        let result = evaluator.evaluate_guard("guard-1", &packet).await.unwrap();
        assert!(matches!(result, GuardEvaluationResult::Satisfied { .. }));
    }

    #[tokio::test]
    async fn test_requires_prior_guard() {
        let mut evaluator = HGuardEvaluatorAdapter::new();

        let required_type = PacketType::new("test", "AuthType", "1.0");
        let guard = HGuard {
            id: "guard-1".to_string(),
            packet_type: PacketType::new("test", "DataType", "1.0"),
            condition: GuardCondition::RequiresPrior {
                packet_type: required_type.clone(),
                packet_id: None,
            },
            description: Some("Requires prior auth".to_string()),
        };

        evaluator.register_guard(guard).await.unwrap();

        let data_packet = Packet {
            id: "pkt-data".to_string(),
            packet_type: PacketType::new("test", "DataType", "1.0"),
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        // Should be violated initially
        let result = evaluator
            .evaluate_guard("guard-1", &data_packet)
            .await
            .unwrap();
        assert!(matches!(result, GuardEvaluationResult::Violated { .. }));

        // Process auth packet
        let auth_packet = Packet {
            id: "pkt-auth".to_string(),
            packet_type: required_type,
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };
        evaluator.record_packet(auth_packet).await.unwrap();

        // Should now be satisfied
        let result = evaluator
            .evaluate_guard("guard-1", &data_packet)
            .await
            .unwrap();
        assert!(matches!(result, GuardEvaluationResult::Satisfied { .. }));
    }

    #[tokio::test]
    async fn test_state_requirement_guard() {
        let mut evaluator = HGuardEvaluatorAdapter::new();

        let guard = HGuard {
            id: "guard-1".to_string(),
            packet_type: PacketType::new("test", "Type1", "1.0"),
            condition: GuardCondition::StateRequirement {
                required_state: "authorized".to_string(),
                context: HashMap::new(),
            },
            description: Some("Requires authorized state".to_string()),
        };

        evaluator.register_guard(guard).await.unwrap();

        let packet = Packet {
            id: "pkt-1".to_string(),
            packet_type: PacketType::new("test", "Type1", "1.0"),
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        // Should be violated initially
        let result = evaluator.evaluate_guard("guard-1", &packet).await.unwrap();
        assert!(matches!(result, GuardEvaluationResult::Violated { .. }));

        // Set state
        evaluator
            .update_state("authorized".to_string(), serde_json::json!(true))
            .await
            .unwrap();

        // Should now be satisfied
        let result = evaluator.evaluate_guard("guard-1", &packet).await.unwrap();
        assert!(matches!(result, GuardEvaluationResult::Satisfied { .. }));
    }
}
