//! SPARQL CONSTRUCT Query Optimizer
//!
//! Provides static analysis, cost modeling, and query rewriting for SPARQL CONSTRUCT
//! queries used in ggen code generation. Optimizations include:
//!
//! - Detection and elimination of redundant graph traversals
//! - Tensor product decomposition for parallel execution
//! - Predicate pushdown to reduce intermediate result sizes
//! - Join elimination when joins are provably unnecessary
//! - Subquery flattening to reduce nesting depth
//!
//! # Architecture
//!
//! ```text
//! Input Query → Parser → AST → Analyzer → Cost Model → Rewriter → Optimized AST
//! ```
//!
//! # Example
//!
//! ```rust
//! use ggen_optimizer::{Parser, Optimizer, OptimizerConfig};
//!
//! let query = r#"
//!     PREFIX a2a: <https://ggen.io/ontology/a2a/>
//!     CONSTRUCT {
//!         ?s a2a:name ?name .
//!         ?s a2a:value ?value .
//!     }
//!     WHERE {
//!         ?s a2a:name ?name .
//!         ?s a2a:value ?value .
//!     }
//! "#;
//!
//! let ast = Parser::parse(query).expect("valid query");
//! let config = OptimizerConfig::default();
//! let mut optimizer = Optimizer::new(config);
//! let optimized = optimizer.optimize(ast).expect("optimization succeeded");
//! ```

pub mod analyzer;
pub mod ast;
pub mod cost;
pub mod error;
pub mod parser;
pub mod rewriter;

pub use analyzer::Analyzer;
pub use ast::{ConstructQuery, GraphPattern, Query, TriplePattern};
pub use cost::{CostModel, QueryCost};
pub use error::{Error, Result};
pub use parser::Parser;
pub use rewriter::{Optimizer, OptimizerConfig};

/// Re-export for convenience
pub use petgraph;
