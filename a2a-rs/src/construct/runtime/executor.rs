//! Runtime μ function - Core execution engine dispatcher and executor
//!
//! Implements the total compiler/runtime function: A = μ(O)
//!
//! Execution pipeline:
//! 1. Type check against ontology
//! 2. Admission guard evaluation
//! 3. Apply Λ (transformations via scheduler)
//! 4. Check Q (invariants)
//! 5. Execute Δ (state deltas)
//! 6. Emit receipts
//!
//! The Runtime orchestrates all CONSTRUCT components into a single execution flow.

use crate::construct::guards::{Guard, RefusalReceipt};
use crate::construct::invariants::{InvariantRegistry, InvariantViolation};
use crate::construct::ontology::OntologyState;
use crate::construct::runtime::{PriorityClass, ScheduledTask, Scheduler};
use crate::domain::{A2AError, Artifact, Message, Task, TaskState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, warn};

/// Runtime execution errors
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "errorType")]
pub enum RuntimeError {
    #[error("Type check failed: {message}")]
    TypeCheckFailed { message: String },

    #[error("Admission denied: {receipt}")]
    AdmissionDenied { receipt: String },

    #[error("Transformation failed: {message}")]
    TransformationFailed { message: String },

    #[error("Invariant violation: {violation}")]
    InvariantViolation { violation: String },

    #[error("Execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("Bounded update limit exceeded: {limit}")]
    BoundedUpdateExceeded { limit: usize },

    #[error("Scheduler error: {message}")]
    SchedulerError { message: String },

    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
}

impl From<RuntimeError> for A2AError {
    fn from(err: RuntimeError) -> Self {
        A2AError::Internal(err.to_string())
    }
}

impl From<InvariantViolation> for RuntimeError {
    fn from(violation: InvariantViolation) -> Self {
        RuntimeError::InvariantViolation {
            violation: violation.to_string(),
        }
    }
}

/// Operation input to the runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "operationType")]
pub enum Operation {
    /// Create a new task
    CreateTask {
        task: Task,
        initial_message: Option<Message>,
        priority: Option<PriorityClass>,
    },

    /// Send a message to a task
    SendMessage { task_id: String, message: Message },

    /// Update task state
    UpdateTaskState { task_id: String, state: TaskState },

    /// Add artifact to task
    AddArtifact { task_id: String, artifact: Artifact },

    /// Complete a task
    CompleteTask { task_id: String, station_id: String },

    /// Cancel a pending task
    CancelTask { task_id: String },
}

/// Output emitted by the runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOutput {
    /// Tasks created or modified
    pub tasks: Vec<Task>,

    /// Events emitted during execution
    pub events: Vec<RuntimeEvent>,

    /// Artifacts generated
    pub artifacts: Vec<Artifact>,

    /// Errors encountered (non-fatal warnings)
    pub errors: Vec<RuntimeError>,

    /// Execution receipt proving operation occurred
    pub receipt: ExecutionReceipt,
}

/// Events emitted during runtime execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "eventType")]
pub enum RuntimeEvent {
    TaskCreated {
        task_id: String,
        station_id: String,
        epoch: u64,
        timestamp: String,
    },

    TaskScheduled {
        task_id: String,
        station_id: String,
        priority: PriorityClass,
        epoch: u64,
        timestamp: String,
    },

    TaskStateChanged {
        task_id: String,
        old_state: TaskState,
        new_state: TaskState,
        timestamp: String,
    },

    MessageProcessed {
        task_id: String,
        message_id: String,
        timestamp: String,
    },

    ArtifactAdded {
        task_id: String,
        artifact_name: String,
        timestamp: String,
    },

    TransformationApplied {
        name: String,
        timestamp: String,
    },

    InvariantChecked {
        invariant: String,
        passed: bool,
        timestamp: String,
    },

    GuardEvaluated {
        guard: String,
        admitted: bool,
        timestamp: String,
    },
}

/// Execution receipt proving operation was processed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReceipt {
    /// Unique execution ID
    pub execution_id: String,

    /// Operation that was executed
    pub operation: String,

    /// Timestamp of execution start
    pub timestamp: String,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Pipeline stages completed
    pub stages_completed: Vec<String>,

    /// Whether execution succeeded
    pub success: bool,

    /// Policy epoch at time of execution
    pub policy_epoch: u64,
}

