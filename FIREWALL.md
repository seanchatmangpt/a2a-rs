# Life Firewall Admission Control System

A production-grade admission control system for a2a-rs implementing WIP token limiting, supplier quality tracking, and Jidoka-style quality gates.

## Architecture Overview

Following hexagonal architecture principles:

```
domain/core/firewall.rs     → Domain types (WorkPacket, RefusalReceipt, etc.)
port/admission.rs           → Port trait (AsyncAdmissionController)
adapter/business/firewall.rs → Adapter implementation (DefaultAdmissionController)
services/firewall.rs        → Service wrapper (FirewallService)
```

## Core Components

### 1. Domain Types (`domain/core/firewall.rs`)

#### IngressChannel
Classification for incoming work:
- **Batch**: Regular batched work with standard SLA
- **Scheduled**: Pre-scheduled work with committed delivery time
- **Emergency**: High-priority work requiring immediate attention

#### JidokaMode
Quality modes that gate admission:
- **GREEN**: Normal operation - all channels accepting work
- **YELLOW**: Degraded operation - emergency only
- **RED**: System halt - no new work accepted

#### WorkPacket
Represents a unit of work requesting admission:
```rust
WorkPacket {
    id: String,
    objective: String,
    constraints: WorkConstraints,
    acceptance_test: String,
    reversibility: bool,
    channel: IngressChannel,
    supplier_id: Option<String>,
    priority: Option<u32>,
}
```

#### RefusalReceipt
Documents work refusal with structured reasons:
```rust
RefusalReceipt {
    work_packet_id: String,
    refused_at: String,
    reason: RefusalReason,
    system_health: SystemHealth,
    message: Option<String>,
}
```

#### RefusalReason
Structured error reasons:
- `WipLimitExceeded`: Backpressure from token exhaustion
- `LowSupplierQuality`: Supplier quality score below threshold
- `JidokaModeRestriction`: Jidoka mode prevents admission
- `ResourceConstraintsUnsatisfiable`: Cannot satisfy constraints
- `ValidationFailure`: Missing fields or validation error

#### SupplierQuality
Tracks defect rates per supplier:
```rust
SupplierQuality {
    supplier_id: String,
    total_submitted: u64,
    successful: u64,
    defects: u64,
    quality_score: f64,  // 0.0-1.0
    last_updated: String,
}
```

### 2. Port Trait (`port/admission.rs`)

#### AsyncAdmissionController
Defines the admission control contract:

```rust
#[async_trait]
pub trait AsyncAdmissionController: Send + Sync {
    async fn request_admission(&self, work_packet: WorkPacket)
        -> Result<AdmissionDecision, A2AError>;

    async fn get_system_health(&self) -> Result<SystemHealth, A2AError>;

    async fn get_supplier_quality(&self, supplier_id: &str)
        -> Result<SupplierQuality, A2AError>;

    async fn set_jidoka_mode(&self, mode: JidokaMode)
        -> Result<(), A2AError>;

    async fn complete_work(&self, work_packet_id: &str, success: bool)
        -> Result<(), A2AError>;

    async fn get_wip_count(&self) -> Result<usize, A2AError>;
    async fn get_wip_limit(&self) -> Result<usize, A2AError>;
    async fn set_wip_limit(&self, limit: usize) -> Result<(), A2AError>;
}
```

### 3. Adapter Implementation (`adapter/business/firewall.rs`)

#### DefaultAdmissionController
In-memory implementation with configurable limits:

```rust
pub struct DefaultAdmissionController {
    state: Arc<RwLock<AdmissionState>>,
}

pub struct AdmissionConfig {
    pub max_wip: usize,
    pub min_supplier_quality: f64,
    pub initial_jidoka_mode: JidokaMode,
}
```

**Admission Decision Flow:**
1. Validate work packet
2. Check Jidoka mode (quality gate)
3. Check WIP limit (backpressure)
4. Check supplier quality
5. Admit or refuse with receipt

### 4. Service Layer (`services/firewall.rs`)

#### FirewallService
High-level wrapper with metrics and queue support:

```rust
pub struct FirewallService<C: AsyncAdmissionController> {
    controller: Arc<C>,
    admission_tx: mpsc::Sender<AdmissionRequest>,
}

pub struct FirewallConfig {
    pub queue_size: usize,
    pub num_processors: usize,
}
```

## Usage Examples

### Basic Admission Control

```rust
use a2a_rs::{
    AdmissionConfig, DefaultAdmissionController,
    IngressChannel, JidokaMode, WorkPacket, WorkConstraints
};
use a2a_rs::port::AsyncAdmissionController;

// Create controller
let config = AdmissionConfig {
    max_wip: 10,
    min_supplier_quality: 0.6,
    initial_jidoka_mode: JidokaMode::Green,
};
let controller = DefaultAdmissionController::with_config(config);

// Request admission
let packet = WorkPacket {
    id: "work-1".to_string(),
    objective: "Process customer request".to_string(),
    constraints: WorkConstraints {
        max_execution_time_secs: 300,
        max_memory_bytes: Some(100 * 1024 * 1024),
        deadline: None,
    },
    acceptance_test: "Verify response format".to_string(),
    reversibility: true,
    channel: IngressChannel::Batch,
    supplier_id: Some("external-api".to_string()),
    priority: Some(10),
};

match controller.request_admission(packet).await? {
    AdmissionDecision::Admitted { work_packet_id, assigned_token_id, .. } => {
        println!("Admitted: {} (token: {})", work_packet_id, assigned_token_id);
    }
    AdmissionDecision::Refused { receipt } => {
        println!("Refused: {:?}", receipt.reason);
    }
}
```

