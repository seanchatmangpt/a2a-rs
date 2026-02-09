//! Example demonstrating the SPARQL CONSTRUCT query optimizer.
//!
//! Run with:
//! ```bash
//! cargo run -p ggen-optimizer --example optimize_query
//! ```

use ggen_optimizer::{Analyzer, CostModel, Optimizer, OptimizerConfig, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example SPARQL CONSTRUCT query (simplified syntax)
    let query = r#"
        PREFIX a2a: <https://ggen.io/ontology/a2a/>

        CONSTRUCT {
            ?entity a2a:name ?name .
            ?entity a2a:hasField ?prop .
        }
        WHERE {
            ?entity a2a:name ?name .
            ?entity a2a:hasProperty ?prop .
            ?prop a2a:name ?propName .
        }
    "#;

    println!("=== SPARQL CONSTRUCT Query Optimizer Demo ===\n");

    // Parse the query
    println!("Parsing query...");
    let ast = Parser::parse(query)?;
    println!("✓ Successfully parsed query");
    println!("  - {} prefixes", ast.prefixes.len());
    println!("  - {} CONSTRUCT patterns", ast.construct.patterns.len());
    println!();

    // Analyze the query
    println!("Analyzing query structure...");
    let mut analyzer = Analyzer::new();
    let analysis = analyzer.analyze(&ast)?;
    println!("✓ Analysis complete");
    println!(
        "  - {} variables in CONSTRUCT",
        analysis.construct_vars.len()
    );
    println!("  - {} variables bound in WHERE", analysis.bound_vars.len());
    println!("  - {} unused variables", analysis.unused_vars.len());
    println!(
        "  - {} redundant patterns",
        analysis.redundant_patterns.len()
    );
    println!("  - {} parallel groups", analysis.parallel_groups.len());
    if !analysis.unused_vars.is_empty() {
        println!("  - Unused: {:?}", analysis.unused_vars);
    }
    println!();

    // Estimate cost
    println!("Estimating query cost...");
    let cost_model = CostModel::new();
    let cost = cost_model.estimate_cost(&ast)?;
    println!("✓ Cost estimation complete");
    println!("  - Total cost: {:.2}", cost.total);
    println!("  - Estimated cardinality: {}", cost.cardinality);
    for (op, cost) in &cost.breakdown {
        println!("    - {}: {:.2}", op, cost);
    }
    println!();

    // Optimize the query
    println!("Optimizing query...");
    let config = OptimizerConfig {
        enable_predicate_pushdown: true,
        enable_join_elimination: true,
        enable_subquery_flattening: true,
        enable_parallel_decomposition: true,
        enable_redundant_elimination: true,
        max_iterations: 5,
    };
    let mut optimizer = Optimizer::new(config);
    let result = optimizer.optimize(ast)?;

    println!("✓ Optimization complete");
    println!("  - Original cost: {:.2}", result.original_cost);
    println!("  - Optimized cost: {:.2}", result.optimized_cost);
    println!("  - Speedup: {:.2}x", result.speedup());
    println!("  - Passes applied: {}", result.passes_applied.len());
    for (i, pass) in result.passes_applied.iter().enumerate() {
        println!("    {}. {}", i + 1, pass);
    }
    println!();

    // Re-analyze optimized query
    println!("Analyzing optimized query...");
    let optimized_analysis = analyzer.analyze(&result.query)?;
    println!("✓ Optimized analysis");
    println!(
        "  - {} unused variables (was {})",
        optimized_analysis.unused_vars.len(),
        analysis.unused_vars.len()
    );
    println!(
        "  - {} redundant patterns (was {})",
        optimized_analysis.redundant_patterns.len(),
        analysis.redundant_patterns.len()
    );
    println!();

    println!("=== Summary ===");
    println!("{}", analysis.summary());
    println!("{}", result.summary());

    Ok(())
}