/// Execution context for transformations and checks
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Current ontology state
    pub ontology: OntologyState,

    /// Metadata for this execution
    pub metadata: HashMap<String, serde_json::Value>,

    /// Policy epoch
    pub policy_epoch: u64,
}

impl ExecutionContext {
    pub fn new(ontology: OntologyState, policy_epoch: u64) -> Self {
        Self {
            ontology,
            metadata: HashMap::new(),
            policy_epoch,
        }
    }

    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// The Runtime - implements μ(O)
///
/// Brings together:
/// - O: Ontology state
/// - Λ: Scheduler (transformations)
/// - G: Guards (admission control)
/// - Q: Invariants (correctness checks)
pub struct Runtime {
    /// Ontology state - the O in μ(O)
    ontology: OntologyState,

    /// Scheduler - implements Λ (ordered execution)
    scheduler: Scheduler,

    /// Guards - admission control predicates
    guards: Vec<Arc<dyn Guard>>,

    /// Invariant registry - Q (correctness checks)
    invariants: InvariantRegistry<Task>,

    /// Policy epoch for guard evaluation
    policy_epoch: u64,

    /// Bounded update limit
    update_limit: usize,
}

impl Runtime {
    /// Create a new runtime with provided components
    pub fn new(
        ontology: OntologyState,
        scheduler: Scheduler,
        guards: Vec<Arc<dyn Guard>>,
        invariants: InvariantRegistry<Task>,
    ) -> Self {
        Self {
            ontology,
            scheduler,
            guards,
            invariants,
            policy_epoch: 0,
            update_limit: 1000,
        }
    }

    /// Create a default runtime with basic configuration
    pub fn default_runtime() -> Self {
        let ontology = OntologyState::new();
        let scheduler = Scheduler::new(10);
        let guards = Vec::new();
        let invariants = InvariantRegistry::new();

        Self::new(ontology, scheduler, guards, invariants)
    }

    /// Set policy epoch
    pub fn with_policy_epoch(mut self, epoch: u64) -> Self {
        self.policy_epoch = epoch;
        self
    }

    /// Set bounded update limit
    pub fn with_update_limit(mut self, limit: usize) -> Self {
        self.update_limit = limit;
        self
    }

    /// Get reference to ontology state
    pub fn ontology(&self) -> &OntologyState {
        &self.ontology
    }

    /// Get mutable reference to ontology state
    pub fn ontology_mut(&mut self) -> &mut OntologyState {
        &mut self.ontology
    }

