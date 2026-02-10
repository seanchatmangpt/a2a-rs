//! Life Firewall admission control demo
//!
//! Demonstrates the Life Firewall admission control system with:
//! - WIP token limiting
//! - Supplier quality tracking
//! - Jidoka mode gating
//! - Emergency channel prioritization

use a2a_rs::port::AsyncAdmissionController;
use a2a_rs::services::{FirewallConfig, FirewallService};
use a2a_rs::{
    AdmissionConfig, AdmissionDecision, DefaultAdmissionController, IngressChannel, JidokaMode,
    WorkConstraints, WorkPacket,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Life Firewall Admission Control Demo ===\n");

    // Create admission controller with custom config
    let config = AdmissionConfig {
        max_wip: 3,
        min_supplier_quality: 0.6,
        initial_jidoka_mode: JidokaMode::Green,
    };
    let controller = DefaultAdmissionController::with_config(config);

    // Wrap in firewall service
    let firewall = FirewallService::new(controller, FirewallConfig::default());

    // Demo 1: Basic admission
    println!("--- Demo 1: Basic Admission ---");
    let packet1 = create_work_packet("work-1", IngressChannel::Batch, "supplier-1");
    match firewall.request_admission(packet1).await? {
        AdmissionDecision::Admitted {
            work_packet_id,
            assigned_token_id,
            ..
        } => {
            println!(
                "✓ Admitted: {} (token: {})",
                work_packet_id, assigned_token_id
            );
        }
        AdmissionDecision::Refused { receipt } => {
            println!("✗ Refused: {:?}", receipt.reason);
        }
    }

    let health = firewall.get_system_health().await?;
    println!(
        "  System Health: WIP={}/{}, Mode={:?}\n",
        health.current_wip, health.max_wip, health.jidoka_mode
    );

    // Demo 2: WIP limit enforcement
    println!("--- Demo 2: WIP Limit Enforcement ---");
    let packet2 = create_work_packet("work-2", IngressChannel::Batch, "supplier-1");
    let packet3 = create_work_packet("work-3", IngressChannel::Batch, "supplier-1");
    firewall.request_admission(packet2).await?;
    firewall.request_admission(packet3).await?;

    // This should be refused due to WIP limit (max 3)
    let packet4 = create_work_packet("work-4", IngressChannel::Batch, "supplier-1");
    match firewall.request_admission(packet4).await? {
        AdmissionDecision::Admitted { .. } => {
            println!("  Unexpected admission!");
        }
        AdmissionDecision::Refused { receipt } => {
            println!("✓ Correctly refused due to WIP limit");
            println!("  Reason: {:?}\n", receipt.reason);
        }
    }

    // Demo 3: Jidoka mode - Yellow (emergency only)
    println!("--- Demo 3: Jidoka Mode - Yellow (Emergency Only) ---");
    firewall.set_jidoka_mode(JidokaMode::Yellow).await?;
    println!("  Set mode to YELLOW (emergency only)");

    // Complete some work to free up tokens
    firewall.complete_work("work-1", true).await?;
    firewall.complete_work("work-2", true).await?;

    // Batch should be refused
    let batch = create_work_packet("work-5", IngressChannel::Batch, "supplier-1");
    match firewall.request_admission(batch).await? {
        AdmissionDecision::Refused { receipt } => {
            println!("✓ Batch work refused in YELLOW mode");
            println!("  Reason: {:?}", receipt.reason);
        }
        _ => println!("  Unexpected admission!"),
    }

    // Emergency should be admitted
    let emergency = create_work_packet("work-6", IngressChannel::Emergency, "supplier-1");
    match firewall.request_admission(emergency).await? {
        AdmissionDecision::Admitted { work_packet_id, .. } => {
            println!("✓ Emergency work admitted: {}\n", work_packet_id);
        }
        _ => println!("  Unexpected refusal!"),
    }

    // Demo 4: Supplier quality tracking
    println!("--- Demo 4: Supplier Quality Tracking ---");
    firewall.set_jidoka_mode(JidokaMode::Green).await?;

    // Complete work with mixed results
    firewall.complete_work("work-3", true).await?;
    firewall.complete_work("work-6", false).await?; // Defect

    let quality = firewall.get_supplier_quality("supplier-1").await?;
    println!("  Supplier: {}", quality.supplier_id);
    println!("  Total submitted: {}", quality.total_submitted);
    println!("  Successful: {}", quality.successful);
    println!("  Defects: {}", quality.defects);
    println!("  Quality score: {:.2}\n", quality.quality_score);

    // Demo 5: Jidoka mode - Red (full halt)
    println!("--- Demo 5: Jidoka Mode - Red (Full Halt) ---");
    firewall.set_jidoka_mode(JidokaMode::Red).await?;
    println!("  Set mode to RED (full halt)");

    let emergency2 = create_work_packet("work-7", IngressChannel::Emergency, "supplier-1");
    match firewall.request_admission(emergency2).await? {
        AdmissionDecision::Refused { receipt } => {
            println!("✓ Even emergency refused in RED mode");
            println!("  Reason: {:?}\n", receipt.reason);
        }
        _ => println!("  Unexpected admission!"),
    }

    // Final metrics
    println!("--- Final Metrics ---");
    let metrics = firewall.get_metrics().await?;
    println!("  Current WIP: {}/{}", metrics.current_wip, metrics.max_wip);
    println!("  Jidoka mode: {:?}", metrics.jidoka_mode);
    println!("  Quality score: {:.2}", metrics.quality_score);
    println!("  Queue depth: {}", metrics.queue_depth);

    Ok(())
}

fn create_work_packet(id: &str, channel: IngressChannel, supplier_id: &str) -> WorkPacket {
    WorkPacket {
        id: id.to_string(),
        objective: format!("Process {} work", id),
        constraints: WorkConstraints {
            max_execution_time_secs: 300,
            max_memory_bytes: Some(1024 * 1024 * 100), // 100MB
            deadline: Some(
                chrono::Utc::now()
                    .checked_add_signed(chrono::Duration::hours(1))
                    .unwrap()
                    .to_rfc3339(),
            ),
        },
        acceptance_test: "Verify output matches specification".to_string(),
        reversibility: true,
        channel,
        supplier_id: Some(supplier_id.to_string()),
        priority: match channel {
            IngressChannel::Emergency => Some(100),
            IngressChannel::Scheduled => Some(50),
            IngressChannel::Batch => Some(10),
        },
    }
}
