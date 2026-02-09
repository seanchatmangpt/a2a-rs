//! Task Finite-State Machine Demo
//!
//! This example demonstrates the TaskStateMachine for managing task lifecycle
//! with deterministic state transitions.
//!
//! Run with:
//! ```bash
//! cargo run --example task_fsm_demo
//! ```

use a2a_rs::construct::{StateTransitionError, TaskStateMachine};
use a2a_rs::domain::{Message, TaskState};

fn main() {
    println!("=== Task FSM Demo ===\n");

    // Create a new task FSM
    let mut fsm = TaskStateMachine::new("task-demo-001".to_string());
    println!("1. Created FSM in state: {:?}", fsm.current_state());
    println!("   Is terminal: {}", fsm.is_terminal());
    println!("   Allowed transitions: {:?}\n", fsm.allowed_transitions());

    // Transition to Working
    println!("2. Transitioning to Working...");
    match fsm.start_working(None) {
        Ok(transition) => {
            println!(
                "   ✓ Transitioned from {:?} to {:?}",
                transition.from, transition.to
            );
            println!("   Timestamp: {}", transition.timestamp);
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!("   Current state: {:?}\n", fsm.current_state());

    // Request input
    println!("3. Requesting input from user...");
    let input_message = Message::assistant_text(
        "Please provide your expense receipt.".to_string(),
        "msg-001".to_string(),
    );
    match fsm.request_input(Some(input_message)) {
        Ok(transition) => {
            println!(
                "   ✓ Transitioned from {:?} to {:?}",
                transition.from, transition.to
            );
            if let Some(msg) = &transition.message {
                println!("   Message: {:?}", msg.parts.first().map(|p| &p.text));
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!("   Current state: {:?}\n", fsm.current_state());

    // Resume working after receiving input
    println!("4. Resuming work after input...");
    match fsm.start_working(None) {
        Ok(transition) => {
            println!(
                "   ✓ Transitioned from {:?} to {:?}",
                transition.from, transition.to
            );
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!("   Current state: {:?}\n", fsm.current_state());

    // Complete the task
    println!("5. Completing task...");
    let completion_message = Message::assistant_text(
        "Expense reimbursement processed successfully.".to_string(),
        "msg-002".to_string(),
    );
    match fsm.complete(Some(completion_message), None) {
        Ok(transition) => {
            println!(
                "   ✓ Transitioned from {:?} to {:?}",
                transition.from, transition.to
            );
            println!("   Is terminal: {}", fsm.is_terminal());
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!("   Current state: {:?}\n", fsm.current_state());

    // Try to transition from terminal state (should fail)
    println!("6. Attempting to transition from terminal state...");
    match fsm.start_working(None) {
        Ok(_) => println!("   ✗ Unexpectedly succeeded!"),
        Err(StateTransitionError::TransitionFromTerminalState { state }) => {
            println!(
                "   ✓ Correctly rejected: Cannot transition from terminal state {:?}",
                state
            );
        }
        Err(e) => println!("   ✗ Different error: {}", e),
    }

    // Print full transition history
    println!("\n7. Transition History:");
    for (i, transition) in fsm.history().iter().enumerate() {
        println!(
            "   {}. {:?} → {:?} at {}",
            i + 1,
            transition.from,
            transition.to,
            transition.timestamp.format("%H:%M:%S%.3f")
        );
    }

    // Demonstrate invalid transition
    println!("\n8. Demo: Invalid Transition Rejection");
    let mut fsm2 = TaskStateMachine::new("task-demo-002".to_string());
    match fsm2.complete(None, None) {
        Ok(_) => println!("   ✗ Unexpectedly succeeded!"),
        Err(StateTransitionError::InvalidTransition { from, to }) => {
            println!(
                "   ✓ Correctly rejected: Cannot go from {:?} to {:?}",
                from, to
            );
        }
        Err(e) => println!("   ✗ Different error: {}", e),
    }

    println!("\n=== Demo Complete ===");
}
