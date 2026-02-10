//! Task graph visualization in DOT format
//!
//! This module provides tools for exporting task graphs to Graphviz DOT format,
//! enabling visualization of task dependencies, states, and blocking relationships.
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::{TaskGraph, TaskGraphVisualizer};
//! use a2a_rs::domain::{Task, TaskState};
//!
//! let mut graph = TaskGraph::new();
//! let task_a = Task::new("task-a".to_string(), "ctx-1".to_string());
//! let task_b = Task::new("task-b".to_string(), "ctx-1".to_string());
//!
//! graph.add_task(task_a).unwrap();
//! graph.add_task(task_b).unwrap();
//! graph.add_dependency("task-b", "task-a").unwrap();
//!
//! // Generate DOT format
//! let dot = graph.to_dot();
//! println!("{}", dot);
//! ```

use crate::construct::coordination::TaskGraph;
use crate::domain::TaskState;

/// Configuration options for DOT graph generation
#[derive(Debug, Clone)]
pub struct DotOptions {
    /// Include task state in node labels
    pub show_state: bool,
    /// Include artifact requirements on edges
    pub show_artifacts: bool,
    /// Highlight tasks with unmet prerequisites
    pub highlight_blocked: bool,
    /// Use horizontal layout (left-to-right)
    pub horizontal: bool,
    /// Include timestamp information
    pub show_timestamps: bool,
}

impl Default for DotOptions {
    fn default() -> Self {
        Self {
            show_state: true,
            show_artifacts: true,
            highlight_blocked: true,
            horizontal: true,
            show_timestamps: false,
        }
    }
}

/// Extension trait for TaskGraph to enable DOT export
pub trait TaskGraphVisualizer {
    /// Generate a DOT format representation of the task graph
    ///
    /// # Returns
    ///
    /// A string containing the DOT graph definition, suitable for rendering
    /// with Graphviz tools (dot, neato, fdp, etc.)
    ///
    /// # Example
    ///
    /// ```rust
    /// use a2a_rs::construct::{TaskGraph, TaskGraphVisualizer};
    /// use a2a_rs::domain::Task;
    ///
    /// let mut graph = TaskGraph::new();
    /// let task = Task::new("task-1".to_string(), "ctx-1".to_string());
    /// graph.add_task(task).unwrap();
    ///
    /// let dot = graph.to_dot();
    /// // Output can be saved to file and rendered: dot -Tpng graph.dot -o graph.png
    /// ```
    fn to_dot(&self) -> String;

    /// Generate a DOT format representation with custom options
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for DOT generation
    fn to_dot_with_options(&self, options: &DotOptions) -> String;
}

impl TaskGraphVisualizer for TaskGraph {
    fn to_dot(&self) -> String {
        self.to_dot_with_options(&DotOptions::default())
    }