    /// Get reference to scheduler
    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Get mutable reference to scheduler
    pub fn scheduler_mut(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    /// A = μ(O) - The total compiler/runtime function
    ///
    /// Executes the complete pipeline:
    /// 1. Type check against ontology
    /// 2. Admission guard evaluation
    /// 3. Apply Λ (transformations via scheduler)
    /// 4. Check Q (invariants)
    /// 5. Execute Δ (state deltas)
    /// 6. Emit receipts
    pub fn handle(&mut self, operation: Operation) -> Result<RuntimeOutput, RuntimeError> {
        let start_time = std::time::Instant::now();
        let execution_id = uuid::Uuid::new_v4().to_string();
        let mut stages_completed = Vec::new();
        let mut events = Vec::new();
        let mut errors = Vec::new();
        let mut artifacts = Vec::new();

        #[cfg(feature = "tracing")]
        info!("Runtime::handle - Starting execution {}", execution_id);

        // Stage 1: Type Check
        #[cfg(feature = "tracing")]
        debug!("Stage 1: Type checking operation");

        if let Err(e) = self.type_check(&operation) {
            #[cfg(feature = "tracing")]
            error!("Type check failed: {}", e);

            return Ok(RuntimeOutput {
                tasks: Vec::new(),
                events: Vec::new(),
                artifacts: Vec::new(),
                errors: vec![e.clone()],
                receipt: self.create_receipt(
                    execution_id,
                    operation,
                    start_time,
                    stages_completed,
                    false,
                ),
            });
        }
        stages_completed.push("type_check".to_string());

        // Stage 2: Admission Guard
        #[cfg(feature = "tracing")]
        debug!("Stage 2: Checking admission guards");

        match self.check_guards(&operation, &mut events) {
            Ok(_) => {
                stages_completed.push("admission_guard".to_string());
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                warn!("Admission denied: {}", e);

                return Ok(RuntimeOutput {
                    tasks: Vec::new(),
                    events,
                    artifacts: Vec::new(),
                    errors: vec![e.clone()],
                    receipt: self.create_receipt(
                        execution_id,
                        operation,
                        start_time,
                        stages_completed,
                        false,
                    ),
                });
            }
        }

        // Stage 3: Apply Λ (Transformations via Scheduler)
        #[cfg(feature = "tracing")]
        debug!("Stage 3: Applying transformations");

        if let Err(e) = self.apply_transformations(&operation, &mut events) {
            #[cfg(feature = "tracing")]
            error!("Transformation failed: {}", e);
            errors.push(e);
        } else {
            stages_completed.push("transformations".to_string());
        }

        // Stage 4: Check Q (Invariants)
        #[cfg(feature = "tracing")]
        debug!("Stage 4: Checking invariants");

        if let Err(e) = self.check_invariants(&operation, &mut events) {
            #[cfg(feature = "tracing")]
            error!("Invariant violation: {}", e);

            return Ok(RuntimeOutput {
                tasks: self.collect_tasks(),
                events,
                artifacts,
                errors: vec![e.clone()],
                receipt: self.create_receipt(
                    execution_id,
                    operation,
                    start_time,
                    stages_completed,
                    false,
                ),
            });
        }
        stages_completed.push("invariants".to_string());

        // Stage 5: Execute Δ (Delta - state changes)
        #[cfg(feature = "tracing")]
        debug!("Stage 5: Executing deltas");

        match self.execute_delta(&operation, &mut events, &mut artifacts) {
            Ok(_) => {
                stages_completed.push("delta_execution".to_string());
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                error!("Execution failed: {}", e);
                errors.push(e);
            }
        }

        // Stage 6: Emit Receipts
        #[cfg(feature = "tracing")]
        debug!("Stage 6: Emitting receipts");

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let success = errors.is_empty();

        stages_completed.push("receipt_emission".to_string());

        #[cfg(feature = "tracing")]
        info!(
            "Runtime::handle - Execution {} completed in {}ms, success: {}",
            execution_id, duration_ms, success
        );

        Ok(RuntimeOutput {
            tasks: self.collect_tasks(),
            events,
            artifacts,
            errors,
            receipt: self.create_receipt(
                execution_id,
                operation,
                start_time,
                stages_completed,
                success,
            ),
        })
    }

    /// Type check operation against ontology
    fn type_check(&self, operation: &Operation) -> Result<(), RuntimeError> {
        match operation {
            Operation::CreateTask { task, .. } => {
                // Check if task would exceed bounds
                let task_count = self.ontology.task_count();
                let max_tasks = self.ontology.bounds().max_tasks;
                if task_count >= max_tasks && !self.ontology.get_task(&task.id).is_some() {
                    return Err(RuntimeError::TypeCheckFailed {
                        message: format!(
                            "Task limit {} exceeded, cannot create task {}",
                            max_tasks, task.id
                        ),
                    });
                }
            }
            Operation::SendMessage { task_id, .. } => {
                // Verify task exists
                if self.ontology.get_task(task_id).is_none() {
                    return Err(RuntimeError::TypeCheckFailed {
                        message: format!("Task {} not found", task_id),
                    });
                }
            }
            Operation::UpdateTaskState { task_id, .. } => {
                // Verify task exists
                if self.ontology.get_task(task_id).is_none() {
                    return Err(RuntimeError::TypeCheckFailed {
                        message: format!("Task {} not found", task_id),
                    });
                }
            }
            Operation::AddArtifact { task_id, .. } => {
                // Verify task exists
                if self.ontology.get_task(task_id).is_none() {
                    return Err(RuntimeError::TypeCheckFailed {
                        message: format!("Task {} not found", task_id),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Check admission guards
    fn check_guards(
        &self,
        operation: &Operation,
        events: &mut Vec<RuntimeEvent>,
    ) -> Result<(), RuntimeError> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let operation_json =
            serde_json::to_value(operation).map_err(|e| RuntimeError::InvalidOperation {
                message: format!("Failed to serialize operation: {}", e),
            })?;

        for guard in &self.guards {
            match guard.check(&operation_json, "operation", self.policy_epoch) {
                Ok(_) => {
                    events.push(RuntimeEvent::GuardEvaluated {
                        guard: guard.name().to_string(),
                        admitted: true,
                        timestamp: timestamp.clone(),
                    });
                }
                Err(receipt) => {
                    events.push(RuntimeEvent::GuardEvaluated {
                        guard: guard.name().to_string(),
                        admitted: false,
                        timestamp: timestamp.clone(),
                    });

                    return Err(RuntimeError::AdmissionDenied {
                        receipt: format!("{}", receipt),
                    });
                }
            }
        }

        Ok(())
    }

    /// Apply transformations (Λ) via scheduler
    fn apply_transformations(
        &mut self,
        operation: &Operation,
        events: &mut Vec<RuntimeEvent>,
    ) -> Result<(), RuntimeError> {
        let timestamp = chrono::Utc::now().to_rfc3339();

        match operation {
            Operation::CreateTask {
                task,
                initial_message,
                priority,
            } => {
                // Add task to ontology
                self.ontology.put_task(task.clone()).map_err(|e| {
                    RuntimeError::TransformationFailed {
                        message: format!("Failed to add task to ontology: {}", e),
                    }
                })?;

                // Add initial message if provided
                if let Some(msg) = initial_message {
                    self.ontology
                        .add_message(&task.id, msg.clone())
                        .map_err(|e| RuntimeError::TransformationFailed {
                            message: format!("Failed to add initial message: {}", e),
                        })?;
                }

                // Schedule task
                let scheduled_task = ScheduledTask::new(
                    task.id.clone(),
                    "default".to_string(), // TODO: Make configurable
                    self.scheduler.current_epoch(),
                    priority.unwrap_or(PriorityClass::Normal),
                );

                self.scheduler.submit(scheduled_task).map_err(|e| {
                    RuntimeError::SchedulerError {
                        message: e.to_string(),
                    }
                })?;

                events.push(RuntimeEvent::TaskCreated {
                    task_id: task.id.clone(),
                    station_id: "default".to_string(),
                    epoch: self.scheduler.current_epoch(),
                    timestamp,
                });
            }
            Operation::SendMessage { task_id, message } => {
                self.ontology
                    .add_message(task_id, message.clone())
                    .map_err(|e| RuntimeError::TransformationFailed {
                        message: format!("Failed to add message: {}", e),
                    })?;
            }
            Operation::UpdateTaskState { task_id, state } => {
                if let Some(task) = self.ontology.get_task_mut(task_id) {
                    task.status.state = state.clone();
                }
            }
            Operation::AddArtifact { task_id, artifact } => {
                // Store artifact in context for emission
                // Note: Artifacts are returned in the output, not stored in ontology
            }
            Operation::CompleteTask {
                task_id,
                station_id,
            } => {
                self.scheduler.complete(task_id, station_id).map_err(|e| {
                    RuntimeError::SchedulerError {
                        message: e.to_string(),
                    }
                })?;
            }
            Operation::CancelTask { task_id } => {
                self.scheduler
                    .cancel(task_id)
                    .map_err(|e| RuntimeError::SchedulerError {
                        message: e.to_string(),
                    })?;
            }
        }

        Ok(())
    }

    /// Check invariants (Q)
    fn check_invariants(
        &self,
        operation: &Operation,
        events: &mut Vec<RuntimeEvent>,
    ) -> Result<(), RuntimeError> {
        let timestamp = chrono::Utc::now().to_rfc3339();

        // Check task invariants if operation involves a task
        match operation {
            Operation::CreateTask { task, .. } => {
                // Check all invariants
                if let Err(violation) = self.invariants.check_all(task) {
                    events.push(RuntimeEvent::InvariantChecked {
                        invariant: "task_invariants".to_string(),
                        passed: false,
                        timestamp,
                    });

                    return Err(RuntimeError::from(violation));
                }

                events.push(RuntimeEvent::InvariantChecked {
                    invariant: "task_invariants".to_string(),
                    passed: true,
                    timestamp,
                });
            }
            Operation::UpdateTaskState { task_id, .. } => {
                if let Some(task) = self.ontology.get_task(task_id) {
                    // Check all invariants
                    if let Err(violation) = self.invariants.check_all(task) {
                        events.push(RuntimeEvent::InvariantChecked {
                            invariant: "task_invariants".to_string(),
                            passed: false,
                            timestamp,
                        });

                        return Err(RuntimeError::from(violation));
                    }

                    events.push(RuntimeEvent::InvariantChecked {
                        invariant: "task_invariants".to_string(),
                        passed: true,
                        timestamp,
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Execute delta (Δ) - apply final state changes
    fn execute_delta(
        &mut self,
        operation: &Operation,
        events: &mut Vec<RuntimeEvent>,
        artifacts: &mut Vec<Artifact>,
    ) -> Result<(), RuntimeError> {
        // Check bounded update limit
        let total_updates = self.ontology.task_count();
        if total_updates > self.update_limit {
            return Err(RuntimeError::BoundedUpdateExceeded {
                limit: self.update_limit,
            });
        }

        let timestamp = chrono::Utc::now().to_rfc3339();

        match operation {
            Operation::SendMessage { task_id, message } => {
                events.push(RuntimeEvent::MessageProcessed {
                    task_id: task_id.clone(),
                    message_id: message.message_id.clone(),
                    timestamp,
                });
            }
            Operation::UpdateTaskState { task_id, state } => {
                if let Some(task) = self.ontology.get_task(task_id) {
                    events.push(RuntimeEvent::TaskStateChanged {
                        task_id: task_id.clone(),
                        old_state: task.status.state.clone(),
                        new_state: state.clone(),
                        timestamp,
                    });
                }
            }
            Operation::AddArtifact { task_id, artifact } => {
                artifacts.push(artifact.clone());
                events.push(RuntimeEvent::ArtifactAdded {
                    task_id: task_id.clone(),
                    artifact_name: artifact.name.clone().unwrap_or_default(),
                    timestamp,
                });
            }
            _ => {}
        }

        Ok(())
    }

    /// Collect all tasks from ontology
    fn collect_tasks(&self) -> Vec<Task> {
        self.ontology.get_all_tasks().into_iter().cloned().collect()
    }

    /// Create execution receipt
    fn create_receipt(
        &self,
        execution_id: String,
        operation: Operation,
        start_time: std::time::Instant,
        stages_completed: Vec<String>,
        success: bool,
    ) -> ExecutionReceipt {
        ExecutionReceipt {
            execution_id,
            operation: format!("{:?}", operation),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: start_time.elapsed().as_millis() as u64,
            stages_completed,
            success,
            policy_epoch: self.policy_epoch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskStatus;

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::default_runtime();
        assert_eq!(runtime.ontology().task_count(), 0);
    }

    #[test]
    fn test_create_task_operation() {
        let mut runtime = Runtime::default_runtime();

        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        let operation = Operation::CreateTask {
            task,
            initial_message: None,
            priority: Some(PriorityClass::Normal),
        };

        let result = runtime.handle(operation);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.receipt.success);
        assert_eq!(output.tasks.len(), 1);
    }

    #[test]
    fn test_send_message_operation() {
        let mut runtime = Runtime::default_runtime();

        // First create a task
        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        runtime.ontology_mut().put_task(task).unwrap();

        // Now send a message
        let message = Message::user_text("Hello".to_string(), "msg-1".to_string());
        let operation = Operation::SendMessage {
            task_id: "task-1".to_string(),
            message,
        };

        let result = runtime.handle(operation);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.receipt.success);
    }

    #[test]
    fn test_type_check_failure() {
        let mut runtime = Runtime::default_runtime();

        // Try to send message to non-existent task
        let message = Message::user_text("Hello".to_string(), "msg-1".to_string());
        let operation = Operation::SendMessage {
            task_id: "nonexistent".to_string(),
            message,
        };

        let result = runtime.handle(operation);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.receipt.success);
        assert!(!output.errors.is_empty());
    }

    #[test]
    fn test_execution_receipt() {
        let mut runtime = Runtime::default_runtime().with_policy_epoch(42);

        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        let operation = Operation::CreateTask {
            task,
            initial_message: None,
            priority: None,
        };

        let result = runtime.handle(operation);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.receipt.policy_epoch, 42);
        assert!(output.receipt.duration_ms > 0);
        assert!(!output.receipt.stages_completed.is_empty());
    }
}
