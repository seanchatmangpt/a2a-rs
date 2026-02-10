//! Application layer for the Osiris compiler.
//!
//! This layer orchestrates the compilation process by combining:
//! - Domain types (Operation, Sigma, Packets)
//! - Port traits (DeterministicOrderer)
//! - Adapter implementations (LambdaOrderer)
//!
//! The application layer implements the high-level compiler logic μ: O → A.

pub mod compiler;
pub mod http_handlers;

pub use compiler::{Compiler, CompilerConfig};
pub use http_handlers::{
    compile, health_check, AppError, CompileRequest, CompileResponse, ErrorResponse, PipelineState,
    PipelineStats,
};
