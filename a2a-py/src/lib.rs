//! Python bindings for the A2A CONSTRUCT runtime
//!
//! This module provides Python access to the CONSTRUCT deterministic runtime,
//! including ontology state management, stations, and cryptographic receipts.
//!
//! # Design
//!
//! - **JSON boundary**: All complex types cross the FFI boundary as JSON strings
//! - **Error handling**: Rust errors are converted to Python exceptions
//! - **Memory safety**: PyO3 ensures safe memory management between Rust and Python
//!
//! # Example (Python)
//!
//! ```python
//! from a2a_construct import OntologyState, Receipt, ReceiptChain
//!
//! # Create ontology state
//! state = OntologyState.new()
//! print(f"Tasks: {state.task_count()}")
//!
//! # Create receipts
//! receipt = Receipt.new(b"observation", b"action", b"delta")
//! chain = ReceiptChain.new()
//! chain.add_receipt(receipt)
//! print(f"Chain length: {chain.length()}")
//! ```

use a2a_rs::construct::{
    OntologyState as RustOntologyState, StateBounds as RustStateBounds,
    StateStats as RustStateStats,
};
use a2a_rs::domain::core::{AgentCard as RustAgentCard, Message as RustMessage, Task as RustTask};
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

#[cfg(feature = "receipts")]
use a2a_rs::construct::receipts::{
    Receipt as RustReceipt, ReceiptChain as RustReceiptChain, ReceiptError as RustReceiptError,
};

/// Python exception for CONSTRUCT errors
create_exception!(a2a_construct, ConstructError, PyException);

/// Helper to convert Rust errors to Python exceptions
fn to_py_err<E: std::fmt::Display>(e: E) -> PyErr {
    ConstructError::new_err(format!("{}", e))
}

/// Ontology state bounds configuration
///
/// Controls the maximum number of entities stored in the ontology to prevent
/// unbounded memory growth.
#[pyclass(name = "StateBounds")]
#[derive(Clone)]
struct PyStateBounds {
    inner: RustStateBounds,
}

#[pymethods]
impl PyStateBounds {
    /// Create new state bounds with default values
    #[new]
    #[pyo3(signature = (max_tasks=None, max_messages_per_task=None, max_agents=None))]
    fn new(
        max_tasks: Option<usize>,
        max_messages_per_task: Option<usize>,
        max_agents: Option<usize>,
    ) -> Self {
        let default = RustStateBounds::default();
        Self {
            inner: RustStateBounds {
                max_tasks: max_tasks.unwrap_or(default.max_tasks),
                max_messages_per_task: max_messages_per_task
                    .unwrap_or(default.max_messages_per_task),
                max_agents: max_agents.unwrap_or(default.max_agents),
            },
        }
    }

    #[getter]
    fn max_tasks(&self) -> usize {
        self.inner.max_tasks
    }

    #[getter]
    fn max_messages_per_task(&self) -> usize {
        self.inner.max_messages_per_task
    }

    #[getter]
    fn max_agents(&self) -> usize {
        self.inner.max_agents
    }

    fn __repr__(&self) -> String {
        format!(
            "StateBounds(max_tasks={}, max_messages_per_task={}, max_agents={})",
            self.inner.max_tasks, self.inner.max_messages_per_task, self.inner.max_agents
        )
    }
}

/// Ontology state statistics
#[pyclass(name = "StateStats")]
#[derive(Clone)]
struct PyStateStats {
    inner: RustStateStats,
}

#[pymethods]
impl PyStateStats {
    #[getter]
    fn task_count(&self) -> usize {
        self.inner.task_count
    }

    #[getter]
    fn agent_count(&self) -> usize {
        self.inner.agent_count
    }

    #[getter]
    fn notification_config_count(&self) -> usize {
        self.inner.notification_config_count
    }

    #[getter]
    fn context_count(&self) -> usize {
        self.inner.context_count
    }

    #[getter]
    fn total_messages(&self) -> usize {
        self.inner.total_messages
    }

    fn __repr__(&self) -> String {
        format!(
            "StateStats(tasks={}, agents={}, notifications={}, contexts={}, messages={})",
            self.inner.task_count,
            self.inner.agent_count,
            self.inner.notification_config_count,
            self.inner.context_count,
            self.inner.total_messages
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(to_py_err)
    }
}

/// Ontology state model representing the complete protocol state
///
/// This class holds all protocol entities (tasks, messages, agents, notification configs)
/// and provides indices for efficient lookups.
///
/// # Example
///
/// ```python
/// state = OntologyState.new()
/// task_json = '{"id": "task-1", "contextId": "ctx-1", "status": {"state": "pending"}}'
/// state.put_task_json(task_json)
/// print(state.task_count())  # 1
/// ```
#[pyclass(name = "OntologyState")]
struct PyOntologyState {
    inner: RustOntologyState,
}

#[pymethods]
impl PyOntologyState {
    /// Create a new empty ontology state with default bounds
    #[new]
    #[pyo3(signature = (bounds=None))]
    fn new(bounds: Option<PyStateBounds>) -> Self {
        let inner = match bounds {
            Some(b) => RustOntologyState::with_bounds(b.inner),
            None => RustOntologyState::new(),
        };
        Self { inner }
    }

