//! Workflow pattern completeness checker based on the Workflow Patterns Initiative.
//!
//! This module provides:
//! - Enumeration of all 43 workflow patterns
//! - Graph-based workflow modeling using petgraph
//! - Pattern detection via graph pattern matching
//! - Completeness validation
//! - Gap analysis showing missing patterns and unreachable states
//! - Export analysis for detecting states requiring human intervention

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::Dfs;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Comprehensive enumeration of all 43 workflow patterns from the Workflow Patterns Initiative
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowPattern {
    // Basic Control-Flow Patterns (1-5)
    /// Sequential execution of activities
    Sequence,
    /// Divergence of a branch into multiple parallel branches
    ParallelSplit,
    /// Convergence of multiple parallel branches into a single branch
    Synchronization,
    /// Choice between multiple branches based on a decision
    ExclusiveChoice,
    /// Convergence of exclusive branches without synchronization
    SimpleMerge,

    // Advanced Branching and Synchronization (6-20)
    /// Selection of multiple branches based on conditions
    MultiChoice,
    /// Convergence of branches activated by multi-choice
    StructuredSynchronizingMerge,
    /// Convergence point for multiple branches running in parallel
    MultiMerge,
    /// First of multiple parallel branches triggers continuation
    StructuredDiscriminator,
    /// Thread waits for first of multiple incoming branches
    BlockingDiscriminator,
    /// First incoming branch cancels other branches
    CancellingDiscriminator,
    /// Waits for N out of M incoming branches (structured)
    StructuredPartialJoin,
    /// Blocks until N out of M branches complete
    BlockingPartialJoin,
    /// N branches cancel remaining branches
    CancellingPartialJoin,
    /// General AND-join with multiple instances
    GeneralizedAndJoin,
    /// Synchronizing merge local to a subprocess
    LocalSynchronizingMerge,
    /// General synchronizing merge for arbitrary cycles
    GeneralSynchronizingMerge,
    /// Merges threads of execution
    ThreadMerge,
    /// Splits into multiple threads
    ThreadSplit,
    /// Explicit termination of workflow instance
    ExplicitTermination,

    // Multiple Instance Patterns (21-27)
    /// Multiple instances without synchronization
    MultipleInstancesWithoutSynchronization,
    /// Multiple instances with design-time cardinality
    MultipleInstancesWithAPrioriDesignTime,
    /// Multiple instances with runtime cardinality
    MultipleInstancesWithAPrioriRunTime,
    /// Multiple instances with dynamic cardinality
    MultipleInstancesWithoutAPrioriRunTime,
    /// Static partial join for multiple instances
    StaticPartialJoinForMultipleInstances,
    /// Cancelling partial join for multiple instances
    CancellingPartialJoinForMultipleInstances,
    /// Dynamic partial join for multiple instances
    DynamicPartialJoinForMultipleInstances,

    // State-Based Patterns (28-30)
    /// Activity enabled when milestone is achieved
    Milestone,
    /// Mutual exclusion for resource access
    CriticalSection,
    /// Interleaved execution without overlap
    InterleavedParallelRouting,

    // Cancellation and Force Completion Patterns (31-35)
    /// Cancel a single task instance
    CancelTask,
    /// Cancel entire workflow case
    CancelCase,
    /// Cancel a region of the workflow
    CancelRegion,
    /// Cancel multiple instance activity
    CancelMultipleInstanceActivity,
    /// Force completion of multiple instance activity
    CompleteMultipleInstanceActivity,

    // Iteration Patterns (36-37)
    /// Arbitrary cycles in workflow
    ArbitraryCycles,
    /// Structured loop construct
    StructuredLoop,

    // Termination Patterns (38-39)
    /// Workflow terminates when no more work
    ImplicitTermination,
    /// Workflow terminates explicitly
    ExplicitTerminationPattern,

    // Trigger Patterns (40-41)
    /// Trigger available for single execution
    TransientTrigger,
    /// Trigger queued for later execution
    PersistentTrigger,

    // Special Patterns (42-43)
    /// Choice made by environment, not workflow
    DeferredChoice,
    /// Interleaved execution of activity group
    InterleavedRouting,
}

