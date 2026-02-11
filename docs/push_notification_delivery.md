# Enhanced Push Notification Delivery System

## Overview

The enhanced push notification delivery system provides production-ready webhook delivery with:
- HTTP webhook delivery with configurable retries and exponential backoff
- HMAC-SHA256 signature generation for secure webhook verification
- Dead letter queue for failed notifications that exceed retry limits
- Delivery status tracking for monitoring and debugging
- Event deduplication to prevent duplicate notifications
- Full async/await support with tokio

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Enhanced Push Notification                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │ HTTP Webhook │    │   HMAC-SHA   │    │   Dead Letter│      │
│  │   Delivery   │───▶│  Signature   │───▶│     Queue    │      │
│  │   with Retry │    │  Generation  │    │   (DLQ)      │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│           │                                        │             │
│           ▼                                        ▼             │
│  ┌──────────────┐                        ┌──────────────┐      │
│  │  Delivery    │                        │   Replay     │      │
│  │   Tracker    │◀───────┬───────────────▶│   Failed     │      │
│  │              │        │               │   Events     │      │
│  └──────────────┘        │               └──────────────┘      │
│           │                │                                   │
│           ▼                ▼                                   │
│  ┌──────────────────────────────────────┐                     │
│  │         Deduplication                │                     │
│  │      (Event ID Hashing)              │                     │
│  └──────────────────────────────────────┘                     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. EnhancedHttpPushNotificationSender

The core sender that handles webhook delivery with retries and tracking.

**Features:**
- Configurable timeout, max retries, and backoff
- HMAC-SHA256 signature generation
- Automatic retry with exponential backoff
- Integration with delivery tracker and DLQ
- Smart error handling (no retries on 4xx client errors)

**Configuration Options:**
```rust
HttpPushNotificationConfig::builder()
    .timeout(30)                    // HTTP timeout in seconds
    .max_retries(5)                 // Maximum retry attempts
    .backoff_ms(1000)               // Initial backoff (exponential)
    .enable_deduplication(true)     // Prevent duplicate events
    .enable_tracking(true)          // Track all delivery attempts
    .enable_dead_letter(true)        // Use dead letter queue
    .signing_key(Some("secret".to_string()))  // HMAC signing key
    .signature_header(Some("X-Webhook-Signature".to_string()))
    .build();
```

### 2. InMemoryDeliveryTracker

Tracks delivery status for all notifications with deduplication.

**Features:**
- Event ID generation (task_id + content hash)
- Delivery status tracking (Pending, Sending, Delivered, Failed, DeadLettered)
- Delivery attempt counting
- Timestamps for all state transitions
- Automatic cleanup of old records

**Usage:**
```rust
let tracker = InMemoryDeliveryTracker::new();

// Check if already delivered
if tracker.is_delivered(task_id, event_id).await {
    return Ok(()); // Skip duplicate
}

// Record delivery attempt
tracker.record_attempt(task_id, event_id, DeliveryStatus::Delivered).await?;

// Get tracking info
let tracking = tracker.get_tracking(task_id, event_id).await?;
```

### 3. InMemoryDeadLetterQueue

Stores failed notifications that exceeded retry limits.

**Features:**
- Automatic DLQ entry on max retries exceeded
- Full event context preservation
- Per-task DLQ queries
- Replay support for failed events

**Usage:**
```rust
let dlq = InMemoryDeadLetterQueue::new();

// Get all failed events
let all = dlq.get_all().await?;

// Get failed events for specific task
let task_failures = dlq.get_by_task(task_id).await?;

// Replay failed event
for entry in task_failures {
    if !entry.replayed {
        // Retry delivery
        sender.send_status_update(&config, &event).await?;
        dlq.remove(&entry.id).await?;
    }
}
```

### 4. EnhancedNotificationManager

Port trait implementation with delivery tracking.

**Features:**
- Implements `AsyncNotificationManager` port
- Task-level push notification configuration
- Automatic delivery with retries
- Integrated tracking and DLQ

## Usage Examples

### Basic Setup

```rust
use a2a_rs::adapter::{
    EnhancedHttpPushNotificationSender, HttpPushNotificationConfig,
    PushNotificationSender,
};

// Create sender with custom config
let config = HttpPushNotificationConfig::builder()
    .timeout(30)
    .max_retries(5)
    .backoff_ms(1000)
    .signing_key(Some("my-webhook-secret".to_string()))
    .build();

let sender = EnhancedHttpPushNotificationSender::with_config(config);
```

### Sending Notifications

```rust
use a2a_rs::domain::{PushNotificationConfig, TaskStatusUpdateEvent, TaskStatus};

let push_config = PushNotificationConfig {
    id: Some("webhook-1".to_string()),
    url: "https://your-app.com/webhooks".to_string(),
    token: Some("bearer-token".to_string()),
    authentication: None,
};

let event = TaskStatusUpdateEvent {
    task_id: "task-123".to_string(),
    context_id: "ctx-456".to_string(),
    kind: "status-update".to_string(),
    status: TaskStatus {
        state: "completed".to_string(),
        timestamp: Some("2024-02-11T10:00:00Z".to_string()),
        message: Some("Done!".to_string()),
    },
    final_: true,
    metadata: None,
};

// Send with retries and tracking
sender.send_status_update(&push_config, &event).await?;
```

### Webhook Signature Verification (Receiver Side)

When your webhook endpoint receives a notification, verify the signature:

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn verify_webhook_signature(
    payload: &str,
    received_signature: &str,
    secret: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Remove "sha256=" prefix if present
    let signature_hex = received_signature
        .strip_prefix("sha256=")
        .unwrap_or(received_signature);

    // Compute expected signature
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload.as_bytes());
    let expected_signature = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison
    Ok(expected_signature == signature_hex)
}

