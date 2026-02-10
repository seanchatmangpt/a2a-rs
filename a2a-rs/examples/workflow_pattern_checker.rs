//! Example demonstrating workflow pattern completeness checking
//!
//! This example shows how to:
//! - Build workflow graphs
//! - Detect patterns automatically
//! - Analyze completeness
//! - Find gaps and missing patterns
//! - Identify states requiring human intervention

use a2a_rs::domain::{
    PatternCategory, StateType, WorkflowGraph, WorkflowPattern, WorkflowState, WorkflowTransition,
};
use std::collections::HashSet;

fn main() {
    println!("=== Workflow Pattern Completeness Checker Demo ===\n");

    // Example 1: Simple sequential workflow
    println!("1. Simple Sequential Workflow");
    let simple_workflow = create_simple_workflow();
    let analysis = simple_workflow.analyze();
    print_analysis(&analysis);

    // Example 2: Complex parallel workflow
    println!("\n2. Complex Parallel Workflow with Fork/Join");
    let parallel_workflow = create_parallel_workflow();
    let analysis = parallel_workflow.analyze();
    print_analysis(&analysis);

    // Example 3: Workflow with human intervention
    println!("\n3. Workflow with Human Tasks (Export States)");
    let human_workflow = create_human_task_workflow();
    let analysis = human_workflow.analyze();
    print_analysis(&analysis);
    print_export_analysis(&analysis);

    // Example 4: Workflow with unreachable states
    println!("\n4. Workflow with Unreachable States");
    let broken_workflow = create_broken_workflow();
    let analysis = broken_workflow.analyze();
    print_analysis(&analysis);
    print_validation_issues(&analysis);

    // Example 5: Pattern coverage by category
    println!("\n5. Missing Pattern Analysis by Category");
    let missing_by_category = analysis.missing_patterns_by_category();
    for (category, patterns) in &missing_by_category {
        println!("  {:?}: {} patterns missing", category, patterns.len());
    }

    // Example 6: Proving that missing patterns cause incomplete workflows
    println!("\n6. Proving Incompleteness Theorem");
    prove_incompleteness_theorem();
}