impl WorkflowPattern {
    /// Returns all 43 workflow patterns
    pub fn all() -> Vec<Self> {
        vec![
            Self::Sequence,
            Self::ParallelSplit,
            Self::Synchronization,
            Self::ExclusiveChoice,
            Self::SimpleMerge,
            Self::MultiChoice,
            Self::StructuredSynchronizingMerge,
            Self::MultiMerge,
            Self::StructuredDiscriminator,
            Self::BlockingDiscriminator,
            Self::CancellingDiscriminator,
            Self::StructuredPartialJoin,
            Self::BlockingPartialJoin,
            Self::CancellingPartialJoin,
            Self::GeneralizedAndJoin,
            Self::LocalSynchronizingMerge,
            Self::GeneralSynchronizingMerge,
            Self::ThreadMerge,
            Self::ThreadSplit,
            Self::ExplicitTermination,
            Self::MultipleInstancesWithoutSynchronization,
            Self::MultipleInstancesWithAPrioriDesignTime,
            Self::MultipleInstancesWithAPrioriRunTime,
            Self::MultipleInstancesWithoutAPrioriRunTime,
            Self::StaticPartialJoinForMultipleInstances,
            Self::CancellingPartialJoinForMultipleInstances,
            Self::DynamicPartialJoinForMultipleInstances,
            Self::Milestone,
            Self::CriticalSection,
            Self::InterleavedParallelRouting,
            Self::CancelTask,
            Self::CancelCase,
            Self::CancelRegion,
            Self::CancelMultipleInstanceActivity,
            Self::CompleteMultipleInstanceActivity,
            Self::ArbitraryCycles,
            Self::StructuredLoop,
            Self::ImplicitTermination,
            Self::ExplicitTerminationPattern,
            Self::TransientTrigger,
            Self::PersistentTrigger,
            Self::DeferredChoice,
            Self::InterleavedRouting,
        ]
    }

    /// Returns the category of the pattern
    pub fn category(&self) -> PatternCategory {
        match self {
            Self::Sequence
            | Self::ParallelSplit
            | Self::Synchronization
            | Self::ExclusiveChoice
            | Self::SimpleMerge => PatternCategory::BasicControlFlow,

            Self::MultiChoice
            | Self::StructuredSynchronizingMerge
            | Self::MultiMerge
            | Self::StructuredDiscriminator
            | Self::BlockingDiscriminator
            | Self::CancellingDiscriminator
            | Self::StructuredPartialJoin
            | Self::BlockingPartialJoin
            | Self::CancellingPartialJoin
            | Self::GeneralizedAndJoin
            | Self::LocalSynchronizingMerge
            | Self::GeneralSynchronizingMerge
            | Self::ThreadMerge
            | Self::ThreadSplit
            | Self::ExplicitTermination => PatternCategory::AdvancedBranchingAndSynchronization,

            Self::MultipleInstancesWithoutSynchronization
            | Self::MultipleInstancesWithAPrioriDesignTime
            | Self::MultipleInstancesWithAPrioriRunTime
            | Self::MultipleInstancesWithoutAPrioriRunTime
            | Self::StaticPartialJoinForMultipleInstances
            | Self::CancellingPartialJoinForMultipleInstances
            | Self::DynamicPartialJoinForMultipleInstances => PatternCategory::MultipleInstance,

            Self::Milestone | Self::CriticalSection | Self::InterleavedParallelRouting => {
                PatternCategory::StateBased
            }

            Self::CancelTask
            | Self::CancelCase
            | Self::CancelRegion
            | Self::CancelMultipleInstanceActivity
            | Self::CompleteMultipleInstanceActivity => PatternCategory::CancellationAndCompletion,

            Self::ArbitraryCycles | Self::StructuredLoop => PatternCategory::Iteration,

            Self::ImplicitTermination | Self::ExplicitTerminationPattern => {
                PatternCategory::Termination
            }

            Self::TransientTrigger | Self::PersistentTrigger => PatternCategory::Trigger,

            Self::DeferredChoice | Self::InterleavedRouting => PatternCategory::Special,
        }
    }
}

