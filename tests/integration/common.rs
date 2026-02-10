//! Common test utilities and helpers

use reqwest::Client;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

/// A spawned compiler service
pub struct CompilerService {
    pub addr: SocketAddr,
    pub handle: JoinHandle<()>,
    pub client: Client,
}

/// A spawned edge service
pub struct EdgeService {
    pub addr: SocketAddr,
    pub handle: JoinHandle<()>,
    pub client: Client,
}

/// Test environment with both services running
pub struct TestEnv {
    pub compiler: CompilerService,
    pub edge: EdgeService,
}

impl CompilerService {
    /// Spawn a compiler service on a random available port
    pub async fn spawn() -> Self {
        // Bind to 127.0.0.1:0 to get a random available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to port");

        let addr = listener.local_addr().expect("Failed to get local addr");
        info!("Starting compiler service on {}", addr);

        // Spawn the compiler service
        let handle = tokio::spawn(async move {
            run_compiler_service(listener).await;
        });

        let client = Client::new();

        Self {
            addr,
            handle,
            client,
        }
    }

    /// Get the base URL for this service
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Check if service is healthy
    pub async fn is_healthy(&self) -> bool {
        match self
            .client
            .get(&format!("{}/health", self.base_url()))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Wait for service to be healthy
    pub async fn wait_healthy(&self, max_attempts: u32) {
        for attempt in 1..=max_attempts {
            if self.is_healthy().await {
                info!("Compiler service is healthy");
                return;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            if attempt == max_attempts {
                panic!("Compiler service did not become healthy");
            }
        }
    }
}

impl EdgeService {
    /// Spawn an edge service on a random available port
    pub async fn spawn() -> Self {
        // Bind to 127.0.0.1:0 to get a random available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to port");

        let addr = listener.local_addr().expect("Failed to get local addr");
        info!("Starting edge service on {}", addr);

        // Spawn the edge service
        let handle = tokio::spawn(async move {
            run_edge_service(listener).await;
        });

        let client = Client::new();

        Self {
            addr,
            handle,
            client,
        }
    }

    /// Get the base URL for this service
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Check if service is healthy
    pub async fn is_healthy(&self) -> bool {
        match self
            .client
            .get(&format!("{}/health", self.base_url()))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Wait for service to be healthy
    pub async fn wait_healthy(&self, max_attempts: u32) {
        for attempt in 1..=max_attempts {
            if self.is_healthy().await {
                info!("Edge service is healthy");
                return;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            if attempt == max_attempts {
                panic!("Edge service did not become healthy");
            }
        }
    }
}

impl TestEnv {
    /// Create a test environment with both services
    pub async fn setup() -> Self {
        // Initialize tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        let compiler = CompilerService::spawn().await;
        let edge = EdgeService::spawn().await;

        // Wait for both services to be healthy
        compiler.wait_healthy(50).await;
        edge.wait_healthy(50).await;

        info!("Test environment is ready");

        Self { compiler, edge }
    }

    /// Shut down the test environment
    pub async fn shutdown(self) {
        // Services will be cleaned up when handles are dropped
        drop(self);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

/// Run the compiler service (internal)
async fn run_compiler_service(listener: tokio::net::TcpListener) {
    use axum::{routing::get, Router};
    use osiris_compiler::application::{compile, health_check, PipelineState};

    let pipeline_state = PipelineState::new_in_memory();

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/compile", axum::routing::post(compile))
        .with_state(pipeline_state);

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Compiler service error: {}", e);
    }
}

/// Run the edge service (internal)
async fn run_edge_service(listener: tokio::net::TcpListener) {
    use axum::{routing::get, Router, http::StatusCode};

    // Simple health check for now - the router will be added later
    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/ready", get(|| async { StatusCode::OK }));

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Edge service error: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compiler_spawns() {
        let compiler = CompilerService::spawn().await;
        compiler.wait_healthy(50).await;
        assert!(compiler.is_healthy().await);
    }

    #[tokio::test]
    async fn test_edge_spawns() {
        let edge = EdgeService::spawn().await;
        edge.wait_healthy(50).await;
        assert!(edge.is_healthy().await);
    }

    #[tokio::test]
    async fn test_both_services_spawn() {
        let env = TestEnv::setup().await;
        assert!(env.compiler.is_healthy().await);
        assert!(env.edge.is_healthy().await);
    }
}