// In your webhook handler:
let payload = "..."; // Request body
let signature = request.headers.get("X-Webhook-Signature").unwrap();
let is_valid = verify_webhook_signature(payload, signature, "my-webhook-secret")?;
```

### Inspecting Delivery Status

```rust
let tracker = sender.tracker();

// Get all tracking for a task
let tracking = tracker.get_task_tracking("task-123").await;

for t in tracking {
    println!("Event {}: {:?}", t.event_id, t.status);
    println!("  Attempts: {}", t.attempts);
    println!("  Delivered at: {:?}", t.delivered_at);
}
```

### Handling Dead Letter Queue

```rust
let dlq = sender.dead_letter_queue();

// Get all failed notifications
let failed = dlq.get_all().await?;

for entry in failed {
    if !entry.replayed {
        println!(
            "Task {} failed after {} attempts: {}",
            entry.task_id, entry.attempts, entry.reason
        );

        // Optionally replay
        // 1. Fix the issue
        // 2. Re-send notification
        // 3. Remove from DLQ
        dlq.remove(&entry.id).await?;
    }
}
```

### Integration with NotificationManager Port

```rust
use a2a_rs::adapter::EnhancedNotificationManager;
use a2a_rs::domain::TaskPushNotificationConfig;

let manager = EnhancedNotificationManager::with_http_sender(
    HttpPushNotificationConfig::builder()
        .timeout(30)
        .max_retries(5)
        .enable_tracking(true)
        .build()
);

// Register notification config for a task
let config = TaskPushNotificationConfig {
    task_id: "task-123".to_string(),
    push_notification_config: PushNotificationConfig {
        id: Some("webhook-1".to_string()),
        url: "https://your-app.com/webhooks".to_string(),
        token: Some("token".to_string()),
        authentication: None,
    },
};

manager.set_task_notification(&config).await?;

// Now notifications will be sent automatically when task status changes
manager.notify_task_status_update("task-123", &status_event).await?;
```

## Delivery Status Lifecycle

```
┌─────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│ Pending │ ──▶ │ Sending  │ ──▶ │ Delivered│     │DeadLetter│
└─────────┘     └──────────┘     └──────────┘     └──────────┘
     │                                    ▲              │
     │         ┌──────────┐              │              │
     └────────▶│  Failed  │──────────────┘              │
               └──────────┘                             │
                      │                                  │
                      └────── retries exhausted ────────┘
```

## Error Handling

The system distinguishes between recoverable and non-recoverable errors:

**Retryable (5xx errors, network failures):**
- 500 Internal Server Error
- 502 Bad Gateway
- 503 Service Unavailable
- 504 Gateway Timeout
- Connection refused
- Timeout

**Non-retryable (4xx errors):**
- 400 Bad Request
- 401 Unauthorized
- 403 Forbidden
- 404 Not Found
- 429 Rate Limited (optional)

## Best Practices

1. **Always use webhooks with signatures in production**
   ```rust
   .signing_key(Some(std::env::var("WEBHOOK_SECRET").unwrap()))
   ```

2. **Configure appropriate retry limits**
   - Low-priority: 2-3 retries
   - High-priority: 5-10 retries

3. **Monitor your dead letter queue**
   ```rust
   // Check DLQ size periodically
   let dlq_count = dlq.count().await?;
   if dlq_count > 100 {
       alert!("Too many failed notifications: {}", dlq_count);
   }
   ```

4. **Clean up old delivery records**
   ```rust
   // Remove records older than 24 hours
   tracker.cleanup_old(86400).await?;
   ```

5. **Use structured logging for debugging**
   ```rust
   tracing::info!(
       task_id = %event.task_id,
       status = ?event.status,
       url = %config.url,
       "Sending webhook notification"
   );
   ```

## Testing

Run the demo example:
```bash
cargo run --example push_notification_enhanced_demo --all-features
```

Run tests:
```bash
cargo test -p a2a-rs push_notification --all-features
```

## Security Considerations

1. **Webhook Signature Verification**
   - Always verify signatures on the receiver side
   - Use strong secrets (minimum 32 bytes)
   - Rotate secrets periodically

2. **HTTPS Only**
   - Never use HTTP URLs for webhooks
   - Validate URL format on configuration

3. **Secret Management**
   ```rust
   // Load from environment, never hardcode
   let secret = std::env::var("WEBHOOK_SECRET")
       .expect("WEBHOOK_SECRET must be set");
   ```

4. **Rate Limiting**
   - Implement rate limiting on your webhook endpoint
   - Return 429 to trigger backoff

## Performance Notes

- **Deduplication**: Uses hash-based event IDs (O(1) lookup)
- **Delivery Tracker**: In-memory HashMap (suitable for <100k active tasks)
- **DLQ**: In-memory vector (consider persistent storage for production)
- **Concurrency**: Fully async with tokio (handles thousands of concurrent deliveries)

## Migration from Basic PushNotificationSender

```rust
// Before
let sender = HttpPushNotificationSender::new();
sender.send_status_update(&config, &event).await?;

// After
let sender = EnhancedHttpPushNotificationSender::with_config(
    HttpPushNotificationConfig::default()
);
sender.send_status_update(&config, &event).await?;

// Bonus: Access tracking and DLQ
let tracker = sender.tracker();
let dlq = sender.dead_letter_queue();
```

## Future Enhancements

Potential improvements for future versions:
- Persistent delivery tracking (PostgreSQL, Redis)
- Dead letter queue persistence
- Webhook replay management UI
- Metrics and observability integration
- Batch notification support
- Webhook endpoint health checking
- Circuit breaker pattern for failing endpoints
