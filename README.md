# A2A-RS - Agent-to-Agent Protocol Implementation for Rust

[![Crates.io](https://img.shields.io/crates/v/a2a-rs.svg)](https://crates.io/crates/a2a-rs)
[![Documentation](https://docs.rs/a2a-rs/badge.svg)](https://docs.rs/a2a-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A Rust implementation of the Agent-to-Agent (A2A) Protocol, providing both a robust framework library and practical agent examples. This project demonstrates production-ready agent communication with modern Rust practices and hexagonal architecture.

## 🎯 Quick Start - Try the Reimbursement Agent

See the A2A protocol in action with our **reimbursement agent demo** - a complete, self-contained example with both agent backend and interactive web frontend:

```bash
# Clone the repository
git clone https://github.com/emillindfors/a2a-rs.git
cd a2a-rs

# Run the complete demo (agent + web UI)
cd a2a-agents
cargo run --bin reimbursement_demo

# Open your browser to http://localhost:3000
```

The reimbursement demo showcases:

- 💬 **Interactive web interface** for submitting expenses
- 📋 **Dynamic form generation** for expense submissions
- ✅ **Request validation** and approval workflows
- 📊 **Real-time task updates** with streaming
- 🔄 **Complete A2A protocol** implementation (HTTP + WebSocket)
- 🌐 **Production-ready architecture** with frontend and backend

**Try it out**: Open `http://localhost:3000` and submit an expense reimbursement request!

## 🏗️ Project Structure

This repository contains a complete A2A ecosystem:

### 📦 [a2a-rs](./a2a-rs/) - Core Framework Library

The main library published on [crates.io](https://crates.io/crates/a2a-rs):

- 🚀 **Complete A2A Protocol Implementation**
- 🔄 **HTTP & WebSocket Support** with streaming
- 🏛️ **Hexagonal Architecture** with clean separation
- 🧩 **Modular Features** - use only what you need
- 📚 **Comprehensive Documentation** with examples

### 🤖 [a2a-agents](./a2a-agents/) - Production Agent Examples

Real-world agent implementations demonstrating best practices:

- 💰 **Reimbursement Agent** - Handles expense requests with interactive workflows
- 🔧 **Modern Architecture** using the a2a-rs framework
- 📖 **Full Documentation** with setup guides

### 🔌 [a2a-mcp](./a2a-mcp/) - MCP Integration

Bridges A2A agents with the Model Context Protocol ecosystem:

- 🌉 **Bidirectional Integration** - A2A agents as MCP tools and vice versa
- 🔗 **Protocol Translation** between A2A and MCP formats
- 🛠️ **Developer Tools** for cross-protocol communication

### 💻 [a2a-client](./a2a-client/) - Web Interface

Browser-based client for interacting with A2A agents:

- 🌐 **Web UI** for agent communication
- 💬 **Chat Interface** with real-time updates
- 📱 **Responsive Design** for all devices

## ✨ Key Features

### 🎯 Framework Library (a2a-rs)

- **Type-Safe Protocol** - Rust's type system ensures protocol compliance
- **Async-First Design** - Built on Tokio with full async/await support
- **Multiple Transports** - HTTP, WebSocket with automatic fallback
- **Streaming Support** - Real-time task updates and progress tracking
- **Authentication** - JWT, OAuth2, OpenID Connect, API keys
- **Storage Backends** - SQLx integration for PostgreSQL, MySQL, SQLite
- **Observability** - Structured logging and tracing throughout

### 🤖 Agent Examples

- **Production Ready** - Complete implementations following best practices
- **Interactive Workflows** - Dynamic form generation and multi-step processes
- **Business Logic Examples** - Real use cases like expense reimbursement
- **Framework Integration** - Shows how to use a2a-rs effectively

### 🔧 Developer Experience

- **Comprehensive Documentation** - API docs, guides, and examples
- **Working Examples** - Copy-paste code that actually works
- **Test Coverage** - Integration tests and property-based testing
- **Error Handling** - Structured errors with helpful messages

## 🚀 Quick Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
a2a-rs = "0.1.0"

# For HTTP client
a2a-rs = { version = "0.1.0", features = ["http-client"] }

# For HTTP server
a2a-rs = { version = "0.1.0", features = ["http-server"] }

# Everything
a2a-rs = { version = "0.1.0", features = ["full"] }
```

### Simple Client Example

```rust
use a2a_rs::{HttpClient, Message};
use a2a_rs::port::AsyncA2AClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HttpClient::new("http://localhost:3030".to_string());

    let message = Message::user_text("I need to submit a $50 lunch expense".to_string());
    let task = client.send_task_message("task-123", &message, None, None).await?;

    println!("Response: {:?}", task);
    Ok(())
}
```

### Simple Server Example

```rust
use a2a_rs::{HttpServer, SimpleAgentInfo, DefaultRequestProcessor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = HttpServer::new(
        DefaultRequestProcessor::new(),
        SimpleAgentInfo::new("my-agent".to_string(), "1.0.0".to_string()),
        "127.0.0.1:3030".to_string(),
    );

    server.start().await?;
    Ok(())
}
```

## 🎪 Advanced Examples

### Streaming Client with WebSocket

```rust
use a2a_rs::{WebSocketClient, Message};
use a2a_rs::services::StreamItem;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = WebSocketClient::new("ws://localhost:3030/ws".to_string());

    let message = Message::user_text("Process my reimbursement request".to_string());
    let mut stream = client.subscribe_to_task("task-456", &message, None, None).await?;

    while let Some(result) = stream.next().await {
        match result? {
            StreamItem::Task(task) => println!("Initial task: {:?}", task),
            StreamItem::StatusUpdate(update) => {
                println!("Status: {:?}", update);
                if update.final_ { break; }
            }
            StreamItem::ArtifactUpdate(artifact) => {
                println!("New artifact: {:?}", artifact);
            }
        }
    }

    Ok(())
}
```

## 🏛️ Architecture

The project follows **hexagonal architecture** principles:

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                     │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │  JSON-RPC       │    │     HTTP/WebSocket          │ │
│  │  Handlers       │    │     Transport               │ │
│  └─────────────────┘    └─────────────────────────────┘ │
└─────────────────┬───────────────────────┬───────────────┘
                  │                       │
┌─────────────────▼───────────────────────▼───────────────┐
│                     Port Layer                          │
│  ┌──────────────────┐    ┌──────────────────────────┐   │
│  │ MessageHandler   │    │  StreamingHandler       │   │
│  │ TaskManager      │    │  NotificationManager    │   │
│  │ Authenticator    │    │  RequestProcessor       │   │
│  └──────────────────┘    └──────────────────────────┘   │
└─────────────────┬───────────────────────┬───────────────┘
                  │                       │
┌─────────────────▼───────────────────────▼───────────────┐
│                    Domain Layer                         │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────────────┐  │
│  │   Message    │ │     Task     │ │   AgentCard     │  │
│  │   Artifact   │ │ TaskStatus   │ │ Capabilities    │  │
│  │     Part     │ │   History    │ │    Skills       │  │
│  └──────────────┘ └──────────────┘ └─────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## 🧬 The Paradigm Shift: Constructive Code Manufacture

a2a-rs is built on a foundational paradigm shift from **Subjective Code Manufacture (SCM)** to **Constructive Code Manufacture (CCM)**. This approach eliminates coordination overhead and enables autonomous agent-driven development through:

- **Ontology-Driven Generation** - Protocol types defined in RDF ontologies, generated via SPARQL CONSTRUCT queries
- **Zero Coordination Overhead** - Conway's Law and Little's Law constraints collapsed through deterministic compilation
- **Toyota Production System Principles** - Just-In-Time generation, Jidoka quality gates, Kanban-based workflows
- **Receipts Over Narratives** - Every decision emits a proof object with hash chains and replay pointers
- **Complete Autonomy** - All admissible states resolved internally; inadmissible states refused at the boundary

**Learn more:**
- **[PARADIGM-SHIFT.md](./docs/PARADIGM-SHIFT.md)** - Complete explanation of SCM→CCM transition
- **[TPS-MAPPING.md](./docs/TPS-MAPPING.md)** - Manufacturing principles applied to code generation
- **[CONSTRUCT.md](./CONSTRUCT.md)** - Theoretical foundations (information theory, category theory)
- **[CLAUDE.md](./CLAUDE.md)** - Development guide with CONSTRUCT workflow

## 📚 Documentation

- **[Core Library Docs](https://docs.rs/a2a-rs)** - Complete API documentation
- **[Reimbursement Agent Guide](./a2a-agents/README.md)** - Building production agents
- **[MCP Integration Guide](./a2a-mcp/README.md)** - Cross-protocol communication
- **[Web Client Setup](./a2a-client/README.md)** - Browser-based interfaces

## 🧪 Testing

```bash
# Test the core library
cd a2a-rs && cargo test --all-features

# Test agent examples
cd a2a-agents && cargo test

# Test MCP integration
cd a2a-mcp && cargo test

# Run integration tests
cargo test --workspace
```

## 🛣️ Roadmap

- [x] **Core Protocol** - Complete A2A specification implementation
- [x] **Documentation** - Comprehensive docs and examples
- [x] **Agent Examples** - Production-ready reimbursement agent
- [x] **MCP Integration** - Cross-protocol compatibility
- [ ] **More Agent Types** - Additional domain examples
- [ ] **Performance Optimization** - Benchmarking and improvements
- [ ] **Advanced Auth** - Enterprise authentication patterns

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
git clone https://github.com/emillindfors/a2a-rs.git
cd a2a-rs
cargo build --workspace
cargo test --workspace
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🌟 Showcase

Built with a2a-rs? We'd love to feature your project! Open an issue to let us know.

---

**Ready to build intelligent agents?** Start with our [reimbursement agent example](./a2a-agents/) or dive into the [core library documentation](https://docs.rs/a2a-rs)!