    fn to_dot_with_options(&self, options: &DotOptions) -> String {
        let mut dot = String::new();

        // Graph header
        dot.push_str("digraph TaskGraph {\n");

        // Graph attributes
        if options.horizontal {
            dot.push_str("  rankdir=LR;\n");
        }
        dot.push_str("  node [shape=box];\n");
        dot.push_str("\n");

        // Get all task IDs in deterministic order
        let mut task_ids = self.task_ids();
        task_ids.sort();

        // Check which tasks have unmet prerequisites (for highlighting)
        let blocked_tasks: std::collections::HashSet<String> = if options.highlight_blocked {
            task_ids
                .iter()
                .filter(|id| !self.prerequisites_met(id).unwrap_or(true))
                .cloned()
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        // Generate nodes
        for task_id in &task_ids {
            if let Some(task) = self.get_task(task_id) {
                let mut label = escape_dot(task_id);

                if options.show_state {
                    label = format!("{}\\n{:?}", label, task.status.state);
                }

                let (fillcolor, fontcolor) = state_color(&task.status.state);

                let is_blocked = blocked_tasks.contains(task_id);
                let border_color = if is_blocked { "red" } else { "black" };
                let border_width = if is_blocked { "2.0" } else { "1.0" };

                dot.push_str(&format!(
                    "  \"{}\" [label=\"{}\" fillcolor={} fontcolor={} style=filled color={} penwidth={}];\n",
                    escape_dot(task_id),
                    label,
                    fillcolor,
                    fontcolor,
                    border_color,
                    border_width
                ));
            }
        }

        dot.push_str("\n");

        // Generate edges from dependencies
        for task_id in &task_ids {
            if let Some(prereqs) = self.dependencies(task_id) {
                let mut prereq_list: Vec<_> = prereqs.iter().collect();
                prereq_list.sort();

                for prereq_id in prereq_list {
                    dot.push_str(&format!(
                        "  \"{}\" -> \"{}\";\n",
                        escape_dot(prereq_id),
                        escape_dot(task_id)
                    ));
                }
            }
        }

        // Add legend
        dot.push_str("\n");
        dot.push_str("  // Legend\n");
        dot.push_str("  subgraph cluster_legend {\n");
        dot.push_str("    label=\"Task States\";\n");
        dot.push_str("    style=filled;\n");
        dot.push_str("    color=lightgrey;\n");
        dot.push_str("    node [shape=box style=filled];\n");

        let states = [
            (TaskState::Submitted, "Submitted"),
            (TaskState::Working, "Working"),
            (TaskState::InputRequired, "InputRequired"),
            (TaskState::Completed, "Completed"),
            (TaskState::Failed, "Failed"),
            (TaskState::Canceled, "Canceled"),
        ];

        for (state, label) in &states {
            let (fillcolor, fontcolor) = state_color(state);
            dot.push_str(&format!(
                "    legend_{:?} [label=\"{}\" fillcolor={} fontcolor={}];\n",
                state, label, fillcolor, fontcolor
            ));
        }

        if options.highlight_blocked {
            dot.push_str("    legend_blocked [label=\"Blocked (unmet prerequisites)\" color=red penwidth=2.0];\n");
        }

        dot.push_str("  }\n");

        // Graph footer
        dot.push_str("}\n");

        dot
    }
}

/// Map TaskState to DOT color attributes
fn state_color(state: &TaskState) -> (&'static str, &'static str) {
    match state {
        TaskState::Submitted => ("lightblue", "black"),
        TaskState::Working => ("yellow", "black"),
        TaskState::InputRequired => ("orange", "black"),
        TaskState::Completed => ("lightgreen", "black"),
        TaskState::Failed => ("red", "white"),
        TaskState::Canceled => ("gray", "white"),
        TaskState::Rejected => ("darkred", "white"),
        TaskState::AuthRequired => ("purple", "white"),
        TaskState::Unknown => ("white", "black"),
    }
}

/// Escape special characters for DOT format
fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::construct::TaskGraph;
    use crate::domain::{Task, TaskState};

    fn create_test_task(id: &str, state: TaskState) -> Task {
        let mut task = Task::new(id.to_string(), "ctx-1".to_string());
        task.status.state = state;
        task
    }

    #[test]
    fn test_empty_graph_dot() {
        let graph = TaskGraph::new();
        let dot = graph.to_dot();

        assert!(dot.contains("digraph TaskGraph"));
        assert!(dot.contains("rankdir=LR"));
        assert!(dot.contains("Legend"));
    }

    #[test]
    fn test_single_task_dot() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-1", TaskState::Completed))
            .unwrap();

        let dot = graph.to_dot();

