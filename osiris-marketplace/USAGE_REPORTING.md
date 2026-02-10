# Cloud Marketplace Usage Reporting

This guide explains how to integrate Google Cloud Service Control usage reporting with osiris-marketplace for Cloud Marketplace billing.

## Overview

Usage reporting enables accurate billing for your Cloud Marketplace product by sending operation metrics to Google Cloud Service Control API. The integration tracks operations like:

- Entitlement provisioning
- Entitlement plan changes
- Entitlement cancellations
- Custom metrics (active users, API calls, data processed, etc.)

## Architecture

The implementation follows hexagonal architecture:

- **Domain** (`domain/usage.rs`): Pure types for `OperationUsage`, `UsageMetric`, and `UsageReport`
- **Port** (`port/usage_reporter.rs`): `UsageReporter` trait defining the interface
- **Adapter** (`adapter/service_control.rs`): `ServiceControlReporter` implementing the API integration
- **Application** (`application/usage_handler.rs`): `UsageTrackingHandler` orchestrating usage tracking

## Setup

### 1. Enable Feature Flag

Add the `service-control` feature to your dependencies:

```toml
[dependencies]
osiris-marketplace = { version = "0.1", features = ["service-control", "procurement-api", "pubsub"] }
```

### 2. Configure Credentials

Set up Google Cloud authentication:

```bash
# Option A: Use service account file
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account-key.json

# Option B: Use default credentials (Cloud Run, GKE, etc.)
# Credentials are automatically detected from the environment
```

### 3. Create Reporter Instance

```rust
use osiris_marketplace::adapter::ServiceControlReporter;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Service Control reporter
    let reporter = Arc::new(
        ServiceControlReporter::with_default_credentials(
            "my-marketplace-service.prod.googleapis.com".to_string(),
            "my-gcp-project".to_string(),
        )
        .await?
    );

    // Verify credentials
    reporter.verify_credentials().await?;

    Ok(())
}
```

## Usage Tracking

### Single Operation Reporting

```rust
use osiris_marketplace::{
    domain::{MetricType, OperationType, OperationUsage, UsageMetric},
    adapter::ServiceControlReporter,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Arc::new(
        ServiceControlReporter::with_default_credentials(
            "my-service.googleapis.com".to_string(),
            "project-id".to_string(),
        )
        .await?
    );

    // Create operation usage record
    let usage = OperationUsage::new(
        "op-12345".to_string(),
        OperationType::ProvisionEntitlement,
        "providers/my-provider/entitlements/123".to_string(),
        "providers/my-provider/accounts/456".to_string(),
        "my-service.googleapis.com".to_string(),
    )
    .add_metric(UsageMetric::new(MetricType::ActiveUsers, 100))
    .add_metric(UsageMetric::new(MetricType::ApiCalls, 5000))
    .with_label("region".to_string(), "us-central1".to_string())
    .with_user_id("user@example.com".to_string());

    // Report to Service Control
    let report = reporter.report_operation(&usage).await?;
    println!("Report submitted: {:?}", report);

    Ok(())
}
```

### Batch Reporting

```rust
use osiris_marketplace::{
    domain::{MetricType, OperationType, OperationUsage, UsageMetric},
    adapter::ServiceControlReporter,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Arc::new(
        ServiceControlReporter::with_default_credentials(
            "my-service.googleapis.com".to_string(),
            "project-id".to_string(),
        )
        .await?
    );

    // Create multiple usage records
    let usages = vec![
        OperationUsage::new(
            "op-1".to_string(),
            OperationType::ProvisionEntitlement,
            "providers/my-provider/entitlements/1".to_string(),
            "providers/my-provider/accounts/1".to_string(),
            "my-service.googleapis.com".to_string(),
        )
        .add_metric(UsageMetric::new(MetricType::ActiveUsers, 50)),

        OperationUsage::new(
            "op-2".to_string(),
            OperationType::ModifyEntitlement,
            "providers/my-provider/entitlements/2".to_string(),
            "providers/my-provider/accounts/2".to_string(),
            "my-service.googleapis.com".to_string(),
        )
        .add_metric(UsageMetric::new(MetricType::ApiCalls, 1000)),
    ];

    // Report all in batch
    let report = reporter.report_batch(&usages).await?;
    println!("Batch report submitted: {:?}", report);

    Ok(())
}
```

### Integration with Event Handler

```rust
use osiris_marketplace::{
    adapter::{ProcurementApiClient, PubSubConsumer, ServiceControlReporter},
    application::{UsageTrackingHandler, MarketplaceEventHandler},
    domain::{OperationType, EntitlementEvent},
    port::{AccountApprover, EventConsumer},
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize all adapters
    let consumer = Arc::new(
        PubSubConsumer::new(
            "my-project".to_string(),
            "marketplace-events".to_string(),
        )
        .await?
    );

    let approver = Arc::new(
        ProcurementApiClient::new(
            "my-project".to_string(),
            std::env::var("GOOGLE_OAUTH_TOKEN")?,
        )?
    );

    let reporter = Arc::new(
        ServiceControlReporter::with_default_credentials(
            "my-service.googleapis.com".to_string(),
            "my-project".to_string(),
        )
        .await?
    );

    // Initialize handlers
    let event_handler = Arc::new(
        MarketplaceEventHandler::new(approver, true)
    );

    let usage_handler = Arc::new(
        UsageTrackingHandler::new(
            reporter,
            "my-service.googleapis.com".to_string(),
        )
    );

    // Consume events with usage tracking
    consumer.consume(|event| {
        let event_handler = Arc::clone(&event_handler);
        let usage_handler = Arc::clone(&usage_handler);

        async move {
            // Handle entitlement event
            event_handler.handle(event.clone()).await?;

            // Track usage
            usage_handler
                .track_entitlement_usage(
                    &event,
                    &event.entitlement,  // Would be account name in real usage
                    OperationType::ProvisionEntitlement,
                )
                .await?;

            Ok(())
        }
    })
    .await?;

    Ok(())
}
```

