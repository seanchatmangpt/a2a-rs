//! Task Graph for coordination topology
//!
//! This module implements a directed acyclic graph (DAG) for managing task dependencies
//! and coordination. Tasks can have prerequisites that must complete before they can start,
//! supporting complex multi-agent workflows.
//!
//! # Coordination Patterns
//!
//! - **Sequential**: Task B depends on Task A completing
//! - **Fan-out**: Multiple tasks depend on a single predecessor
//! - **Join**: A task depends on multiple predecessors (all must complete)
//! - **Cancellation propagation**: Canceling a task propagates to its dependents
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::TaskGraph;
//! use a2a_rs::domain::{Task, TaskState};
//!
//! let mut graph = TaskGraph::new();
//!
//! // Add tasks
//! let task_a = Task::new("task-a".to_string(), "ctx-1".to_string());
//! let task_b = Task::new("task-b".to_string(), "ctx-1".to_string());
//!
//! graph.add_task(task_a.clone());
//! graph.add_task(task_b.clone());
//!
//! // Task B depends on Task A
//! graph.add_dependency("task-b", "task-a").unwrap();
//!
//! // Check if task B can start (requires task A to complete)
//! assert!(!graph.prerequisites_met("task-b").unwrap());
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

use crate::domain::{Artifact, Task, TaskState};

/// Errors that can occur during task coordination
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum CoordinationError {
    /// Task not found in the graph
    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },

    /// Attempted to add a cyclic dependency
    #[error("Cyclic dependency detected: {from} -> {to}")]
    CyclicDependency { from: String, to: String },

    /// Task already exists in the graph
    #[error("Task already exists: {task_id}")]
    TaskAlreadyExists { task_id: String },

    /// Dependency already exists
    #[error("Dependency already exists: {from} -> {to}")]
    DependencyAlreadyExists { from: String, to: String },

    /// Prerequisites not met
    #[error("Prerequisites not met for task {task_id}: missing {missing:?}")]
    PrerequisitesNotMet {
        task_id: String,
        missing: Vec<String>,
    },

    /// Invalid state transition
    #[error("Invalid state for operation: task {task_id} is in state {state:?}")]
    InvalidState { task_id: String, state: TaskState },
}

/// Result type for coordination operations
pub type CoordinationResult<T> = Result<T, CoordinationError>;

/// A dependency edge from one task to another
///
/// Represents a prerequisite relationship where the dependent task
/// cannot start until the prerequisite task has completed and produced
/// the required artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// ID of the task that depends on another
    pub dependent: String,
    /// ID of the task that must complete first
    pub prerequisite: String,
    /// Optional specific artifact IDs required from the prerequisite
    /// If None, any completion is sufficient
    pub required_artifacts: Option<Vec<String>>,
    /// When this dependency was established
    pub created_at: DateTime<Utc>,
}

/// A node in the task graph representing a task and its relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// The task itself
    pub task: Task,
    /// IDs of tasks this task depends on (prerequisites)
    pub prerequisites: HashSet<String>,
    /// IDs of tasks that depend on this task (dependents)
    pub dependents: HashSet<String>,
    /// When this node was added to the graph
    pub added_at: DateTime<Utc>,
}

/// A directed acyclic graph of tasks with dependency relationships
///
/// Manages task coordination through explicit dependency edges.
/// Supports prerequisite checking, join semantics, cancellation
/// propagation, and termination detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    /// All task nodes in the graph, indexed by task ID
    nodes: HashMap<String, TaskNode>,
    /// All dependency edges in the graph
    edges: Vec<DependencyEdge>,
    /// Timestamp when the graph was created
    created_at: DateTime<Utc>,
}

impl TaskGraph {
    /// Create a new empty task graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a task to the graph
    ///
    /// # Errors
    ///
    /// Returns an error if the task already exists in the graph.
    pub fn add_task(&mut self, task: Task) -> CoordinationResult<()> {
        if self.nodes.contains_key(&task.id) {
            return Err(CoordinationError::TaskAlreadyExists {
                task_id: task.id.clone(),
            });
        }

        let node = TaskNode {
            task,
            prerequisites: HashSet::new(),
            dependents: HashSet::new(),
            added_at: Utc::now(),
        };

        self.nodes.insert(node.task.id.clone(), node);
        Ok(())
    }