### Using the Firewall Service

```rust
use a2a_rs::services::{FirewallService, FirewallConfig};

let service = FirewallService::new(
    controller,
    FirewallConfig::default()
);

// Request admission
let decision = service.request_admission(packet).await?;

// Get metrics
let metrics = service.get_metrics().await?;
println!("WIP: {}/{}", metrics.current_wip, metrics.max_wip);
println!("Quality: {:.2}", metrics.quality_score);
```

### Jidoka Mode Control

```rust
// Normal operation
service.set_jidoka_mode(JidokaMode::Green).await?;

// Degraded - emergency only
service.set_jidoka_mode(JidokaMode::Yellow).await?;

// Full halt
service.set_jidoka_mode(JidokaMode::Red).await?;
```

### Supplier Quality Tracking

```rust
// Complete work successfully
service.complete_work("work-1", true).await?;

// Complete with defect
service.complete_work("work-2", false).await?;

// Check quality
let quality = service.get_supplier_quality("external-api").await?;
println!("Quality score: {:.2}", quality.quality_score);
```

## Running the Demo

```bash
# Run the comprehensive demo
cargo run -p a2a-rs --example firewall_demo --features server
```

The demo demonstrates:
1. Basic admission
2. WIP limit enforcement (backpressure)
3. Jidoka mode Yellow (emergency only)
4. Supplier quality tracking
5. Jidoka mode Red (full halt)
6. Final metrics

## Testing

Comprehensive test coverage in `adapter/business/firewall.rs`:

```bash
# Run firewall tests (when other modules compile)
cargo test -p a2a-rs --lib firewall --features server
```

Test scenarios:
- ✅ Basic admission flow
- ✅ WIP limit enforcement
- ✅ Jidoka Yellow mode (emergency only)
- ✅ Jidoka Red mode (full halt)
- ✅ Supplier quality tracking
- ✅ Work completion releases tokens
- ✅ Validation errors

## Design Principles

### Hexagonal Architecture
- **Domain types**: Pure data structures, zero dependencies
- **Port traits**: Contract definitions, domain-only dependencies
- **Adapter implementations**: Concrete implementations with external dependencies
- **Service layer**: High-level orchestration

### Rust Conventions
- ✅ Edition 2024, MSRV 1.85
- ✅ All public types: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- ✅ JSON compatibility: `#[serde(rename_all = "camelCase")]`
- ✅ Error handling: `thiserror` for domain errors
- ✅ Async-first: `#[async_trait]` for all async traits
- ✅ No `unwrap()`/`expect()` in library code
- ✅ Feature-gated: `#[cfg(feature = "server")]`

### Production Quality
- **Type safety**: Strong typing for all domain concepts
- **Structured errors**: Machine-readable refusal reasons
- **Observability**: System health metrics, supplier quality tracking
- **Backpressure**: WIP token limiting prevents overload
- **Quality gates**: Jidoka modes for system protection
- **Testing**: Comprehensive test coverage

## Integration Points

### With A2A Protocol
The firewall can gate task admission:

```rust
// In task handler
let work_packet = WorkPacket::from_task(&task);
match firewall.request_admission(work_packet).await? {
    AdmissionDecision::Admitted { assigned_token_id, .. } => {
        // Process task with token
        task_manager.create_task(&task.id, &task.context_id).await?;
    }
    AdmissionDecision::Refused { receipt } => {
        // Return refusal to sender
        return Err(A2AError::ValidationError {
            field: "admission".to_string(),
            message: format!("Work refused: {:?}", receipt.reason),
        });
    }
}
```

### With Observability
All metrics exposed for monitoring:

```rust
let metrics = firewall.get_metrics().await?;
// Export to Prometheus, StatsD, etc.
```

## File Locations

- **Domain types**: `/home/user/a2a-rs/a2a-rs/src/domain/core/firewall.rs`
- **Port trait**: `/home/user/a2a-rs/a2a-rs/src/port/admission.rs`
- **Adapter**: `/home/user/a2a-rs/a2a-rs/src/adapter/business/firewall.rs`
- **Service**: `/home/user/a2a-rs/a2a-rs/src/services/firewall.rs`
- **Demo**: `/home/user/a2a-rs/a2a-rs/examples/firewall_demo.rs`

## Future Enhancements

- [ ] Persistent storage backend (SQLx adapter)
- [ ] Distributed WIP limiting (Redis-backed)
- [ ] Admission queue with priority scheduling
- [ ] Automatic Jidoka mode switching based on metrics
- [ ] Circuit breaker integration
- [ ] Rate limiting per supplier
- [ ] Time-windowed quality scoring
- [ ] Admission webhooks/notifications
