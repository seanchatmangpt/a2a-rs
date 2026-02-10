# A2A Orchestrator - Quick Start Guide

## 5-Minute Integration

### 1. Import the Orchestrator
```rust
use osiris_compiler::adapter::RemoteA2AOrchestratorAdapter;
use osiris_compiler::port::A2AOrchestratorPort;
use osiris_compiler::domain::OperationPayload;
```

### 2. Create an Orchestrator Instance
```rust
// Use defaults
let orchestrator = RemoteA2AOrchestratorAdapter::default();

// Or customize
use osiris_compiler::port::A2AOrchestratorConfig;

let config = A2AOrchestratorConfig {
    timeout_secs: 600,      // 10 minutes
    auto_retry: true,
    max_retries: 5,
    poll_interval_ms: 1000, // 1 second
    ..Default::default()
};
let orchestrator = RemoteA2AOrchestratorAdapter::new(config);
```

### 3. Submit a Compilation Task
```rust
let task = orchestrator.submit_task(
    "osiris-macos",                           // agent_id
    "https://agent.example.com/api",          // agent_url
    "my-compilation-session",                 // context_id
    OperationPayload::Compile {
        source: std::fs::read_to_string("main.rs")?,
        target: "aarch64-apple-darwin".to_string(),
        flags: Some(vec!["-O2".to_string()]),
        opt_level: 2,
    },
).await?;

println!("Task ID: {}", task.id);
```

### 4. Monitor Task Progress
```rust
// Option A: Stream updates (non-blocking)
let mut updates = orchestrator.stream_task_updates(&task).await?;

while let Some(event) = updates.next().await {
    println!("Event: {:?}", event);
}

// Option B: Poll status periodically
loop {
    let snapshot = orchestrator.get_task_status(&task).await?;
    println!("Progress: {}%", snapshot.progress);
    println!("State: {:?}", snapshot.state);

    if snapshot.state.is_terminal() {
        break;
    }

    tokio::time::sleep(Duration::from_secs(1)).await;
}

// Option C: Wait for completion (blocking)
let final_state = orchestrator.wait_for_completion(
    &mut task,
    Some(600),  // 10 minute timeout
).await?;

println!("Final artifacts: {:?}", final_state.artifacts);
```

### 5. Handle Task Results
```rust
use osiris_compiler::domain::OrchestrationTaskState;

match final_state.state {
    OrchestrationTaskState::Completed => {
        println!("Success!");
        for artifact in &final_state.artifacts {
            println!("- {} ({} bytes)", artifact.name, artifact.size.unwrap_or(0));
        }
    }
    OrchestrationTaskState::Failed => {
        // Get detailed error info
        let details = orchestrator.get_failure_details(&task).await?;
        eprintln!("Compilation failed: {}", details);

        // Retry if not exhausted
        if task.can_retry() {
            task.increment_retry();
            let retried = orchestrator.retry_task(&mut task).await?;
            // Continue monitoring...
        }
    }
    OrchestrationTaskState::Canceled => {
        println!("Task was canceled");
    }
    _ => {
        println!("Unexpected state: {:?}", final_state.state);
    }
}
```

## Common Patterns

### Compile Multiple Targets in Parallel
```rust
use futures::future::join_all;

let targets = vec![
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
];

let mut tasks = Vec::new();
for target in targets {
    let task = orchestrator.submit_task(
        agent_for_target(target),
        url_for_agent(agent_for_target(target)),
        "parallel-build",
        OperationPayload::Compile {
            source: src.clone(),
            target: target.to_string(),
            flags: None,
            opt_level: 3,
        },
    ).await?;
    tasks.push(task);
}

// Wait for all
let results = join_all(
    tasks.iter_mut().map(|t| {
        orchestrator.wait_for_completion(t, Some(300))
    })
).await;

for result in results {
    println!("Result: {:?}", result?);
}
```

### Link Multiple Object Files
```rust
let link_task = orchestrator.submit_task(
    "osiris-linker",
    "https://linker.example.com/api",
    "my-build",
    OperationPayload::Link {
        objects: vec![
            "build/main.o".to_string(),
            "build/lib.a".to_string(),
        ],
        output_format: "elf".to_string(),
    },
).await?;

let result = orchestrator.wait_for_completion(&mut link_task, None).await?;
```

### Type Check Multiple Files
```rust
let check_task = orchestrator.submit_task(
    "osiris-checker",
    "https://checker.example.com/api",
    "type-check-session",
    OperationPayload::Analyze {
        source: std::fs::read_to_string("src/lib.rs")?,
        analysis_type: "type-check".to_string(),
        parameters: Default::default(),
    },
).await?;

orchestrator.wait_for_completion(&mut check_task, Some(60)).await?;
```

