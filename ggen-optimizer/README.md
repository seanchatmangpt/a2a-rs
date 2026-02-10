# ggen-optimizer

SPARQL CONSTRUCT query optimizer for ggen code generation.

## Features

- **SPARQL CONSTRUCT Parser**: Full nom-based parser supporting:
  - PREFIX declarations
  - CONSTRUCT templates
  - Complex WHERE clauses (OPTIONAL, UNION, FILTER, BIND)
  - Triple patterns with variables, IRIs, prefixed names, and literals

- **Static Analysis**: Detects optimization opportunities including:
  - Redundant graph traversals
  - Unused variables
  - Independent subquery groups for parallelization
  - Join patterns and selectivity estimation

- **Cost Model**: Estimates query execution cost based on:
  - Triple pattern selectivity
  - Join cardinality
  - Operator costs (filter, bind, optional, union)
  - Predicate statistics

- **Query Rewriting**: Applies optimization passes:
  - **Predicate Pushdown**: Moves filters closer to data sources
  - **Join Elimination**: Removes unnecessary OPTIONAL joins
  - **Subquery Flattening**: Reduces nesting depth
  - **Redundant Pattern Elimination**: Removes duplicate triple patterns
  - **Parallel Decomposition**: Identifies tensor product structure for parallel execution

## Usage

```rust
use ggen_optimizer::{Parser, Optimizer, OptimizerConfig};

let query = r#"
    PREFIX a2a: <https://ggen.io/ontology/a2a/>
    CONSTRUCT {
        ?entity a a2a:GeneratedStruct ;
            a2a:structName ?name .
    }
    WHERE {
        ?entity a a2a:Entity ;
            a2a:name ?name .
        OPTIONAL {
            ?entity a2a:unusedProperty ?unused .
        }
    }
"#;

// Parse the query
let ast = Parser::parse(query)?;

// Configure and run optimizer
let config = OptimizerConfig::default();
let mut optimizer = Optimizer::new(config);
let result = optimizer.optimize(ast)?;

println!("Original cost: {:.2}", result.original_cost);
println!("Optimized cost: {:.2}", result.optimized_cost);
println!("Speedup: {:.2}x", result.speedup());
println!("Passes applied: {:?}", result.passes_applied);
```

## Architecture

```
Query String
    ↓
Parser (nom)
    ↓
AST (Query, GraphPattern, TriplePattern)
    ↓
Analyzer (petgraph) → AnalysisResult
    ↓
CostModel → QueryCost
    ↓
Rewriter → OptimizedQuery
```

## Optimization Passes

### 1. Predicate Pushdown

Moves FILTER constraints closer to the patterns they constrain, reducing intermediate result sizes.

**Before:**
```sparql
{ ?s ?p ?o . ?s a:type "Person" . } FILTER(?o = "value")
```

**After:**
```sparql
{ ?s ?p ?o . FILTER(?o = "value") . ?s a:type "Person" . }
```

### 2. Join Elimination

Removes OPTIONAL patterns whose variables are never used in the CONSTRUCT template.

**Before:**
```sparql
CONSTRUCT { ?s a:name ?name . }
WHERE {
    ?s a:name ?name .
    OPTIONAL { ?s a:unusedProp ?unused . }
}
```

**After:**
```sparql
CONSTRUCT { ?s a:name ?name . }
WHERE {
    ?s a:name ?name .
}
```

### 3. Subquery Flattening

Removes unnecessary nesting in graph patterns.

**Before:**
```sparql
WHERE { { { ?s ?p ?o . } } }
```

**After:**
```sparql
WHERE { ?s ?p ?o . }
```

### 4. Redundant Elimination

Removes duplicate triple patterns that don't contribute additional constraints.

### 5. Parallel Decomposition

Identifies independent subqueries that can be executed in parallel using tensor product analysis.

## Configuration

```rust
let config = OptimizerConfig {
    enable_predicate_pushdown: true,
    enable_join_elimination: true,
    enable_subquery_flattening: true,
    enable_parallel_decomposition: true,
    enable_redundant_elimination: true,
    max_iterations: 5,
};
```

## Cost Model

The cost model uses heuristics based on:

- **Selectivity**: Patterns with fewer variables are more selective
- **Cardinality**: Estimated result set sizes based on predicate statistics
- **Operation Costs**: Each operator (scan, join, filter) has a base cost
- **Join Estimation**: Uses simplified selectivity assumptions

You can provide predicate statistics for more accurate cost estimation:

```rust
use ggen_optimizer::{CostModel, PredicateStats};

let mut cost_model = CostModel::new();
cost_model.add_predicate_stats(
    "a2a:name".to_string(),
    PredicateStats {
        count: 10_000,
        distinct_subjects: 5_000,
        distinct_objects: 8_000,
    },
);
```

## Testing

```bash
cargo test -p ggen-optimizer
```

## License

MIT OR Apache-2.0