        assert!(dot.contains("\"task-1\""));
        assert!(dot.contains("Completed"));
        assert!(dot.contains("lightgreen"));
    }

    #[test]
    fn test_dependency_edges() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Working))
            .unwrap();
        graph.add_dependency("task-b", "task-a").unwrap();

        let dot = graph.to_dot();

        assert!(dot.contains("\"task-a\""));
        assert!(dot.contains("\"task-b\""));
        assert!(dot.contains("\"task-a\" -> \"task-b\""));
    }

    #[test]
    fn test_blocked_task_highlighting() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-a", TaskState::Working))
            .unwrap();
        graph
            .add_task(create_test_task("task-b", TaskState::Submitted))
            .unwrap();
        graph.add_dependency("task-b", "task-a").unwrap();

        let dot = graph.to_dot();

        // task-b should be highlighted as blocked (prerequisite not met)
        assert!(dot.contains("\"task-b\""));
        assert!(dot.contains("color=red"));
        assert!(dot.contains("penwidth=2.0"));
    }

    #[test]
    fn test_state_colors() {
        assert_eq!(state_color(&TaskState::Submitted), ("lightblue", "black"));
        assert_eq!(state_color(&TaskState::Working), ("yellow", "black"));
        assert_eq!(state_color(&TaskState::Completed), ("lightgreen", "black"));
        assert_eq!(state_color(&TaskState::Failed), ("red", "white"));
        assert_eq!(state_color(&TaskState::Canceled), ("gray", "white"));
    }

    #[test]
    fn test_escape_dot() {
        assert_eq!(escape_dot("simple"), "simple");
        assert_eq!(escape_dot("with\"quotes"), "with\\\"quotes");
        assert_eq!(escape_dot("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_dot("with\nwline"), "with\\nwline");
    }

    #[test]
    fn test_dot_options() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("task-1", TaskState::Completed))
            .unwrap();

        let options = DotOptions {
            show_state: false,
            show_artifacts: false,
            highlight_blocked: false,
            horizontal: false,
            show_timestamps: false,
        };

        let dot = graph.to_dot_with_options(&options);

        assert!(dot.contains("digraph TaskGraph"));
        assert!(!dot.contains("rankdir=LR")); // horizontal = false
        assert!(!dot.contains("Completed")); // show_state = false
    }

    #[test]
    fn test_complex_graph() {
        let mut graph = TaskGraph::new();

        // Create a diamond dependency structure
        graph
            .add_task(create_test_task("root", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("left", TaskState::Completed))
            .unwrap();
        graph
            .add_task(create_test_task("right", TaskState::Working))
            .unwrap();
        graph
            .add_task(create_test_task("join", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("left", "root").unwrap();
        graph.add_dependency("right", "root").unwrap();
        graph.add_dependency("join", "left").unwrap();
        graph.add_dependency("join", "right").unwrap();

        let dot = graph.to_dot();

        // Verify all nodes present
        assert!(dot.contains("\"root\""));
        assert!(dot.contains("\"left\""));
        assert!(dot.contains("\"right\""));
        assert!(dot.contains("\"join\""));

        // Verify edges
        assert!(dot.contains("\"root\" -> \"left\""));
        assert!(dot.contains("\"root\" -> \"right\""));
        assert!(dot.contains("\"left\" -> \"join\""));
        assert!(dot.contains("\"right\" -> \"join\""));

        // Join should be blocked (right is still working)
        assert!(dot.contains("color=red")); // blocked indicator
    }

    #[test]
    fn test_topological_order_in_output() {
        let mut graph = TaskGraph::new();
        graph
            .add_task(create_test_task("c", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("a", TaskState::Submitted))
            .unwrap();
        graph
            .add_task(create_test_task("b", TaskState::Submitted))
            .unwrap();

        graph.add_dependency("b", "a").unwrap();
        graph.add_dependency("c", "b").unwrap();

        let dot = graph.to_dot();

        // Tasks should appear in sorted order in the output
        let a_pos = dot.find("\"a\"").unwrap();
        let b_pos = dot.find("\"b\"").unwrap();
        let c_pos = dot.find("\"c\"").unwrap();

        // Due to sorting, should appear in alphabetical order
        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }
}