    /// Add a dependency edge between two tasks
    ///
    /// The dependent task cannot start until the prerequisite task completes.
    ///
    /// # Arguments
    ///
    /// * `dependent_id` - ID of the task that depends on another
    /// * `prerequisite_id` - ID of the task that must complete first
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either task is not found
    /// - The dependency would create a cycle
    /// - The dependency already exists
    pub fn add_dependency(
        &mut self,
        dependent_id: &str,
        prerequisite_id: &str,
    ) -> CoordinationResult<()> {
        self.add_dependency_with_artifacts(dependent_id, prerequisite_id, None)
    }

    /// Add a dependency edge with specific artifact requirements
    ///
    /// # Arguments
    ///
    /// * `dependent_id` - ID of the task that depends on another
    /// * `prerequisite_id` - ID of the task that must complete first
    /// * `required_artifacts` - Specific artifact IDs required from the prerequisite
    pub fn add_dependency_with_artifacts(
        &mut self,
        dependent_id: &str,
        prerequisite_id: &str,
        required_artifacts: Option<Vec<String>>,
    ) -> CoordinationResult<()> {
        // Validate both tasks exist
        if !self.nodes.contains_key(dependent_id) {
            return Err(CoordinationError::TaskNotFound {
                task_id: dependent_id.to_string(),
            });
        }
        if !self.nodes.contains_key(prerequisite_id) {
            return Err(CoordinationError::TaskNotFound {
                task_id: prerequisite_id.to_string(),
            });
        }

        // Check if dependency already exists
        if self
            .edges
            .iter()
            .any(|e| e.dependent == dependent_id && e.prerequisite == prerequisite_id)
        {
            return Err(CoordinationError::DependencyAlreadyExists {
                from: prerequisite_id.to_string(),
                to: dependent_id.to_string(),
            });
        }

        // Check for cycles by doing a DFS from the prerequisite
        if self.would_create_cycle(dependent_id, prerequisite_id) {
            return Err(CoordinationError::CyclicDependency {
                from: prerequisite_id.to_string(),
                to: dependent_id.to_string(),
            });
        }

        // Add the edge
        let edge = DependencyEdge {
            dependent: dependent_id.to_string(),
            prerequisite: prerequisite_id.to_string(),
            required_artifacts,
            created_at: Utc::now(),
        };
        self.edges.push(edge);

        // Update node relationships
        if let Some(dependent_node) = self.nodes.get_mut(dependent_id) {
            dependent_node
                .prerequisites
                .insert(prerequisite_id.to_string());
        }
        if let Some(prerequisite_node) = self.nodes.get_mut(prerequisite_id) {
            prerequisite_node
                .dependents
                .insert(dependent_id.to_string());
        }

        Ok(())
    }

    /// Check if adding a dependency would create a cycle
    fn would_create_cycle(&self, from: &str, to: &str) -> bool {
        // If we add edge from -> to, we need to check if there's already
        // a path from to -> from. If so, adding this edge would create a cycle.
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(to);

        while let Some(current) = stack.pop_front() {
            if current == from {
                return true; // Found a path back to the starting node
            }

            if !visited.insert(current) {
                continue; // Already visited this node
            }

            // Add all nodes that current depends on
            if let Some(node) = self.nodes.get(current) {
                for prereq in &node.prerequisites {
                    stack.push_back(prereq.as_str());
                }
            }
        }

        false
    }