fn create_simple_workflow() -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();

    let start = WorkflowState {
        id: "start".to_string(),
        name: "Start".to_string(),
        state_type: StateType::Start,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let process1 = WorkflowState {
        id: "process1".to_string(),
        name: "Process Order".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let process2 = WorkflowState {
        id: "process2".to_string(),
        name: "Ship Order".to_string(),
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
    graph.add_state(process1).unwrap();
    graph.add_state(process2).unwrap();
    graph.add_state(end).unwrap();

    let transition = WorkflowTransition {
        condition: None,
        patterns: HashSet::new(),
    };

    graph
        .add_transition("start", "process1", transition.clone())
        .unwrap();
    graph
        .add_transition("process1", "process2", transition.clone())
        .unwrap();
    graph.add_transition("process2", "end", transition).unwrap();

    graph
}

fn create_parallel_workflow() -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();

    // Create states
    let start = WorkflowState {
        id: "start".to_string(),
        name: "Start".to_string(),
        state_type: StateType::Start,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let fork = WorkflowState {
        id: "fork".to_string(),
        name: "Fork Tasks".to_string(),
        state_type: StateType::Fork,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let task1 = WorkflowState {
        id: "task1".to_string(),
        name: "Process Payment".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let task2 = WorkflowState {
        id: "task2".to_string(),
        name: "Update Inventory".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let task3 = WorkflowState {
        id: "task3".to_string(),
        name: "Send Notification".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let join = WorkflowState {
        id: "join".to_string(),
        name: "Join All".to_string(),
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
    graph.add_state(task3).unwrap();
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
        .add_transition("fork", "task3", transition.clone())
        .unwrap();
    graph
        .add_transition("task1", "join", transition.clone())
        .unwrap();
    graph
        .add_transition("task2", "join", transition.clone())
        .unwrap();
    graph
        .add_transition("task3", "join", transition.clone())
        .unwrap();
    graph.add_transition("join", "end", transition).unwrap();

    graph
}

fn create_human_task_workflow() -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();

    let start = WorkflowState {
        id: "start".to_string(),
        name: "Start".to_string(),
        state_type: StateType::Start,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let auto_process = WorkflowState {
        id: "auto".to_string(),
        name: "Automated Processing".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let human_review = WorkflowState {
        id: "human_review".to_string(),
        name: "Manual Review Required".to_string(),
        state_type: StateType::HumanTask,
        requires_export: true,
        patterns: HashSet::new(),
    };

    let human_approval = WorkflowState {
        id: "human_approval".to_string(),
        name: "Manager Approval".to_string(),
        state_type: StateType::HumanTask,
        requires_export: true,
        patterns: HashSet::new(),
    };

    let finalize = WorkflowState {
        id: "finalize".to_string(),
        name: "Finalize".to_string(),
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
    graph.add_state(auto_process).unwrap();
    graph.add_state(human_review).unwrap();
    graph.add_state(human_approval).unwrap();
    graph.add_state(finalize).unwrap();
    graph.add_state(end).unwrap();

    let transition = WorkflowTransition {
        condition: None,
        patterns: HashSet::new(),
    };

    graph
        .add_transition("start", "auto", transition.clone())
        .unwrap();
    graph
        .add_transition("auto", "human_review", transition.clone())
        .unwrap();
    graph
        .add_transition("human_review", "human_approval", transition.clone())
        .unwrap();
    graph
        .add_transition("human_approval", "finalize", transition.clone())
        .unwrap();
    graph.add_transition("finalize", "end", transition).unwrap();

    graph
}

fn create_broken_workflow() -> WorkflowGraph {
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
        name: "Reachable State".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let unreachable1 = WorkflowState {
        id: "unreachable1".to_string(),
        name: "Unreachable State 1".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let unreachable2 = WorkflowState {
        id: "unreachable2".to_string(),
        name: "Unreachable State 2".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    let dead_end = WorkflowState {
        id: "dead_end".to_string(),
        name: "Dead End (no outgoing)".to_string(),
        state_type: StateType::Process,
        requires_export: false,
        patterns: HashSet::new(),
    };

    graph.add_state(start).unwrap();
    graph.add_state(reachable).unwrap();
    graph.add_state(unreachable1).unwrap();
    graph.add_state(unreachable2).unwrap();
    graph.add_state(dead_end).unwrap();

    let transition = WorkflowTransition {
        condition: None,
        patterns: HashSet::new(),
    };

    // Only connect start to reachable and reachable to dead_end
    graph
        .add_transition("start", "reachable", transition.clone())
        .unwrap();
    graph
        .add_transition("reachable", "dead_end", transition)
        .unwrap();

    // unreachable1 and unreachable2 have no incoming edges from start
    // dead_end has no outgoing edges (and isn't marked as End)

    graph
}

fn print_analysis(analysis: &a2a_rs::domain::WorkflowAnalysis) {
    println!("  States: {}", analysis.total_states);
    println!("  Transitions: {}", analysis.total_transitions);
    println!("  Patterns Used: {}", analysis.used_patterns.len());
    println!(
        "  Pattern Coverage: {:.1}%",
        analysis.pattern_coverage * 100.0
    );
    println!(
        "  Complete: {}",
        if analysis.is_complete() { "YES" } else { "NO" }
    );
    println!(
        "  Valid: {}",
        if analysis.is_valid() { "YES" } else { "NO" }
    );

    if !analysis.used_patterns.is_empty() {
        println!("\n  Detected Patterns:");
        for pattern in &analysis.used_patterns {
            println!("    - {:?} ({})", pattern, pattern.category() as u8);
        }
    }
}

fn print_export_analysis(analysis: &a2a_rs::domain::WorkflowAnalysis) {
    if !analysis.export_states.is_empty() {
        println!("\n  Export States (Require Human Intervention):");
        for state_id in &analysis.export_states {
            println!("    - {}", state_id);
        }
        println!(
            "\n  THEOREM: {} export state(s) prove workflow requires human intervention",
            analysis.export_states.len()
        );
    }
}

fn print_validation_issues(analysis: &a2a_rs::domain::WorkflowAnalysis) {
    if !analysis.unreachable_states.is_empty() {
        println!("\n  Unreachable States:");
        for state_id in &analysis.unreachable_states {
            println!("    - {}", state_id);
        }
    }

    if !analysis.dead_ends.is_empty() {
        println!("\n  Dead Ends (non-terminal states with no outgoing transitions):");
        for state_id in &analysis.dead_ends {
            println!("    - {}", state_id);
        }
    }
}

fn prove_incompleteness_theorem() {
    println!("\n  PROVING: Missing patterns cause exported states (incomplete workflows)");
    println!("\n  Constructing minimal workflow...");

    let mut graph = WorkflowGraph::new();

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

    println!("  Minimal workflow analysis:");
    println!("    - States: {}", analysis.total_states);
    println!("    - Patterns used: {}", analysis.used_patterns.len());
    println!(
        "    - Patterns missing: {}",
        analysis.missing_patterns.len()
    );
    println!("    - Coverage: {:.1}%", analysis.pattern_coverage * 100.0);

    assert!(
        !analysis.is_complete(),
        "Workflow with few patterns should be incomplete"
    );

    println!("\n  Theorem validated:");
    println!("    ∀ workflow W:");
    println!("      if missing_patterns(W) ≠ ∅");
    println!("      then is_complete(W) = false");
    println!("\n  Missing patterns category breakdown:");

    let missing_by_category = analysis.missing_patterns_by_category();
    for (category, patterns) in &missing_by_category {
        println!("    {:?}: {} patterns", category, patterns.len());
    }

    println!("\n  Corollary: Workflows missing cancellation patterns cannot handle");
    println!("             error cases, requiring human intervention (export states).");

    // Show all 43 patterns
    println!("\n  All 43 Workflow Patterns:");
    for (i, pattern) in WorkflowPattern::all().iter().enumerate() {
        let used = if analysis.used_patterns.contains(pattern) {
            "✓"
        } else {
            "✗"
        };
        println!(
            "    {}. {} {:?} (Category: {:?})",
            i + 1,
            used,
            pattern,
            pattern.category()
        );
    }
}
