# Google Cloud Service Control Integration

## Overview

This document describes the implementation of Google Cloud Service Control integration for osiris-marketplace, enabling usage-based billing for Cloud Marketplace products.

## Architecture

The implementation follows hexagonal architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                          │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ UsageTrackingHandler                                 │   │
│  │ - Orchestrates usage tracking across operations     │   │
│  │ - Coordinates with UsageReporter port               │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ depends on
                           │
┌─────────────────────────────────────────────────────────────┐
│                     Port Layer                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ UsageReporter (async trait)                          │   │
│  │ - report_operation()                                 │   │
│  │ - report_batch()                                     │   │
│  │ - verify_credentials()                               │   │
│  │ - get_service_name() / get_project_id()              │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ implements
                           │
┌─────────────────────────────────────────────────────────────┐
│                    Adapter Layer                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ServiceControlReporter                               │   │
│  │ - Uses google-servicecontrol1 crate                 │   │
│  │ - Handles Service Control API calls                  │   │
│  │ - Manages OAuth2 authentication                      │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ uses
                           │
┌─────────────────────────────────────────────────────────────┐
│                    Domain Layer                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ OperationUsage, UsageMetric, UsageReport             │   │
│  │ OperationType, MetricType                             │   │
│  │ Pure types with no external dependencies             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## File Structure

### Domain Layer (`domain/usage.rs`)

Pure Rust types with no external dependencies:

- **`OperationType`** - Enum defining operation kinds:
  - `ProvisionEntitlement` - Initial provisioning
  - `ModifyEntitlement` - Plan changes
  - `CancelEntitlement` - Termination
  - `Custom(String)` - Custom operation types

- **`MetricType`** - Enum defining billable metrics:
  - `ActiveUsers` - Count of active users/seats
  - `ApiCalls` - API call volume
  - `DataProcessedGb` - Data processing volume
  - `SupportIncidents` - Support case count
  - `Custom(String)` - Custom metrics

- **`UsageMetric`** - Single metric value:
  ```rust
  pub struct UsageMetric {
      pub metric_type: MetricType,
      pub value: i64,
  }
  ```

- **`OperationUsage`** - Complete operation record:
  ```rust
  pub struct OperationUsage {
      pub operation_id: String,
      pub operation_type: OperationType,
      pub entitlement: String,
      pub account: String,
      pub operation_timestamp: DateTime<Utc>,
      pub service_name: String,
      pub metrics: Vec<UsageMetric>,
      pub labels: HashMap<String, String>,
      pub user_id: Option<String>,
  }
  ```

- **`UsageReport`** - Report confirmation:
  ```rust
  pub struct UsageReport {
      pub service_name: String,
      pub operation_ids: Vec<String>,
      pub report_timestamp: DateTime<Utc>,
      pub success: bool,
      pub error_message: Option<String>,
  }
  ```

### Port Layer (`port/usage_reporter.rs`)

Defines the `UsageReporter` async trait with methods:

```rust
pub trait UsageReporter: Send + Sync {
    async fn report_operation(&self, usage: &OperationUsage)
        -> UsageReporterResult<UsageReport>;

    async fn report_batch(&self, usages: &[OperationUsage])
        -> UsageReporterResult<UsageReport>;

    async fn verify_credentials(&self) -> UsageReporterResult<()>;

    fn get_service_name(&self) -> &str;
    fn get_project_id(&self) -> &str;
}
```

Error types:
- `AuthenticationError` - Credential/auth failures
- `NotFound` - Operation not found
- `InvalidUsage` - Malformed usage data
- `RequestError` - HTTP/API failures
- `ParseError` - Response parsing failures
- `RateLimitError` - API rate limiting
- `ServiceUnavailable` - Transient service issues
- `Other` - Generic errors

### Adapter Layer (`adapter/service_control.rs`)

`ServiceControlReporter` implementation using google-servicecontrol1 crate:

**Authentication:**
```rust
// Option 1: Explicit credentials file
let reporter = ServiceControlReporter::new(
    service_name,
    project_id,
    "/path/to/service-account.json"
).await?;

// Option 2: Default credentials (Application Default Credentials)
let reporter = ServiceControlReporter::with_default_credentials(
    service_name,
    project_id
).await?;
```

**Implementation details:**
- Handles OAuth2 authentication via `yup-oauth2` and `ServiceAccountAuthenticator`
- Converts `OperationUsage` to `google_servicecontrol1::api::Operation` format
- Adds operation metadata with entitlement, account, operation type, and user ID
- Converts metrics to JSON for storage in operation metadata
- Implements batch reporting for efficiency
- Proper error handling and retries

### Application Layer (`application/usage_handler.rs`)

`UsageTrackingHandler` orchestrates usage tracking:

```rust
pub struct UsageTrackingHandler<U: UsageReporter> {
    usage_reporter: Arc<U>,
    service_name: String,
}

impl<U: UsageReporter> UsageTrackingHandler<U> {
    pub async fn track_entitlement_usage(
        &self,
        event: &EntitlementEvent,
        account_name: &str,
        operation_type: OperationType,
    ) -> Result<UsageReport, Box<dyn std::error::Error + Send + Sync>>;

    pub async fn track_batch_usage(
        &self,
        usages: &[OperationUsage],
    ) -> Result<UsageReport, Box<dyn std::error::Error + Send + Sync>>;

    pub async fn verify_reporter_credentials(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
```

## Feature Flag

The service-control integration is feature-gated:

```toml
[features]
service-control = ["google-servicecontrol1", "yup-oauth2"]
full = ["pubsub", "procurement-api", "service-control", "server"]
```

