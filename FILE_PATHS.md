# Enhanced Push Notification - File Paths Reference

## Implementation Files

### Core Implementation
- `/Users/sac/a2a-rs/a2a-rs/src/adapter/business/push_notification_enhanced.rs`
  - 810 lines
  - EnhancedHttpPushNotificationSender, InMemoryDeliveryTracker, InMemoryDeadLetterQueue

### Notification Manager
- `/Users/sac/a2a-rs/a2a-rs/src/adapter/business/enhanced_notification_manager.rs`
  - 320 lines
  - EnhancedNotificationManager, EnhancedHttpNotificationSender

### Module Exports
- `/Users/sac/a2a-rs/a2a-rs/src/adapter/mod.rs` (modified)
- `/Users/sac/a2a-rs/a2a-rs/src/adapter/business/mod.rs` (modified)

### Configuration
- `/Users/sac/a2a-rs/a2a-rs/Cargo.toml` (modified)
  - Added hmac dependency
  - Added push_notification_enhanced_demo example

## Documentation Files

- `/Users/sac/a2a-rs/docs/push_notification_delivery.md`
  - Complete usage guide with examples

- `/Users/sac/a2a-rs/docs/push_notification_implementation_summary.md`
  - Technical implementation details

- `/Users/sac/a2a-rs/docs/IMPLEMENTATION_COMPLETE.md`
  - Success criteria and verification

## Example Files

- `/Users/sac/a2a-rs/examples/push_notification_enhanced_demo.rs`
  - Complete demo of all features

- `/Users/sac/a2a-rs/examples/README.md`
  - Examples overview

## Reference Files

- `/Users/sac/a2a-rs/ENHANCED_PUSH_NOTIFICATION_TREE.txt`
  - File structure overview

## Related Existing Files (Unchanged)

- `/Users/sac/a2a-rs/a2a-rs/src/adapter/business/push_notification.rs`
  - Base PushNotificationSender trait

- `/Users/sac/a2a-rs/a2a-rs/src/domain/core/agent.rs`
  - PushNotificationConfig domain type

- `/Users/sac/a2a-rs/a2a-rs/src/domain/events/task_events.rs`
  - TaskStatusUpdateEvent, TaskArtifactUpdateEvent

- `/Users/sac/a2a-rs/a2a-rs/src/port/notification_manager.rs`
  - AsyncNotificationManager port trait

## Usage

### Import Enhanced Types
```rust
use a2a_rs::adapter::{
    EnhancedHttpPushNotificationSender,
    HttpPushNotificationConfig,
    InMemoryDeliveryTracker,
    InMemoryDeadLetterQueue,
    DeliveryStatus,
    EnhancedNotificationManager,
};
```

### Run Demo
```bash
cargo run --example push_notification_enhanced_demo --all-features
```

### Run Tests
```bash
cargo test -p a2a-rs push_notification_enhanced --all-features
```