/// Category classification for workflow patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternCategory {
    BasicControlFlow,
    AdvancedBranchingAndSynchronization,
    MultipleInstance,
    StateBased,
    CancellationAndCompletion,
    Iteration,
    Termination,
    Trigger,
    Special,
}

/// Workflow state node in the graph
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowState {
    /// Unique identifier for the state
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// State type
    pub state_type: StateType,
    /// Whether this state requires human intervention (export)
    pub requires_export: bool,
    /// Patterns used at this state
    pub patterns: HashSet<WorkflowPattern>,
}

/// Type of workflow state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StateType {
    /// Starting state
    Start,
    /// Ending state
    End,
    /// Normal processing state
    Process,
    /// Decision point
    Decision,
    /// Fork for parallel execution
    Fork,
    /// Join for parallel execution
    Join,
    /// State requiring human intervention
    HumanTask,
    /// Subprocess
    Subprocess,
}

/// Transition between workflow states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTransition {
    /// Condition for this transition
    pub condition: Option<String>,
    /// Patterns used in this transition
    pub patterns: HashSet<WorkflowPattern>,
}

/// Workflow graph representation using petgraph
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowGraph {
    /// Directed graph of states and transitions
    #[serde(skip)]
    graph: DiGraph<WorkflowState, WorkflowTransition>,
    /// Mapping from state IDs to node indices
    state_index: HashMap<String, NodeIndex>,
    /// Start state
    pub start_state: Option<String>,
    /// End states
    pub end_states: Vec<String>,
}