Enable with:
```bash
cargo build --features service-control
cargo build --all-features
```

## Dependencies

Core dependencies:
- `google-servicecontrol1 = "5.0"` - Google Service Control API client
- `yup-oauth2 = "8"` - OAuth2 authentication
- `async-trait = "0.1"` - Async trait support
- `thiserror = "2"` - Error types
- `chrono = "0.4"` - Timestamps
- `uuid = "1"` - Operation ID generation
- `serde_json = "1"` - JSON handling
- `tracing = "0.1"` - Structured logging

## Usage Examples

### Single Operation Reporting

```rust
use osiris_marketplace::{
    adapter::ServiceControlReporter,
    domain::{MetricType, OperationType, OperationUsage, UsageMetric},
};

let reporter = ServiceControlReporter::with_default_credentials(
    "my-service.googleapis.com".to_string(),
    "project-id".to_string(),
).await?;

let usage = OperationUsage::new(
    "op-123".to_string(),
    OperationType::ProvisionEntitlement,
    "providers/my/entitlements/1".to_string(),
    "providers/my/accounts/2".to_string(),
    "my-service.googleapis.com".to_string(),
)
.add_metric(UsageMetric::new(MetricType::ActiveUsers, 100));

let report = reporter.report_operation(&usage).await?;
```

### Batch Reporting

```rust
let usages = vec![usage1, usage2, usage3];
let report = reporter.report_batch(&usages).await?;
```

### Event-based Integration

```rust
use osiris_marketplace::{
    adapter::ServiceControlReporter,
    application::UsageTrackingHandler,
};

let reporter = ServiceControlReporter::with_default_credentials(
    service_name,
    project_id,
).await?;

let handler = UsageTrackingHandler::new(
    Arc::new(reporter),
    service_name,
);

// In event processing loop
handler.track_entitlement_usage(
    &event,
    &account_name,
    OperationType::ProvisionEntitlement,
).await?;
```

## Testing

Domain types are tested for:
- Serialization/deserialization
- Builder pattern correctness
- Field defaults

Application logic is tested with mock `UsageReporter`:

```rust
struct MockReporter;

#[async_trait]
impl UsageReporter for MockReporter {
    async fn report_operation(&self, usage: &OperationUsage)
        -> UsageReporterResult<UsageReport> {
        Ok(UsageReport {
            service_name: "test".to_string(),
            operation_ids: vec![usage.operation_id.clone()],
            report_timestamp: Utc::now(),
            success: true,
            error_message: None,
        })
    }
    // ... other methods
}
```

Run tests with:
```bash
cargo test -p osiris-marketplace --all-features
```

## Integration Points

### With Event Handler

The `UsageTrackingHandler` can be integrated with the existing `MarketplaceEventHandler`:

```rust
let event_handler = Arc::new(MarketplaceEventHandler::new(approver, auto_approve));
let usage_handler = Arc::new(UsageTrackingHandler::new(reporter, service_name));

consumer.consume(|event| {
    let eh = Arc::clone(&event_handler);
    let uh = Arc::clone(&usage_handler);

    async move {
        eh.handle(event.clone()).await?;
        uh.track_entitlement_usage(&event, account, op_type).await?;
        Ok(())
    }
}).await?;
```

### Error Handling

Errors should be handled gracefully with retry logic:

```rust
use tokio::time::{sleep, Duration};

async fn report_with_retry(
    reporter: &ServiceControlReporter,
    usage: &OperationUsage,
) -> Result<UsageReport, Box<dyn std::error::Error>> {
    for attempt in 0..3 {
        match reporter.report_operation(usage).await {
            Ok(report) => return Ok(report),
            Err(UsageReporterError::RateLimitError(_)) if attempt < 2 => {
                let backoff = Duration::from_secs(2_u64.pow(attempt));
                sleep(backoff).await;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }
    Err("Max retries exceeded".into())
}
```

## GCP Permissions

Service account requires these IAM roles:
- `roles/servicemanagement.metricWriter` - Write usage metrics
- `roles/servicecontrol.quotaEditor` - For quota management (optional)

Or the custom role with these permissions:
```
servicecontrol.services.report
servicecontrol.services.reportApiKeyInfo
```

## Monitoring and Observasting

Enable tracing to monitor usage reporting:

```rust
use tracing::{info, debug, warn};

tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .with_writer(std::io::stderr)
    .init();
```

Traces include:
- `info!` - Report submissions, verification
- `debug!` - Operation details, API calls
- `warn!` - Failures, rate limits

## Production Considerations

1. **Batching**: Always batch operations when possible for efficiency
2. **Retry Logic**: Implement exponential backoff for transient failures
3. **Monitoring**: Log all reporting operations and failures
4. **Credentials**: Use Application Default Credentials in cloud environments
5. **Rate Limiting**: Be aware of Service Control API quotas
6. **Queuing**: Consider persistent queues for critical usage reports

## References

- [Google Cloud Service Control API](https://cloud.google.com/service-management/reference/rest)
- [Service Control Quotas](https://cloud.google.com/service-management/quotas)
- [Cloud Marketplace Partner Guide](https://cloud.google.com/marketplace/docs/partners)
- [google-servicecontrol1 Rust Crate](https://docs.rs/google-servicecontrol1/)
- [yup-oauth2 Rust Crate](https://docs.rs/yup-oauth2/)

## Examples

See `examples/usage_reporting_example.rs` for a complete working example demonstrating:
- Reporter initialization
- Single operation reporting
- Batch operation reporting
- Custom metrics and labels
- Error handling patterns

Run with:
```bash
export GCP_PROJECT_ID=my-project
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
cargo run --example usage_reporting_example --all-features
```
