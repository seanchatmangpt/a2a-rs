//! Service layer for the A2A protocol
//!
//! Services provide application-level abstractions that orchestrate
//! between ports and adapters.

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod coordinator;

#[cfg(feature = "server")]
pub mod firewall;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "crypto")]
pub mod receipt;

#[cfg(feature = "client")]
pub use client::{
    A2AClientConfig, AsyncA2AClient, BatchClientOperations, BatchConfig, EnhancedHttpClient,
    PoolConfig, RetryConfig, StreamItem, TokenInfo, TokenRefreshConfig,
};

#[cfg(feature = "client")]
pub use client::{StreamItem};

#[cfg(feature = "server")]
pub use coordinator::{
    AndonSignal, AndonStatus, CoordinatorConfig, CoordinatorMetrics, HeijunkaScheduler, JidokaGate,
    Station, StationMetrics, TaktTime, TpsCoordinator,
};

#[cfg(feature = "server")]
pub use firewall::{FirewallConfig, FirewallMetrics, FirewallService};

#[cfg(feature = "server")]
pub use server::{AgentInfoProvider, AsyncA2ARequestProcessor};

#[cfg(feature = "crypto")]
pub use receipt::{
    MerkleTree, Receipt, ReceiptChain, ReceiptError, ReceiptResult, ReplayValidator,
};