impl WorkflowGraph {
    /// Creates a new empty workflow graph
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            state_index: HashMap::new(),
            start_state: None,
            end_states: Vec::new(),
        }
    }

    /// Adds a state to the workflow
    pub fn add_state(&mut self, state: WorkflowState) -> Result<NodeIndex, WorkflowError> {
        if self.state_index.contains_key(&state.id) {
            return Err(WorkflowError::DuplicateState(state.id));
        }

        let id = state.id.clone();
        let state_type = state.state_type;
        let index = self.graph.add_node(state);
        self.state_index.insert(id.clone(), index);

        match state_type {
            StateType::Start => self.start_state = Some(id),
            StateType::End => self.end_states.push(id),
            _ => {}
        }

        Ok(index)
    }

    /// Adds a transition between states
    pub fn add_transition(
        &mut self,
        from: &str,
        to: &str,
        transition: WorkflowTransition,
    ) -> Result<(), WorkflowError> {
        let from_idx = self
            .state_index
            .get(from)
            .ok_or_else(|| WorkflowError::StateNotFound(from.to_string()))?;
        let to_idx = self
            .state_index
            .get(to)
            .ok_or_else(|| WorkflowError::StateNotFound(to.to_string()))?;

        self.graph.add_edge(*from_idx, *to_idx, transition);
        Ok(())
    }

    /// Returns all states in the workflow
    pub fn states(&self) -> Vec<&WorkflowState> {
        self.graph.node_weights().collect()
    }

    /// Returns a specific state by ID
    pub fn get_state(&self, id: &str) -> Option<&WorkflowState> {
        self.state_index
            .get(id)
            .and_then(|idx| self.graph.node_weight(*idx))
    }

    /// Detects all patterns used in the workflow
    pub fn detect_patterns(&self) -> HashSet<WorkflowPattern> {
        let mut patterns = HashSet::new();

        // Collect patterns from states
        for state in self.graph.node_weights() {
            patterns.extend(&state.patterns);
        }

        // Collect patterns from transitions
        for edge in self.graph.edge_weights() {
            patterns.extend(&edge.patterns);
        }

        // Detect structural patterns
        patterns.extend(self.detect_structural_patterns());

        patterns
    }

    /// Detects structural patterns based on graph topology
    fn detect_structural_patterns(&self) -> HashSet<WorkflowPattern> {
        let mut patterns = HashSet::new();

        for node_idx in self.graph.node_indices() {
            let incoming = self
                .graph
                .edges_directed(node_idx, Direction::Incoming)
                .count();
            let outgoing = self
                .graph
                .edges_directed(node_idx, Direction::Outgoing)
                .count();

            // Detect basic patterns
            if incoming == 1 && outgoing == 1 {
                patterns.insert(WorkflowPattern::Sequence);
            }
            if outgoing > 1 {
                let state = self.graph.node_weight(node_idx).unwrap();
                match state.state_type {
                    StateType::Fork => patterns.insert(WorkflowPattern::ParallelSplit),
                    StateType::Decision => patterns.insert(WorkflowPattern::ExclusiveChoice),
                    _ => false,
                };
            }
            if incoming > 1 {
                let state = self.graph.node_weight(node_idx).unwrap();
                match state.state_type {
                    StateType::Join => patterns.insert(WorkflowPattern::Synchronization),
                    _ => {
                        patterns.insert(WorkflowPattern::SimpleMerge);
                        false
                    }
                };
            }
        }

        // Detect cycles
        if self.has_cycles() {
            patterns.insert(WorkflowPattern::ArbitraryCycles);
        }

        // Detect termination patterns
        if self.end_states.is_empty() {
            patterns.insert(WorkflowPattern::ImplicitTermination);
        } else {
            patterns.insert(WorkflowPattern::ExplicitTerminationPattern);
        }

        patterns
    }

    /// Checks if the graph contains cycles
    fn has_cycles(&self) -> bool {
        petgraph::algo::is_cyclic_directed(&self.graph)
    }

    /// Finds all reachable states from a given state
    pub fn reachable_from(&self, state_id: &str) -> Result<HashSet<String>, WorkflowError> {
        let start_idx = self
            .state_index
            .get(state_id)
            .ok_or_else(|| WorkflowError::StateNotFound(state_id.to_string()))?;

        let mut reachable = HashSet::new();
        let mut dfs = Dfs::new(&self.graph, *start_idx);

        while let Some(idx) = dfs.next(&self.graph) {
            if let Some(state) = self.graph.node_weight(idx) {
                reachable.insert(state.id.clone());
            }
        }

        Ok(reachable)
    }

    /// Finds all unreachable states from the start state
    pub fn unreachable_states(&self) -> Result<HashSet<String>, WorkflowError> {
        if let Some(start) = &self.start_state {
            let reachable = self.reachable_from(start)?;
            let all_states: HashSet<String> =
                self.graph.node_weights().map(|s| s.id.clone()).collect();
            Ok(all_states.difference(&reachable).cloned().collect())
        } else {
            // No start state, all states are unreachable
            Ok(self.graph.node_weights().map(|s| s.id.clone()).collect())
        }
    }

    /// Finds all states requiring export (human intervention)
    pub fn export_states(&self) -> Vec<&WorkflowState> {
        self.graph
            .node_weights()
            .filter(|s| s.requires_export || s.state_type == StateType::HumanTask)
            .collect()
    }

    /// Analyzes workflow for completeness
    pub fn analyze(&self) -> WorkflowAnalysis {
        let used_patterns = self.detect_patterns();
        let all_patterns: HashSet<_> = WorkflowPattern::all().into_iter().collect();
        let missing_patterns: HashSet<_> =
            all_patterns.difference(&used_patterns).copied().collect();

        let unreachable = self.unreachable_states().unwrap_or_default();
        let export_states: Vec<_> = self.export_states().iter().map(|s| s.id.clone()).collect();

        // Check for dead ends (non-end states with no outgoing edges)
        let mut dead_ends = Vec::new();
        for node_idx in self.graph.node_indices() {
            let state = self.graph.node_weight(node_idx).unwrap();
            if state.state_type != StateType::End
                && self
                    .graph
                    .edges_directed(node_idx, Direction::Outgoing)
                    .count()
                    == 0
            {
                dead_ends.push(state.id.clone());
            }
        }

        // Calculate coverage before moving used_patterns
        let pattern_coverage = (used_patterns.len() as f64) / (all_patterns.len() as f64);

        WorkflowAnalysis {
            total_states: self.graph.node_count(),
            total_transitions: self.graph.edge_count(),
            used_patterns: used_patterns.into_iter().collect(),
            missing_patterns: missing_patterns.into_iter().collect(),
            pattern_coverage,
            unreachable_states: unreachable.into_iter().collect(),
            export_states,
            dead_ends,
            has_cycles: self.has_cycles(),
        }
    }
}