### Implement Auto-Retry with Backoff
```rust
async fn compile_with_backoff(
    orchestrator: &RemoteA2AOrchestratorAdapter,
    agent_id: &str,
    agent_url: &str,
    operation: OperationPayload,
) -> Result<OrchestrationSnapshot, Box<dyn std::error::Error>> {
    let mut task = orchestrator.submit_task(
        agent_id,
        agent_url,
        "auto-retry",
        operation.clone(),
    ).await?;

    loop {
        match orchestrator.wait_for_completion(&mut task, Some(300)).await {
            Ok(snapshot) if snapshot.state.is_terminal() => {
                return Ok(snapshot);
            }
            Ok(_) => {} // Loop again
            Err(e) if task.can_retry() => {
                eprintln!("Retrying due to: {}", e);
                tokio::time::sleep(
                    Duration::from_secs(2u64.pow(task.retry_count))
                ).await;
                task = orchestrator.retry_task(&mut task).await?;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

### Stream Updates to Web UI
```rust
use axum::response::sse::{Event, Sse};
use futures::Stream;

async fn compilation_stream(
    orchestrator: Arc<RemoteA2AOrchestratorAdapter>,
    task_id: String,
) -> Result<Sse<impl Stream<Item = Result<Event, Box<dyn std::error::Error>>>>, String> {
    let task = load_task_from_db(&task_id).await?;

    let updates = orchestrator
        .stream_task_updates(&task)
        .await?
        .map(|event| {
            Ok(Event::default()
                .json_data(event)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>))
        });

    Ok(Sse::new(updates))
}
```

## Error Handling

```rust
use osiris_compiler::port::OrchestrationError;

match orchestrator.submit_task(...).await {
    Ok(task) => {
        // Monitor task...
    }
    Err(OrchestrationError::SubmissionFailed(msg)) => {
        eprintln!("Failed to submit: {}", msg);
    }
    Err(OrchestrationError::NetworkError(msg)) => {
        eprintln!("Network error: {}", msg);
        // Maybe retry with different agent
    }
    Err(OrchestrationError::RemoteAgentError(msg)) => {
        eprintln!("Agent error: {}", msg);
    }
    Err(OrchestrationError::Timeout(msg)) => {
        eprintln!("Operation timed out: {}", msg);
        // Cancel task
    }
    Err(e) => {
        eprintln!("Orchestration error: {}", e);
    }
}
```

## Configuration Reference

```rust
pub struct A2AOrchestratorConfig {
    /// Default timeout in seconds (0-300 recommended)
    pub timeout_secs: u64,              // Default: 300

    /// Automatically retry failed tasks
    pub auto_retry: bool,               // Default: true

    /// Maximum retries per task
    pub max_retries: u32,               // Default: 3

    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,            // Default: 1000

    /// Use streaming updates (vs one-shot status)
    pub stream_updates: bool,           // Default: true

    /// Poll interval for status updates (milliseconds)
    pub poll_interval_ms: u64,          // Default: 500

    /// User agent string for HTTP requests
    pub user_agent: String,             // Default: "osiris-compiler/0.1.0"
}
```

## TaskState Transitions

```
     ┌──────────────────────────┐
     ▼                          │
  SUBMITTING ──→ SUBMITTED ──→ EXECUTING
                                │   │
                        ┌───────┘   ├─→ PAUSED ──┐
                        │           │            │
                        │           └────────────┘
                        │                │
                        └────────────────┤
                                         ▼
                        ┌─────────────────────────┐
                        │                         │
                        ▼                         ▼
                    COMPLETED                  FAILED ──→ (retry) ──→ SUBMITTING
                                                 │
                                                 │
                        ┌────────────────────────┘
                        │
                        ▼
                     CANCELED
```

## Types Quick Reference

**Task States:**
- `Submitting` - Being sent to agent
- `Submitted` - Queued on agent
- `Executing` - Currently running
- `Paused` - Awaiting input
- `Completed` - Successfully finished
- `Failed` - Encountered error
- `Canceled` - User cancellation
- `Unknown` - Unable to determine

**Operation Types:**
```rust
OperationPayload::Compile {
    source: String,                  // Source code
    target: String,                  // Target triple (e.g., "aarch64-apple-darwin")
    flags: Option<Vec<String>>,      // Compiler flags
    opt_level: u8,                   // Optimization level (0-3)
}

OperationPayload::Link {
    objects: Vec<String>,            // Object file paths
    output_format: String,           // "elf", "mach-o", "pe"
}

OperationPayload::Analyze {
    source: String,                  // Code to analyze
    analysis_type: String,           // "type-check", "invariant-verify", etc.
    parameters: HashMap<String, Value>,
}

OperationPayload::Custom {
    op_type: String,
    data: Value,
}
```

**Events:**
- `StateChanged` - Task transitioned states
- `ProgressUpdate` - Work progress (0-100%)
- `ArtifactProduced` - New output file created
- `RetryScheduled` - Automatic retry queued
- `Completed` - Task finished (success or failure)

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Tasks timing out | Increase `timeout_secs` in config |
| High poll load | Increase `poll_interval_ms` |
| Agent not responding | Call `check_agent_health()` first |
| Artifacts not updating | Call `update_artifacts()` explicitly |
| Tasks not retrying | Enable `auto_retry` and check `max_retries` |

## See Also

- `A2A_ORCHESTRATOR.md` - Complete architecture documentation
- `IMPLEMENTATION_SUMMARY.md` - Implementation details
- Unit tests in source for more examples

---

**Ready to orchestrate your first compilation!**
