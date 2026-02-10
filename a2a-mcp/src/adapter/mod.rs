//! Adapters for converting between A2A and RMCP

mod agent_to_tool;
mod origin_guard;
pub mod session_manager;
pub mod sse_manager;
pub mod task_wrapper;
mod tool_to_agent;

pub use agent_to_tool::AgentToToolAdapter;
pub use origin_guard::OriginGuard;
pub use session_manager::InMemorySessionManager;
pub use sse_manager::{AxumSseStream, SseEvent, SseManager, SseManagerConfig};
pub use task_wrapper::TaskWrapper;
pub use tool_to_agent::ToolToAgentAdapter;
