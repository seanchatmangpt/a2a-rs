# A2A-RS Examples

This directory contains example programs demonstrating the a2a-rs library capabilities.

## Push Notification Examples

### Enhanced Push Notification Demo

**File:** `push_notification_enhanced_demo.rs`

Demonstrates the complete enhanced push notification delivery system with:
- HTTP webhook delivery with configurable retries
- HMAC-SHA256 signature generation for secure webhooks
- Dead letter queue for failed notifications
- Delivery status tracking and monitoring
- Event deduplication to prevent duplicate notifications

**Run:**
```bash
cargo run --example push_notification_enhanced_demo --all-features
```

**Features:**
- Sends real HTTP webhooks (to httpbin.org for testing)
- Configurable retry logic with exponential backoff
- Webhook signature generation
- Dead letter queue management
- Delivery tracking inspection
- Event deduplication demonstration

## Other Examples

- `http_client_server.rs` - Basic HTTP client and server example
- `websocket_client_server.rs` - WebSocket client and server example
- `sqlx_storage_demo.rs` - Database storage example
- `firewall_demo.rs` - Jidoka firewall demonstration
- `receipt_demo.rs` - Digital receipt generation
- `auth_demo.rs` - Authentication examples (JWT, OAuth2)