impl Default for WorkflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Analysis results for a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAnalysis {
    /// Total number of states
    pub total_states: usize,
    /// Total number of transitions
    pub total_transitions: usize,
    /// Patterns used in the workflow
    pub used_patterns: Vec<WorkflowPattern>,
    /// Patterns not used in the workflow
    pub missing_patterns: Vec<WorkflowPattern>,
    /// Percentage of patterns covered
    pub pattern_coverage: f64,
    /// States that cannot be reached from start
    pub unreachable_states: Vec<String>,
    /// States requiring human intervention
    pub export_states: Vec<String>,
    /// Dead-end states (non-end states with no outgoing transitions)
    pub dead_ends: Vec<String>,
    /// Whether the workflow contains cycles
    pub has_cycles: bool,
}

impl WorkflowAnalysis {
    /// Returns whether the workflow is complete (covers all patterns)
    pub fn is_complete(&self) -> bool {
        self.missing_patterns.is_empty()
    }

    /// Returns whether the workflow is valid (no unreachable states or dead ends)
    pub fn is_valid(&self) -> bool {
        self.unreachable_states.is_empty() && self.dead_ends.is_empty()
    }

    /// Categorizes missing patterns by category
    pub fn missing_patterns_by_category(&self) -> HashMap<PatternCategory, Vec<WorkflowPattern>> {
        let mut result: HashMap<PatternCategory, Vec<WorkflowPattern>> = HashMap::new();
        for pattern in &self.missing_patterns {
            result.entry(pattern.category()).or_default().push(*pattern);
        }
        result
    }
}