    /// Check if the state is empty
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the state bounds configuration
    fn bounds(&self) -> PyStateBounds {
        PyStateBounds {
            inner: self.inner.bounds().clone(),
        }
    }

    /// Get the number of tasks in the state
    fn task_count(&self) -> usize {
        self.inner.task_count()
    }

    /// Get the number of agents in the state
    fn agent_count(&self) -> usize {
        self.inner.agent_count()
    }

    /// Get statistics about the current state
    fn stats(&self) -> PyStateStats {
        PyStateStats {
            inner: self.inner.stats(),
        }
    }

    /// Add or update a task from JSON string
    ///
    /// # Arguments
    ///
    /// * `task_json` - JSON string representing a Task object
    ///
    /// # Example
    ///
    /// ```python
    /// task_json = '{"id": "task-1", "contextId": "ctx-1", "status": {"state": "pending"}}'
    /// state.put_task_json(task_json)
    /// ```
    fn put_task_json(&mut self, task_json: &str) -> PyResult<()> {
        let task: RustTask = serde_json::from_str(task_json).map_err(to_py_err)?;
        self.inner.put_task(task).map_err(to_py_err)
    }

    /// Get a task by ID as JSON string
    ///
    /// Returns None if the task doesn't exist.
    fn get_task_json(&self, task_id: &str) -> PyResult<Option<String>> {
        match self.inner.get_task(task_id) {
            Some(task) => {
                let json = serde_json::to_string(task).map_err(to_py_err)?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }

    /// Get all tasks as JSON array string
    fn get_all_tasks_json(&self) -> PyResult<String> {
        let tasks = self.inner.get_all_tasks();
        serde_json::to_string(&tasks).map_err(to_py_err)
    }

    /// Remove a task by ID
    ///
    /// Returns the removed task as JSON string, or None if not found.
    fn remove_task(&mut self, task_id: &str) -> PyResult<Option<String>> {
        match self.inner.remove_task(task_id) {
            Some(task) => {
                let json = serde_json::to_string(&task).map_err(to_py_err)?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }

    /// Add a message to a task from JSON string
    fn add_message_json(&mut self, task_id: &str, message_json: &str) -> PyResult<()> {
        let message: RustMessage = serde_json::from_str(message_json).map_err(to_py_err)?;
        self.inner.add_message(task_id, message).map_err(to_py_err)
    }

    /// Get messages for a task as JSON array string
    fn get_messages_json(&self, task_id: &str) -> PyResult<Option<String>> {
        match self.inner.get_messages(task_id) {
            Some(messages) => {
                let json = serde_json::to_string(messages).map_err(to_py_err)?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }

    /// Get the number of messages for a task
    fn message_count(&self, task_id: &str) -> usize {
        self.inner.message_count(task_id)
    }

    /// Register or update an agent from JSON string
    fn put_agent_json(&mut self, agent_json: &str) -> PyResult<()> {
        let agent: RustAgentCard = serde_json::from_str(agent_json).map_err(to_py_err)?;
        self.inner.put_agent(agent).map_err(to_py_err)
    }

    /// Get an agent by name as JSON string
    fn get_agent_json(&self, agent_name: &str) -> PyResult<Option<String>> {
        match self.inner.get_agent(agent_name) {
            Some(agent) => {
                let json = serde_json::to_string(agent).map_err(to_py_err)?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }

    /// Clear all state
    fn clear(&mut self) {
        self.inner.clear();
    }

    /// Export state as JSON string
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(to_py_err)
    }

    /// Import state from JSON string
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RustOntologyState = serde_json::from_str(json).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "OntologyState(tasks={}, agents={}, messages={})",
            self.inner.task_count(),
            self.inner.agent_count(),
            self.inner.stats().total_messages
        )
    }
}

/// Cryptographic receipt binding observation, action, and state delta
///
/// Receipts form the basis of the tamper-proof audit trail in CONSTRUCT.
/// Each receipt captures a single state transition with three components:
/// - Observation: The input state or trigger
/// - Action: The output behavior or response
/// - Delta: The state change or effect
///
/// # Example
///
/// ```python
/// receipt = Receipt.new(b"observation", b"action", b"delta")
/// print(receipt.receipt_hash())
/// print(receipt.to_json())
/// ```
#[cfg(feature = "receipts")]
#[pyclass(name = "Receipt")]
struct PyReceipt {
    inner: RustReceipt,
}

#[cfg(feature = "receipts")]
#[pymethods]
impl PyReceipt {
    /// Create a new receipt from observation, action, and delta bytes
    #[new]
    fn new(py: Python, observation: &PyBytes, action: &PyBytes, delta: &PyBytes) -> Self {
        let inner = RustReceipt::new(observation.as_bytes(), action.as_bytes(), delta.as_bytes());
        Self { inner }
    }

    /// Get the receipt sequence number
    #[getter]
    fn sequence(&self) -> u64 {
        self.inner.sequence
    }

    /// Get the timestamp as ISO 8601 string
    #[getter]
    fn timestamp(&self) -> String {
        self.inner.timestamp.to_rfc3339()
    }

    /// Get the observation hash
    #[getter]
    fn observation_hash(&self) -> String {
        self.inner.observation_hash.clone()
    }

    /// Get the action hash
    #[getter]
    fn action_hash(&self) -> String {
        self.inner.action_hash.clone()
    }

    /// Get the delta hash
    #[getter]
    fn delta_hash(&self) -> String {
        self.inner.delta_hash.clone()
    }

    /// Get the combined receipt hash
    #[getter]
    fn receipt_hash(&self) -> String {
        self.inner.receipt_hash.clone()
    }

    /// Get the previous receipt hash (None for genesis receipt)
    #[getter]
    fn previous_hash(&self) -> Option<String> {
        self.inner.previous_hash.clone()
    }

    /// Verify the receipt's internal hashes are consistent
    fn verify_hashes(&self) -> PyResult<()> {
        self.inner.verify_hashes().map_err(to_py_err)
    }

    /// Export receipt as JSON string
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(to_py_err)
    }

    /// Import receipt from JSON string
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RustReceipt = serde_json::from_str(json).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "Receipt(seq={}, hash={}...)",
            self.inner.sequence,
            &self.inner.receipt_hash[..16]
        )
    }
}

/// A chain of receipts forming a tamper-proof audit trail
///
/// Each receipt links to the previous one via cryptographic hashes, creating
/// a structure similar to a blockchain. The chain can be verified for integrity.
///
/// # Example
///
/// ```python
/// chain = ReceiptChain.new()
/// chain.add_transition(b"obs1", b"act1", b"delta1")
/// chain.add_transition(b"obs2", b"act2", b"delta2")
/// assert chain.verify_integrity()
/// print(f"Chain length: {chain.length()}")
/// ```
#[cfg(feature = "receipts")]
#[pyclass(name = "ReceiptChain")]
struct PyReceiptChain {
    inner: RustReceiptChain,
}

#[cfg(feature = "receipts")]
#[pymethods]
impl PyReceiptChain {
    /// Create a new empty receipt chain
    #[new]
    fn new() -> Self {
        Self {
            inner: RustReceiptChain::new(),
        }
    }

    /// Add a receipt to the chain
    ///
    /// The receipt will be assigned the next sequence number and linked to
    /// the previous receipt's hash.
    fn add_receipt(&mut self, receipt: PyReceipt) {
        self.inner.add_receipt(receipt.inner);
    }

    /// Create and add a new receipt from raw components
    fn add_transition(
        &mut self,
        py: Python,
        observation: &PyBytes,
        action: &PyBytes,
        delta: &PyBytes,
    ) -> PyReceipt {
        let receipt =
            self.inner
                .add_transition(observation.as_bytes(), action.as_bytes(), delta.as_bytes());
        PyReceipt {
            inner: receipt.clone(),
        }
    }

    /// Verify the integrity of the entire receipt chain
    ///
    /// Checks:
    /// 1. All receipts have correct internal hashes
    /// 2. All receipts link properly to their predecessors
    /// 3. Sequence numbers are consecutive
    ///
    /// Returns True if valid, raises ConstructError if invalid.
    fn verify_integrity(&self) -> PyResult<bool> {
        self.inner.verify_integrity().map_err(to_py_err)?;
        Ok(true)
    }

    /// Get the number of receipts in the chain
    fn length(&self) -> usize {
        self.inner.len()
    }

    /// Check if the chain is empty
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get a receipt by sequence number
    fn get(&self, sequence: u64) -> Option<PyReceipt> {
        self.inner
            .get(sequence)
            .map(|r| PyReceipt { inner: r.clone() })
    }

    /// Get the most recent receipt in the chain
    fn latest(&self) -> Option<PyReceipt> {
        self.inner.latest().map(|r| PyReceipt { inner: r.clone() })
    }

    /// Export chain as JSON string
    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json().map_err(to_py_err)
    }

    /// Import chain from JSON string
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = RustReceiptChain::from_json(json).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!("ReceiptChain(length={})", self.inner.len())
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }
}

/// A Python module for the A2A CONSTRUCT runtime
#[pymodule]
fn a2a_construct(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyStateBounds>()?;
    m.add_class::<PyStateStats>()?;
    m.add_class::<PyOntologyState>()?;

    #[cfg(feature = "receipts")]
    {
        m.add_class::<PyReceipt>()?;
        m.add_class::<PyReceiptChain>()?;
    }

    m.add("ConstructError", py.get_type::<ConstructError>())?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__doc__", "Python bindings for the A2A CONSTRUCT runtime - deterministic agent execution with cryptographic receipts")?;

    Ok(())
}
