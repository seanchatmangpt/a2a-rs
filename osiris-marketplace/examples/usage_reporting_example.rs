//! Example demonstrating usage reporting integration with Service Control
//!
//! This example shows how to:
//! 1. Initialize a Service Control reporter
//! 2. Track operation usage
//! 3. Report metrics for billing
//!
//! Requires: GOOGLE_APPLICATION_CREDENTIALS environment variable set

#[cfg(all(
    feature = "service-control",
    feature = "procurement-api",
    feature = "pubsub"
))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_marketplace::{
        adapter::ServiceControlReporter,
        domain::{MetricType, OperationType, OperationUsage, UsageMetric},
        port::UsageReporter,
    };
    use std::sync::Arc;

    // Initialize tracing for debugging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Google Cloud Service Control Usage Reporting Example ===\n");

    // Step 1: Create Service Control reporter
    println!("Step 1: Initializing Service Control reporter...");
    let service_name = "example-marketplace-service.prod.googleapis.com";
    let project_id = std::env::var("GCP_PROJECT_ID").unwrap_or_else(|_| "my-project".to_string());

    let reporter = Arc::new(
        ServiceControlReporter::with_default_credentials(
            service_name.to_string(),
            project_id.clone(),
        )
        .await?,
    );

    println!("✓ Reporter created for service: {}", service_name);
    println!("  Project ID: {}", project_id);

    // Step 2: Verify credentials
    println!("\nStep 2: Verifying credentials...");
    reporter.verify_credentials().await?;
    println!("✓ Credentials verified successfully");

    // Step 3: Create and report a single operation
    println!("\nStep 3: Reporting single operation usage...");
    let single_usage = OperationUsage::new(
        "op-provision-001".to_string(),
        OperationType::ProvisionEntitlement,
        "providers/example-provider/entitlements/cust-123".to_string(),
        "providers/example-provider/accounts/acct-456".to_string(),
        service_name.to_string(),
    )
    .add_metric(UsageMetric::new(MetricType::ActiveUsers, 10))
    .add_metric(UsageMetric::new(MetricType::ApiCalls, 250))
    .with_label("customer_tier".to_string(), "premium".to_string())
    .with_label("region".to_string(), "us-central1".to_string())
    .with_user_id("customer-admin@example.com".to_string());

    match reporter.report_operation(&single_usage).await {
        Ok(report) => {
            println!("✓ Operation reported successfully");
            println!("  Service: {}", report.service_name);
            println!("  Operations: {:?}", report.operation_ids);
            println!("  Timestamp: {}", report.report_timestamp);
        }
        Err(e) => {
            eprintln!("✗ Failed to report operation: {}", e);
        }
    }

    // Step 4: Create and report batch operations
    println!("\nStep 4: Reporting batch operations...");
    let batch_operations = vec![
        OperationUsage::new(
            "op-modify-001".to_string(),
            OperationType::ModifyEntitlement,
            "providers/example-provider/entitlements/cust-789".to_string(),
            "providers/example-provider/accounts/acct-789".to_string(),
            service_name.to_string(),
        )
        .add_metric(UsageMetric::new(MetricType::ActiveUsers, 25))
        .with_label("upgrade".to_string(), "true".to_string()),
        OperationUsage::new(
            "op-modify-002".to_string(),
            OperationType::ModifyEntitlement,
            "providers/example-provider/entitlements/cust-001".to_string(),
            "providers/example-provider/accounts/acct-001".to_string(),
            service_name.to_string(),
        )
        .add_metric(UsageMetric::new(MetricType::ActiveUsers, 15))
        .with_label("upgrade".to_string(), "false".to_string()),
    ];

    println!(
        "Reporting {} operations in batch...",
        batch_operations.len()
    );
    match reporter.report_batch(&batch_operations).await {
        Ok(report) => {
            println!("✓ Batch reported successfully");
            println!("  Operations reported: {}", report.operation_ids.len());
            for op_id in &report.operation_ids {
                println!("    - {}", op_id);
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to report batch: {}", e);
        }
    }

    // Step 5: Report cancellation
    println!("\nStep 5: Reporting cancellation operation...");
    let cancel_usage = OperationUsage::new(
        "op-cancel-001".to_string(),
        OperationType::CancelEntitlement,
        "providers/example-provider/entitlements/cust-123".to_string(),
        "providers/example-provider/accounts/acct-456".to_string(),
        service_name.to_string(),
    )
    .add_metric(UsageMetric::new(MetricType::ActiveUsers, 0))
    .with_label("reason".to_string(), "customer_request".to_string());

    match reporter.report_operation(&cancel_usage).await {
        Ok(report) => {
            println!("✓ Cancellation reported successfully");
        }
        Err(e) => {
            eprintln!("✗ Failed to report cancellation: {}", e);
        }
    }

    // Step 6: Report custom metrics
    println!("\nStep 6: Reporting custom metrics...");
    let custom_usage = OperationUsage::new(
        "op-custom-001".to_string(),
        OperationType::Custom("DataImport".to_string()),
        "providers/example-provider/entitlements/cust-456".to_string(),
        "providers/example-provider/accounts/acct-789".to_string(),
        service_name.to_string(),
    )
    .add_metric(UsageMetric::new(MetricType::DataProcessedGb, 500))
    .add_metric(UsageMetric::new(MetricType::SupportIncidents, 2))
    .with_label("data_source".to_string(), "s3".to_string())
    .with_label("duration_minutes".to_string(), "45".to_string());

    match reporter.report_operation(&custom_usage).await {
        Ok(report) => {
            println!("✓ Custom metrics reported successfully");
        }
        Err(e) => {
            eprintln!("✗ Failed to report custom metrics: {}", e);
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nKey Points:");
    println!("• Service Control API requires valid GCP credentials");
    println!("• Operations are batched for efficient reporting");
    println!("• Metrics enable accurate usage-based billing");
    println!("• Labels help with filtering and cost allocation");
    println!("\nFor more information, see USAGE_REPORTING.md");

    Ok(())
}

#[cfg(not(all(
    feature = "service-control",
    feature = "procurement-api",
    feature = "pubsub"
)))]
fn main() {
    eprintln!("This example requires all features to be enabled:");
    eprintln!("cargo run --example usage_reporting_example --all-features");
}