/// Errors that can occur during workflow operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("Duplicate state: {0}")]
    DuplicateState(String),
    #[error("State not found: {0}")]
    StateNotFound(String),
    #[error("Invalid workflow: {0}")]
    InvalidWorkflow(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_all_patterns_enumerated() {
        let patterns = WorkflowPattern::all();
        assert_eq!(patterns.len(), 43, "Should have exactly 43 patterns");

        // Ensure no duplicates
        let unique: HashSet<_> = patterns.iter().collect();
        assert_eq!(unique.len(), 43, "All patterns should be unique");
    }

    #[test]
    fn test_pattern_categories() {
        for pattern in WorkflowPattern::all() {
            let category = pattern.category();
            // Just ensure every pattern has a category
            assert!(matches!(
                category,
                PatternCategory::BasicControlFlow
                    | PatternCategory::AdvancedBranchingAndSynchronization
                    | PatternCategory::MultipleInstance
                    | PatternCategory::StateBased
                    | PatternCategory::CancellationAndCompletion
                    | PatternCategory::Iteration
                    | PatternCategory::Termination
                    | PatternCategory::Trigger
                    | PatternCategory::Special
            ));
        }
    }

    #[test]
    fn test_empty_workflow() {
        let graph = WorkflowGraph::new();
        let analysis = graph.analyze();

        assert_eq!(analysis.total_states, 0);
        assert_eq!(analysis.total_transitions, 0);
        assert_eq!(analysis.pattern_coverage, 0.0);
        assert_eq!(analysis.missing_patterns.len(), 43);
    }

    #[test]
    fn test_simple_sequence_workflow() {
        let mut graph = WorkflowGraph::new();

        let start = WorkflowState {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Start,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let process = WorkflowState {
            id: "process".to_string(),
            name: "Process".to_string(),
            state_type: StateType::Process,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let end = WorkflowState {
            id: "end".to_string(),
            name: "End".to_string(),
            state_type: StateType::End,
            requires_export: false,
            patterns: HashSet::new(),
        };

        graph.add_state(start).unwrap();
        graph.add_state(process).unwrap();
        graph.add_state(end).unwrap();

        let transition = WorkflowTransition {
            condition: None,
            patterns: HashSet::new(),
        };

        graph
            .add_transition("start", "process", transition.clone())
            .unwrap();
        graph.add_transition("process", "end", transition).unwrap();

        let analysis = graph.analyze();

        assert_eq!(analysis.total_states, 3);
        assert_eq!(analysis.total_transitions, 2);
        assert!(analysis.used_patterns.contains(&WorkflowPattern::Sequence));
        assert!(analysis.unreachable_states.is_empty());
        assert!(analysis.dead_ends.is_empty());
    }

    #[test]
    fn test_parallel_split_workflow() {
        let mut graph = WorkflowGraph::new();

        let start = WorkflowState {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Start,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let fork = WorkflowState {
            id: "fork".to_string(),
            name: "Fork".to_string(),
            state_type: StateType::Fork,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let task1 = WorkflowState {
            id: "task1".to_string(),
            name: "Task 1".to_string(),
            state_type: StateType::Process,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let task2 = WorkflowState {
            id: "task2".to_string(),
            name: "Task 2".to_string(),
            state_type: StateType::Process,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let join = WorkflowState {
            id: "join".to_string(),
            name: "Join".to_string(),
            state_type: StateType::Join,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let end = WorkflowState {
            id: "end".to_string(),
            name: "End".to_string(),
            state_type: StateType::End,
            requires_export: false,
            patterns: HashSet::new(),
        };

        graph.add_state(start).unwrap();
        graph.add_state(fork).unwrap();
        graph.add_state(task1).unwrap();
        graph.add_state(task2).unwrap();
        graph.add_state(join).unwrap();
        graph.add_state(end).unwrap();

        let transition = WorkflowTransition {
            condition: None,
            patterns: HashSet::new(),
        };

        graph
            .add_transition("start", "fork", transition.clone())
            .unwrap();
        graph
            .add_transition("fork", "task1", transition.clone())
            .unwrap();
        graph
            .add_transition("fork", "task2", transition.clone())
            .unwrap();
        graph
            .add_transition("task1", "join", transition.clone())
            .unwrap();
        graph
            .add_transition("task2", "join", transition.clone())
            .unwrap();
        graph.add_transition("join", "end", transition).unwrap();

        let analysis = graph.analyze();

        assert!(
            analysis
                .used_patterns
                .contains(&WorkflowPattern::ParallelSplit)
        );
        assert!(
            analysis
                .used_patterns
                .contains(&WorkflowPattern::Synchronization)
        );
        assert!(analysis.unreachable_states.is_empty());
    }

    #[test]
    fn test_unreachable_state_detection() {
        let mut graph = WorkflowGraph::new();

        let start = WorkflowState {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Start,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let reachable = WorkflowState {
            id: "reachable".to_string(),
            name: "Reachable".to_string(),
            state_type: StateType::Process,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let unreachable = WorkflowState {
            id: "unreachable".to_string(),
            name: "Unreachable".to_string(),
            state_type: StateType::Process,
            requires_export: false,
            patterns: HashSet::new(),
        };

        graph.add_state(start).unwrap();
        graph.add_state(reachable).unwrap();
        graph.add_state(unreachable).unwrap();

        let transition = WorkflowTransition {
            condition: None,
            patterns: HashSet::new(),
        };

        graph
            .add_transition("start", "reachable", transition)
            .unwrap();

        let analysis = graph.analyze();

        assert_eq!(analysis.unreachable_states.len(), 1);
        assert!(
            analysis
                .unreachable_states
                .contains(&"unreachable".to_string())
        );
    }

    #[test]
    fn test_export_state_detection() {
        let mut graph = WorkflowGraph::new();

        let start = WorkflowState {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Start,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let human_task = WorkflowState {
            id: "human".to_string(),
            name: "Human Task".to_string(),
            state_type: StateType::HumanTask,
            requires_export: true,
            patterns: HashSet::new(),
        };

        let end = WorkflowState {
            id: "end".to_string(),
            name: "End".to_string(),
            state_type: StateType::End,
            requires_export: false,
            patterns: HashSet::new(),
        };

        graph.add_state(start).unwrap();
        graph.add_state(human_task).unwrap();
        graph.add_state(end).unwrap();

        let transition = WorkflowTransition {
            condition: None,
            patterns: HashSet::new(),
        };

        graph
            .add_transition("start", "human", transition.clone())
            .unwrap();
        graph.add_transition("human", "end", transition).unwrap();

        let analysis = graph.analyze();

        assert_eq!(analysis.export_states.len(), 1);
        assert!(analysis.export_states.contains(&"human".to_string()));
    }

    #[test]
    fn test_dead_end_detection() {
        let mut graph = WorkflowGraph::new();

        let start = WorkflowState {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Start,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let dead_end = WorkflowState {
            id: "dead".to_string(),
            name: "Dead End".to_string(),
            state_type: StateType::Process,
            requires_export: false,
            patterns: HashSet::new(),
        };

        graph.add_state(start).unwrap();
        graph.add_state(dead_end).unwrap();

        let transition = WorkflowTransition {
            condition: None,
            patterns: HashSet::new(),
        };

        graph.add_transition("start", "dead", transition).unwrap();

        let analysis = graph.analyze();

        assert_eq!(analysis.dead_ends.len(), 1);
        assert!(analysis.dead_ends.contains(&"dead".to_string()));
    }

    #[test]
    fn test_missing_patterns_cause_incomplete_workflow() {
        let mut graph = WorkflowGraph::new();

        // Create a minimal workflow with only Sequence pattern
        let start = WorkflowState {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Start,
            requires_export: false,
            patterns: HashSet::new(),
        };

        let end = WorkflowState {
            id: "end".to_string(),
            name: "End".to_string(),
            state_type: StateType::End,
            requires_export: false,
            patterns: HashSet::new(),
        };

        graph.add_state(start).unwrap();
        graph.add_state(end).unwrap();

        let transition = WorkflowTransition {
            condition: None,
            patterns: HashSet::new(),
        };

        graph.add_transition("start", "end", transition).unwrap();

        let analysis = graph.analyze();

        // Should not be complete because many patterns are missing
        assert!(!analysis.is_complete());
        assert!(analysis.missing_patterns.len() > 40);
        assert!(analysis.pattern_coverage < 0.1);
    }

    // Property-based tests
    proptest! {
        #[test]
        fn prop_workflow_analysis_totals_consistent(
            num_states in 1usize..20,
            num_transitions in 0usize..50
        ) {
            let mut graph = WorkflowGraph::new();

            // Add states
            for i in 0..num_states {
                let state = WorkflowState {
                    id: format!("state_{}", i),
                    name: format!("State {}", i),
                    state_type: if i == 0 {
                        StateType::Start
                    } else if i == num_states - 1 {
                        StateType::End
                    } else {
                        StateType::Process
                    },
                    requires_export: false,
                    patterns: HashSet::new(),
                };
                let _ = graph.add_state(state);
            }

            // Add transitions (with bounds checking)
            for i in 0..num_transitions.min(num_states.saturating_sub(1)) {
                let from = i;
                let to = (i + 1) % num_states;
                let transition = WorkflowTransition {
                    condition: None,
                    patterns: HashSet::new(),
                };
                let _ = graph.add_transition(
                    &format!("state_{}", from),
                    &format!("state_{}", to),
                    transition
                );
            }

            let analysis = graph.analyze();

            // Verify consistency
            prop_assert_eq!(analysis.total_states, num_states);
            prop_assert!(analysis.total_transitions <= num_transitions);
            prop_assert!(analysis.pattern_coverage >= 0.0);
            prop_assert!(analysis.pattern_coverage <= 1.0);
        }

        #[test]
        fn prop_missing_patterns_implies_incomplete(
            has_parallel_split: bool,
            has_exclusive_choice: bool,
            has_synchronization: bool
        ) {
            let mut graph = WorkflowGraph::new();

            let start = WorkflowState {
                id: "start".to_string(),
                name: "Start".to_string(),
                state_type: StateType::Start,
                requires_export: false,
                patterns: HashSet::new(),
            };

            graph.add_state(start).unwrap();

            // Add patterns conditionally
            if has_parallel_split {
                let fork = WorkflowState {
                    id: "fork".to_string(),
                    name: "Fork".to_string(),
                    state_type: StateType::Fork,
                    requires_export: false,
                    patterns: HashSet::new(),
                };
                graph.add_state(fork).unwrap();

                let transition = WorkflowTransition {
                    condition: None,
                    patterns: HashSet::new(),
                };
                graph.add_transition("start", "fork", transition).unwrap();
            }

            if has_exclusive_choice {
                let decision = WorkflowState {
                    id: "decision".to_string(),
                    name: "Decision".to_string(),
                    state_type: StateType::Decision,
                    requires_export: false,
                    patterns: HashSet::new(),
                };
                graph.add_state(decision).unwrap();
            }

            if has_synchronization {
                let join = WorkflowState {
                    id: "join".to_string(),
                    name: "Join".to_string(),
                    state_type: StateType::Join,
                    requires_export: false,
                    patterns: HashSet::new(),
                };
                graph.add_state(join).unwrap();
            }

            let analysis = graph.analyze();

            // If we don't have all 43 patterns, workflow should be incomplete
            if analysis.used_patterns.len() < 43 {
                prop_assert!(!analysis.is_complete());
                prop_assert!(!analysis.missing_patterns.is_empty());
            }
        }

        #[test]
        fn prop_export_states_require_human_intervention(
            num_human_tasks in 0usize..10
        ) {
            let mut graph = WorkflowGraph::new();

            let start = WorkflowState {
                id: "start".to_string(),
                name: "Start".to_string(),
                state_type: StateType::Start,
                requires_export: false,
                patterns: HashSet::new(),
            };
            graph.add_state(start).unwrap();

            // Add human task states
            for i in 0..num_human_tasks {
                let human_task = WorkflowState {
                    id: format!("human_{}", i),
                    name: format!("Human Task {}", i),
                    state_type: StateType::HumanTask,
                    requires_export: true,
                    patterns: HashSet::new(),
                };
                graph.add_state(human_task).unwrap();
            }

            let analysis = graph.analyze();

            // Number of export states should match human tasks
            prop_assert_eq!(analysis.export_states.len(), num_human_tasks);

            // All export states should be human tasks
            for export_state_id in &analysis.export_states {
                let state = graph.get_state(export_state_id).unwrap();
                prop_assert!(state.requires_export || state.state_type == StateType::HumanTask);
            }
        }

        #[test]
        fn prop_unreachable_states_not_reachable_from_start(
            num_reachable in 1usize..10,
            num_unreachable in 0usize..10
        ) {
            let mut graph = WorkflowGraph::new();

            // Create start and reachable states
            let start = WorkflowState {
                id: "start".to_string(),
                name: "Start".to_string(),
                state_type: StateType::Start,
                requires_export: false,
                patterns: HashSet::new(),
            };
            graph.add_state(start).unwrap();

            for i in 0..num_reachable {
                let state = WorkflowState {
                    id: format!("reachable_{}", i),
                    name: format!("Reachable {}", i),
                    state_type: StateType::Process,
                    requires_export: false,
                    patterns: HashSet::new(),
                };
                graph.add_state(state).unwrap();

                if i == 0 {
                    let transition = WorkflowTransition {
                        condition: None,
                        patterns: HashSet::new(),
                    };
                    graph.add_transition("start", &format!("reachable_{}", i), transition).unwrap();
                } else {
                    let transition = WorkflowTransition {
                        condition: None,
                        patterns: HashSet::new(),
                    };
                    graph.add_transition(
                        &format!("reachable_{}", i - 1),
                        &format!("reachable_{}", i),
                        transition
                    ).unwrap();
                }
            }

            // Create unreachable states (not connected to start)
            for i in 0..num_unreachable {
                let state = WorkflowState {
                    id: format!("unreachable_{}", i),
                    name: format!("Unreachable {}", i),
                    state_type: StateType::Process,
                    requires_export: false,
                    patterns: HashSet::new(),
                };
                graph.add_state(state).unwrap();
            }

            let analysis = graph.analyze();

            // Verify unreachable states are detected
            prop_assert_eq!(analysis.unreachable_states.len(), num_unreachable);

            // Verify none of the "reachable_*" states are in unreachable list
            for i in 0..num_reachable {
                prop_assert!(!analysis.unreachable_states.contains(&format!("reachable_{}", i)));
            }

            // Verify all "unreachable_*" states are in unreachable list
            for i in 0..num_unreachable {
                prop_assert!(analysis.unreachable_states.contains(&format!("unreachable_{}", i)));
            }
        }
    }
}
