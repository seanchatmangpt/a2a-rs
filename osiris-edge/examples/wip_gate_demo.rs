//! Demonstrates the Kanban WIP gate for bounded concurrency
//!
//! This example shows how the WIP gate prevents overload by
//! rejecting work when at capacity, ensuring bounded response times.

use osiris_edge::{AsyncWipGate, KanbanWipGate, WipError};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() {
    // Create a gate allowing max 3 concurrent work items
    let gate = Arc::new(KanbanWipGate::new(3));
    println!("Created WIP gate with limit of {}", gate.limit());
    println!();

    // Simulate incoming work requests
    let mut handles = vec![];

    for i in 1..=10 {
        let gate = Arc::clone(&gate);

        let handle = tokio::spawn(async move {
            println!(
                "[Request {}] Attempting to acquire slot ({}/{} occupied)",
                i,
                gate.current(),
                gate.limit()
            );

            // Try to acquire a WIP slot
            match gate.try_acquire().await {
                Ok(_permit) => {
                    println!(
                        "[Request {}] ✓ Accepted! Processing... ({}/{} occupied)",
                        i,
                        gate.current(),
                        gate.limit()
                    );

                    // Simulate work
                    sleep(Duration::from_millis(500)).await;

                    println!(
                        "[Request {}] ✓ Completed! ({}/{} occupied)",
                        i,
                        gate.current(),
                        gate.limit()
                    );
                    // Permit auto-released here
                }
                Err(WipError::WipLimitReached { current, limit }) => {
                    println!(
                        "[Request {}] ✗ Rejected! WIP limit reached ({}/{})",
                        i, current, limit
                    );
                    // In a real system, emit refusal receipt here
                }
                Err(e) => {
                    eprintln!("[Request {}] Error: {}", i, e);
                }
            }
        });

        handles.push(handle);

        // Stagger requests slightly
        sleep(Duration::from_millis(50)).await;
    }

    // Wait for all requests to complete
    for handle in handles {
        let _ = handle.await;
    }

    println!();
    println!(
        "All requests processed. Final gate state: {}/{} occupied",
        gate.current(),
        gate.limit()
    );
}
