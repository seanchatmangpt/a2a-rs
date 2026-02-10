# osiris-marketplace

Google Cloud Marketplace Partner Integration for the a2a-rs workspace.

## Overview

This crate implements a hexagonal architecture for integrating with the Google Cloud Marketplace Partner APIs:

- **Consume entitlement events** from Partner Pub/Sub topic
- **Parse ENTITLEMENT_OFFER_ACCEPTED events** and extract account information
- **Approve account resources** via Partner Procurement API

## Architecture

Following the hexagonal (ports & adapters) pattern:

- **`domain/`** - Core types: `EntitlementEvent`, `Account`, `Entitlement`, `AccountState`
- **`port/`** - Trait definitions: `EventConsumer`, `AccountApprover`
- **`adapter/`** - Implementations:
  - `PubSubConsumer` - Google Cloud Pub/Sub adapter
  - `ProcurementApiClient` - Partner Procurement API adapter

## Feature Flags

| Feature | Description |
|---------|-------------|
| `pubsub` | Enable Google Cloud Pub/Sub consumer adapter |
| `procurement-api` | Enable Procurement API client adapter |
| `full` | Enable all features |

## Usage

```toml
[dependencies]
osiris-marketplace = { path = "../osiris-marketplace", features = ["full"] }
```

### Consuming Events and Approving Accounts

```rust
use osiris_marketplace::{
    domain::{ApproveAccountRequest, EntitlementEventType},
    adapter::{ProcurementApiClient, PubSubConsumer},
    port::{AccountApprover, EventConsumer},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Pub/Sub consumer
    let consumer = PubSubConsumer::new(
        "my-gcp-project".to_string(),
        "marketplace-events-sub".to_string(),
    ).await?;

    // Initialize Procurement API client
    let approver = ProcurementApiClient::new(
        "my-gcp-project".to_string(),
        std::env::var("GCP_ACCESS_TOKEN")?,
    )?;

    // Consume events and auto-approve accounts
    consumer.consume(|event| {
        let approver = approver.clone();
        async move {
            if event.event_type == EntitlementEventType::EntitlementOfferAccepted {
                // Fetch entitlement details
                let entitlement = approver.get_entitlement(&event.entitlement).await?;
                
                // Fetch associated account
                let account = approver.get_account(&entitlement.account).await?;
                
                // Approve the account
                let request = ApproveAccountRequest::default();
                approver.approve_account(&account.name, &request).await?;
                
                println!("Approved account: {}", account.name);
            }
            Ok(())
        }
    }).await?;

    Ok(())
}
```

## Google Cloud Setup

### 1. Create Pub/Sub Subscription

The Partner Pub/Sub topic is automatically created by Google Cloud Marketplace. You need to create a subscription:

```bash
gcloud pubsub subscriptions create marketplace-events-sub \
  --topic=projects/cloudcommerceproc-prod/topics/providers/{provider-id} \
  --project=my-gcp-project
```

### 2. Enable Procurement API

```bash
gcloud services enable cloudcommerceprocurement.googleapis.com \
  --project=my-gcp-project
```

### 3. Service Account Permissions

Your service account needs:
- `roles/pubsub.subscriber` for the subscription
- `cloudcommerceprocurement.accounts.approve` permission

## API Reference

### Cloud Commerce Partner Procurement API

- Base URL: `https://cloudcommerceprocurement.googleapis.com/v1`
- Documentation: https://cloud.google.com/marketplace/docs/partners/commerce-procurement-api/reference/rest

### Key Operations

- `GET /providers/{provider}/entitlements/{entitlement}` - Get entitlement
- `GET /providers/{provider}/accounts/{account}` - Get account
- `POST /providers/{provider}/accounts/{account}/approved:approve` - Approve account
- `POST /providers/{provider}/accounts/{account}/rejected:reject` - Reject account

## Testing

```bash
# Run tests with all features
cargo test -p osiris-marketplace --all-features

# Run with specific feature
cargo test -p osiris-marketplace --features pubsub
```

## License

Same as parent workspace.
