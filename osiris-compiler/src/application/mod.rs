//! Application layer for the Osiris compiler.
//!
//! This layer orchestrates the compilation process by combining:
//! - Domain types (Operation, Sigma, Packets)
//! - Port traits (DeterministicOrderer)
//! - Adapter implementations (LambdaOrderer)
//!
//! The application layer implements the high-level compiler logic μ: O → A.

pub mod compiler;

pub use compiler::{Compiler, CompilerConfig};