## Metric Types

The following metric types are supported:

- `MetricType::ActiveUsers` - Count of active users/seats
- `MetricType::ApiCalls` - Number of API calls made
- `MetricType::DataProcessedGb` - Data processed in GB
- `MetricType::SupportIncidents` - Number of support cases
- `MetricType::Custom(String)` - Custom metric name

## Operation Types

Track different operation types for billing:

- `OperationType::ProvisionEntitlement` - Initial provision
- `OperationType::ModifyEntitlement` - Plan changes, upgrades/downgrades
- `OperationType::CancelEntitlement` - Cancellation/termination
- `OperationType::Custom(String)` - Custom operation types

## Labels and Metadata

Add custom labels to operations for filtering and analysis:

```rust
let usage = OperationUsage::new(...)
    .with_label("tier".to_string(), "premium".to_string())
    .with_label("region".to_string(), "us-west2".to_string())
    .with_user_id("customer-id@example.com".to_string());
```

## Error Handling

The `UsageReporter` trait returns `UsageReporterResult<T>` with specific error types:

```rust
use osiris_marketplace::port::UsageReporterError;

match reporter.report_operation(&usage).await {
    Ok(report) => println!("Reported successfully: {:?}", report),
    Err(UsageReporterError::AuthenticationError(e)) => {
        eprintln!("Authentication failed: {}", e);
    }
    Err(UsageReporterError::RateLimitError(e)) => {
        eprintln!("Rate limit exceeded: {}", e);
        // Implement retry logic with backoff
    }
    Err(UsageReporterError::ServiceUnavailable(e)) => {
        eprintln!("Service temporarily unavailable: {}", e);
        // Implement retry logic
    }
    Err(e) => eprintln!("Error reporting usage: {}", e),
}
```

## Best Practices

### 1. Batch Operations

For better performance, collect multiple operations and report them in batches:

```rust
let mut operations = Vec::new();

// Collect operations
for event in events {
    let usage = create_usage_from_event(&event);
    operations.push(usage);
}

// Report all at once
reporter.report_batch(&operations).await?;
```

### 2. Verify Credentials on Startup

Always verify credentials when initializing:

```rust
let reporter = ServiceControlReporter::with_default_credentials(
    service_name.clone(),
    project_id.clone(),
)
.await?;

reporter.verify_credentials().await?;
```

### 3. Implement Retry Logic

Handle transient failures:

```rust
use std::time::Duration;

async fn report_with_retry(
    reporter: &ServiceControlReporter,
    usage: &OperationUsage,
    max_retries: u32,
) -> Result<UsageReport, Box<dyn std::error::Error>> {
    let mut retries = 0;
    loop {
        match reporter.report_operation(usage).await {
            Ok(report) => return Ok(report),
            Err(e) if retries < max_retries => {
                retries += 1;
                let backoff = Duration::from_secs(2_u64.pow(retries));
                tokio::time::sleep(backoff).await;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }
}
```

### 4. Log Usage Events

Enable tracing for debugging:

```rust
use tracing_subscriber;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
}
```

## Testing

Mock the `UsageReporter` trait for testing:

```rust
use async_trait::async_trait;
use osiris_marketplace::port::{UsageReporter, UsageReporterResult};

struct MockReporter;

#[async_trait]
impl UsageReporter for MockReporter {
    async fn report_operation(
        &self,
        usage: &OperationUsage,
    ) -> UsageReporterResult<UsageReport> {
        Ok(UsageReport {
            service_name: "test".to_string(),
            operation_ids: vec![usage.operation_id.clone()],
            report_timestamp: chrono::Utc::now(),
            success: true,
            error_message: None,
        })
    }

    // ... implement other trait methods
}
```

## Troubleshooting

### Authentication Failures

**Error**: `AuthenticationError: Failed to authenticate`

**Solution**:
- Verify `GOOGLE_APPLICATION_CREDENTIALS` points to a valid service account key
- Ensure service account has `servicecontrol.metricReporter` permission
- Check that the JSON key file is readable

### Rate Limiting

**Error**: `RateLimitError: API rate limit exceeded`

**Solution**:
- Implement exponential backoff retry logic
- Batch multiple operations in a single report
- Contact Google Cloud support to increase rate limits

### Service Unavailable

**Error**: `ServiceUnavailable: Service temporarily unavailable`

**Solution**:
- Implement retry logic with exponential backoff
- Queue failed reports for retry
- Monitor Service Control API status

## References

- [Google Cloud Service Control API Documentation](https://cloud.google.com/service-management/reference/rest)
- [Cloud Commerce Partner Procurement API](https://cloud.google.com/marketplace/docs/partners)
- [Service Control Quota and Limits](https://cloud.google.com/service-management/quotas)