    /// Get a task from the graph
    pub fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.nodes.get(task_id).map(|node| &node.task)
    }

    /// Get a mutable reference to a task in the graph
    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.nodes.get_mut(task_id).map(|node| &mut node.task)
    }

    /// Update a task in the graph
    ///
    /// # Errors
    ///
    /// Returns an error if the task is not found.
    pub fn update_task(&mut self, task: Task) -> CoordinationResult<()> {
        let task_id = task.id.clone();
        if let Some(node) = self.nodes.get_mut(&task_id) {
            node.task = task;
            Ok(())
        } else {
            Err(CoordinationError::TaskNotFound { task_id })
        }
    }

    /// Check if all prerequisites for a task are met
    ///
    /// A prerequisite is met if:
    /// - The prerequisite task is in a terminal state (Completed, Failed, Canceled)
    /// - If specific artifacts are required, they are available
    ///
    /// # Errors
    ///
    /// Returns an error if the task is not found.
    pub fn prerequisites_met(&self, task_id: &str) -> CoordinationResult<bool> {
        let node = self
            .nodes
            .get(task_id)
            .ok_or_else(|| CoordinationError::TaskNotFound {
                task_id: task_id.to_string(),
            })?;

        // Check each prerequisite
        for prereq_id in &node.prerequisites {
            let prereq_node =
                self.nodes
                    .get(prereq_id)
                    .ok_or_else(|| CoordinationError::TaskNotFound {
                        task_id: prereq_id.clone(),
                    })?;

            // Check if prerequisite is complete
            if prereq_node.task.status.state != TaskState::Completed {
                return Ok(false);
            }

            // Check if specific artifacts are required
            if let Some(edge) = self
                .edges
                .iter()
                .find(|e| e.dependent == task_id && e.prerequisite == prereq_id.as_str())
            {
                if let Some(required_artifacts) = &edge.required_artifacts {
                    // Check if all required artifacts are present
                    let available_artifacts = prereq_node
                        .task
                        .artifacts
                        .as_ref()
                        .map(|artifacts| {
                            artifacts
                                .iter()
                                .map(|a| a.artifact_id.as_str())
                                .collect::<HashSet<_>>()
                        })
                        .unwrap_or_default();

                    for required in required_artifacts {
                        if !available_artifacts.contains(required.as_str()) {
                            return Ok(false);
                        }
                    }
                }
            }
        }

        Ok(true)
    }

    /// Get the list of prerequisite task IDs that are not yet met
    ///
    /// # Errors
    ///
    /// Returns an error if the task is not found.
    pub fn unmet_prerequisites(&self, task_id: &str) -> CoordinationResult<Vec<String>> {
        let node = self
            .nodes
            .get(task_id)
            .ok_or_else(|| CoordinationError::TaskNotFound {
                task_id: task_id.to_string(),
            })?;

        let mut unmet = Vec::new();

        for prereq_id in &node.prerequisites {
            let prereq_node =
                self.nodes
                    .get(prereq_id)
                    .ok_or_else(|| CoordinationError::TaskNotFound {
                        task_id: prereq_id.clone(),
                    })?;

            if prereq_node.task.status.state != TaskState::Completed {
                unmet.push(prereq_id.clone());
            }
        }

        Ok(unmet)
    }

    /// Get all tasks that are ready to execute
    ///
    /// A task is ready if:
    /// - All prerequisites are met
    /// - The task is in Submitted state
    pub fn ready_tasks(&self) -> Vec<String> {
        self.nodes
            .values()
            .filter(|node| {
                node.task.status.state == TaskState::Submitted
                    && self.prerequisites_met(&node.task.id).unwrap_or(false)
            })
            .map(|node| node.task.id.clone())
            .collect()
    }

    /// Propagate cancellation from a task to all its dependents
    ///
    /// When a task is canceled, all tasks that depend on it (directly or
    /// indirectly) should also be canceled.
    ///
    /// # Returns
    ///
    /// A list of task IDs that were canceled as a result of the propagation.
    ///
    /// # Errors
    ///
    /// Returns an error if the root task is not found.
    pub fn propagate_cancellation(&mut self, task_id: &str) -> CoordinationResult<Vec<String>> {
        // Validate task exists
        if !self.nodes.contains_key(task_id) {
            return Err(CoordinationError::TaskNotFound {
                task_id: task_id.to_string(),
            });
        }

        let mut canceled = Vec::new();
        let mut to_cancel = VecDeque::new();
        to_cancel.push_back(task_id.to_string());

        while let Some(current_id) = to_cancel.pop_front() {
            // Get the node (skip if already processed)
            let Some(node) = self.nodes.get(&current_id) else {
                continue;
            };

            // Only cancel if not already in a terminal state
            if matches!(
                node.task.status.state,
                TaskState::Submitted | TaskState::Working | TaskState::InputRequired
            ) {
                // Mark as canceled
                if let Some(node) = self.nodes.get_mut(&current_id) {
                    node.task.status.state = TaskState::Canceled;
                    node.task.status.timestamp = Some(Utc::now());
                    canceled.push(current_id.clone());
                }

                // Add all dependents to the cancellation queue
                if let Some(node) = self.nodes.get(&current_id) {
                    for dependent_id in &node.dependents {
                        to_cancel.push_back(dependent_id.clone());
                    }
                }
            }
        }

        Ok(canceled)
    }

    /// Check if the entire graph has terminated
    ///
    /// The graph is considered terminated when all tasks are in terminal states
    /// (Completed, Failed, Canceled, Rejected, or Unknown).
    pub fn is_terminated(&self) -> bool {
        self.nodes.values().all(|node| {
            matches!(
                node.task.status.state,
                TaskState::Completed
                    | TaskState::Failed
                    | TaskState::Canceled
                    | TaskState::Rejected
                    | TaskState::Unknown
            )
        })
    }

    /// Get all terminal tasks (tasks in a terminal state)
    pub fn terminal_tasks(&self) -> Vec<String> {
        self.nodes
            .values()
            .filter(|node| {
                matches!(
                    node.task.status.state,
                    TaskState::Completed
                        | TaskState::Failed
                        | TaskState::Canceled
                        | TaskState::Rejected
                        | TaskState::Unknown
                )
            })
            .map(|node| node.task.id.clone())
            .collect()
    }

    /// Get all root tasks (tasks with no prerequisites)
    pub fn root_tasks(&self) -> Vec<String> {
        self.nodes
            .values()
            .filter(|node| node.prerequisites.is_empty())
            .map(|node| node.task.id.clone())
            .collect()
    }

    /// Get all leaf tasks (tasks with no dependents)
    pub fn leaf_tasks(&self) -> Vec<String> {
        self.nodes
            .values()
            .filter(|node| node.dependents.is_empty())
            .map(|node| node.task.id.clone())
            .collect()
    }

    /// Get the number of tasks in the graph
    pub fn task_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of dependency edges in the graph
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get all task IDs in the graph
    pub fn task_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Get all dependencies for a task
    pub fn dependencies(&self, task_id: &str) -> Option<&HashSet<String>> {
        self.nodes.get(task_id).map(|node| &node.prerequisites)
    }

    /// Get all dependents for a task
    pub fn dependents(&self, task_id: &str) -> Option<&HashSet<String>> {
        self.nodes.get(task_id).map(|node| &node.dependents)
    }

    /// Perform a topological sort of the task graph
    ///
    /// Returns tasks in an order such that all prerequisites come before
    /// their dependents. This is useful for scheduling and visualization.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph contains a cycle (which should be
    /// prevented by add_dependency).
    pub fn topological_sort(&self) -> CoordinationResult<Vec<String>> {
        let mut sorted = Vec::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();

        // Calculate in-degree for each node
        for (task_id, node) in &self.nodes {
            in_degree.insert(task_id.clone(), node.prerequisites.len());
            if node.prerequisites.is_empty() {
                queue.push_back(task_id.clone());
            }
        }

        // Process nodes with no prerequisites
        while let Some(task_id) = queue.pop_front() {
            sorted.push(task_id.clone());

            // Reduce in-degree for dependents
            if let Some(node) = self.nodes.get(&task_id) {
                for dependent_id in &node.dependents {
                    if let Some(degree) = in_degree.get_mut(dependent_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent_id.clone());
                        }
                    }
                }
            }
        }

        // If we didn't process all nodes, there's a cycle
        if sorted.len() != self.nodes.len() {
            // This should not happen if add_dependency is working correctly
            Err(CoordinationError::CyclicDependency {
                from: "unknown".to_string(),
                to: "unknown".to_string(),
            })
        } else {
            Ok(sorted)
        }
    }

    /// Get artifacts from a prerequisite task
    ///
    /// # Errors
    ///
    /// Returns an error if the task is not found.
    pub fn get_prerequisite_artifacts(
        &self,
        _task_id: &str,
        prerequisite_id: &str,
    ) -> CoordinationResult<Vec<Artifact>> {
        let prereq_node =
            self.nodes
                .get(prerequisite_id)
                .ok_or_else(|| CoordinationError::TaskNotFound {
                    task_id: prerequisite_id.to_string(),
                })?;

        Ok(prereq_node.task.artifacts.clone().unwrap_or_default())
    }

    /// Get all artifacts from all prerequisites of a task
    ///
    /// # Errors
    ///
    /// Returns an error if the task is not found.
    pub fn get_all_prerequisite_artifacts(
        &self,
        task_id: &str,
    ) -> CoordinationResult<Vec<Artifact>> {
        let node = self
            .nodes
            .get(task_id)
            .ok_or_else(|| CoordinationError::TaskNotFound {
                task_id: task_id.to_string(),
            })?;

        let mut all_artifacts = Vec::new();

        for prereq_id in &node.prerequisites {
            let prereq_node =
                self.nodes
                    .get(prereq_id)
                    .ok_or_else(|| CoordinationError::TaskNotFound {
                        task_id: prereq_id.clone(),
                    })?;

            if let Some(artifacts) = &prereq_node.task.artifacts {
                all_artifacts.extend(artifacts.clone());
            }
        }

        Ok(all_artifacts)
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task(id: &str, state: TaskState) -> Task {
        let mut task = Task::new(id.to_string(), "ctx-1".to_string());
        task.status.state = state;
        task
    }

    #[test]
    fn test_new_graph() {
        let graph = TaskGraph::new();
        assert_eq!(graph.task_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.is_terminated());
    }

    #[test]
    fn test_add_task() {
        let mut graph = TaskGraph::new();
        let task = create_test_task("task-1", TaskState::Submitted);

        assert!(graph.add_task(task.clone()).is_ok());
        assert_eq!(graph.task_count(), 1);
        assert!(graph.get_task("task-1").is_some());

        // Adding duplicate should fail
        assert!(graph.add_task(task).is_err());
    }

    #[test]
    fn test_add_dependency() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();

        assert!(graph.add_dependency("task-b", "task-a").is_ok());
        assert_eq!(graph.edge_count(), 1);

        // Adding duplicate dependency should fail
        assert!(graph.add_dependency("task-b", "task-a").is_err());
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("task-b", "task-a").unwrap();
        graph.add_dependency("task-c", "task-b").unwrap();

        // Creating a cycle should fail
        assert!(graph.add_dependency("task-a", "task-c").is_err());
    }

    #[test]
    fn test_prerequisites_met() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("task-b", "task-a").unwrap();

        // Prerequisites should be met since task-a is completed
        assert!(graph.prerequisites_met("task-b").unwrap());

        // Change task-a to working
        graph.get_task_mut("task-a").unwrap().status.state = TaskState::Working;

        // Prerequisites should not be met
        assert!(!graph.prerequisites_met("task-b").unwrap());
    }

    #[test]
    fn test_join_semantics() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Submitted))
            .unwrap();

        // Task C depends on both A and B (join)
        graph.add_dependency("task-c", "task-a").unwrap();
        graph.add_dependency("task-c", "task-b").unwrap();

        // Prerequisites should be met since both are completed
        assert!(graph.prerequisites_met("task-c").unwrap());

        // Change task-b to working
        graph.get_task_mut("task-b").unwrap().status.state = TaskState::Working;

        // Prerequisites should not be met (join requires all)
        assert!(!graph.prerequisites_met("task-c").unwrap());
    }

    #[test]
    fn test_ready_tasks() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("task-b", "task-a").unwrap();
        graph.add_dependency("task-c", "task-b").unwrap();

        // Only task-a should be ready (no prerequisites)
        let ready = graph.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&"task-a".to_string()));

        // Complete task-a
        graph.get_task_mut("task-a").unwrap().status.state = TaskState::Completed;

        // Now task-b should be ready
        let ready = graph.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&"task-b".to_string()));
    }

    #[test]
    fn test_cancellation_propagation() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Working))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("task-b", "task-a").unwrap();
        graph.add_dependency("task-c", "task-b").unwrap();

        // Cancel task-a
        let canceled = graph.propagate_cancellation("task-a").unwrap();

        // All three tasks should be canceled
        assert_eq!(canceled.len(), 3);
        assert!(canceled.contains(&"task-a".to_string()));
        assert!(canceled.contains(&"task-b".to_string()));
        assert!(canceled.contains(&"task-c".to_string()));

        assert_eq!(
            graph.get_task("task-a").unwrap().status.state,
            TaskState::Canceled
        );
        assert_eq!(
            graph.get_task("task-b").unwrap().status.state,
            TaskState::Canceled
        );
        assert_eq!(
            graph.get_task("task-c").unwrap().status.state,
            TaskState::Canceled
        );
    }

    #[test]
    fn test_termination_detection() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Failed))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Canceled))
            .unwrap();

        assert!(graph.is_terminated());

        // Add a working task
        graph
            .add_task(create_test_task("task-d", TaskState::Working))
            .unwrap();

        assert!(!graph.is_terminated());
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("task-b", "task-a").unwrap();
        graph.add_dependency("task-c", "task-b").unwrap();

        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted.len(), 3);

        // task-a should come before task-b
        let pos_a = sorted.iter().position(|id| id == "task-a").unwrap();
        let pos_b = sorted.iter().position(|id| id == "task-b").unwrap();
        let pos_c = sorted.iter().position(|id| id == "task-c").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_root_and_leaf_tasks() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("task-c", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("task-b", "task-a").unwrap();
        graph.add_dependency("task-c", "task-b").unwrap();

        let roots = graph.root_tasks();
        assert_eq!(roots.len(), 1);
        assert!(roots.contains(&"task-a".to_string()));

        let leaves = graph.leaf_tasks();
        assert_eq!(leaves.len(), 1);
        assert!(leaves.contains(&"task-c".to_string()));
    }

    #[test]
    fn test_artifact_requirements() {
        let mut graph = TaskGraph::new();

        let mut task_a = create_test_task("task-a", TaskState::Completed);
        task_a.artifacts = Some(vec![Artifact {
            artifact_id: "art-1".to_string(),
            name: Some("result.json".to_string()),
            description: None,
            parts: vec![],
            metadata: None,
            extensions: None,
        }]);

        graph.add_task(task_a).unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();

        // Task B requires specific artifact from Task A
        graph
            .add_dependency_with_artifacts("task-b", "task-a", Some(vec!["art-1".to_string()]))
            .unwrap();

        // Prerequisites should be met
        assert!(graph.prerequisites_met("task-b").unwrap());

        // Change required artifact to something not available
        if let Some(edge) = graph.edges.iter_mut().find(|e| e.dependent == "task-b") {
            edge.required_artifacts = Some(vec!["art-2".to_string()]);
        }

        // Prerequisites should not be met
        assert!(!graph.prerequisites_met("task-b").unwrap());
    }

    #[test]
    fn test_get_prerequisite_artifacts() {
        let mut graph = TaskGraph::new();

        let mut task_a = create_test_task("task-a", TaskState::Completed);
        let artifact = Artifact {
            artifact_id: "art-1".to_string(),
            name: Some("result.json".to_string()),
            description: None,
            parts: vec![],
            metadata: None,
            extensions: None,
        };
        task_a.artifacts = Some(vec![artifact.clone()]);

        graph.add_task(task_a).unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph.add_dependency("task-b", "task-a").unwrap();

        let artifacts = graph
            .get_prerequisite_artifacts("task-b", "task-a")
            .unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].artifact_id, "art-1");
    }
}
