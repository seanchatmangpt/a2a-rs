//! Query rewriting and optimization.
//!
//! Applies optimization passes to transform queries into more efficient forms:
//! - Predicate pushdown
//! - Join elimination
//! - Subquery flattening
//! - Parallel execution decomposition

use crate::analyzer::{AnalysisResult, Analyzer};
use crate::ast::*;
use crate::cost::CostModel;
use crate::error::Result;
use rustc_hash::FxHashSet;

/// Configuration for the optimizer.
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Enable predicate pushdown optimization.
    pub enable_predicate_pushdown: bool,
    /// Enable join elimination.
    pub enable_join_elimination: bool,
    /// Enable subquery flattening.
    pub enable_subquery_flattening: bool,
    /// Enable parallel execution decomposition.
    pub enable_parallel_decomposition: bool,
    /// Enable redundant pattern elimination.
    pub enable_redundant_elimination: bool,
    /// Maximum number of optimization iterations.
    pub max_iterations: usize,
}

/// Query optimizer.
pub struct Optimizer {
    config: OptimizerConfig,
    analyzer: Analyzer,
    cost_model: CostModel,
}

/// Result of optimization.
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Optimized query.
    pub query: Query,
    /// Cost before optimization.
    pub original_cost: f64,
    /// Cost after optimization.
    pub optimized_cost: f64,
    /// Optimization passes applied.
    pub passes_applied: Vec<String>,
}

impl Optimizer {
    /// Create a new optimizer with the given configuration.
    pub fn new(config: OptimizerConfig) -> Self {
        Self {
            config,
            analyzer: Analyzer::new(),
            cost_model: CostModel::new(),
        }
    }

    /// Optimize a query.
    pub fn optimize(&mut self, query: Query) -> Result<OptimizationResult> {
        let original_cost = self.cost_model.estimate_cost(&query)?.total;
        let mut current_query = query;
        let mut passes_applied = Vec::new();

        for iteration in 0..self.config.max_iterations {
            let mut modified = false;

            // Analyze the query
            let analysis = self.analyzer.analyze(&current_query)?;

            // Apply optimization passes
            if self.config.enable_redundant_elimination {
                if let Some(optimized) =
                    self.eliminate_redundant_patterns(&current_query, &analysis)?
                {
                    current_query = optimized;
                    passes_applied.push(format!("redundant-elimination-{}", iteration));
                    modified = true;
                }
            }

            if self.config.enable_predicate_pushdown {
                if let Some(optimized) = self.pushdown_predicates(&current_query, &analysis)? {
                    current_query = optimized;
                    passes_applied.push(format!("predicate-pushdown-{}", iteration));
                    modified = true;
                }
            }

            if self.config.enable_join_elimination {
                if let Some(optimized) = self.eliminate_joins(&current_query, &analysis)? {
                    current_query = optimized;
                    passes_applied.push(format!("join-elimination-{}", iteration));
                    modified = true;
                }
            }

            if self.config.enable_subquery_flattening {
                if let Some(optimized) = self.flatten_subqueries(&current_query)? {
                    current_query = optimized;
                    passes_applied.push(format!("subquery-flattening-{}", iteration));
                    modified = true;
                }
            }

            if self.config.enable_parallel_decomposition {
                if let Some(optimized) =
                    self.decompose_for_parallelism(&current_query, &analysis)?
                {
                    current_query = optimized;
                    passes_applied.push(format!("parallel-decomposition-{}", iteration));
                    modified = true;
                }
            }

            // If no modifications were made, we're done
            if !modified {
                break;
            }
        }

        let optimized_cost = self.cost_model.estimate_cost(&current_query)?.total;

        Ok(OptimizationResult {
            query: current_query,
            original_cost,
            optimized_cost,
            passes_applied,
        })
    }

    /// Eliminate redundant triple patterns.
    fn eliminate_redundant_patterns(
        &self,
        query: &Query,
        analysis: &AnalysisResult,
    ) -> Result<Option<Query>> {
        if analysis.redundant_patterns.is_empty() {
            return Ok(None);
        }

        let mut optimized = query.clone();
        let redundant_set: FxHashSet<usize> = analysis.redundant_patterns.iter().copied().collect();

        optimized.where_clause =
            self.filter_redundant_in_pattern(&query.where_clause, &redundant_set, &mut 0);

        Ok(Some(optimized))
    }

    /// Filter out redundant patterns from a graph pattern.
    fn filter_redundant_in_pattern(
        &self,
        pattern: &GraphPattern,
        redundant: &FxHashSet<usize>,
        counter: &mut usize,
    ) -> GraphPattern {
        match pattern {
            GraphPattern::Basic(patterns) => {
                let filtered: Vec<_> = patterns
                    .iter()
                    .filter(|_| {
                        let idx = *counter;
                        *counter += 1;
                        !redundant.contains(&idx)
                    })
                    .cloned()
                    .collect();
                GraphPattern::Basic(filtered)
            }
            GraphPattern::Optional(p) => GraphPattern::Optional(Box::new(
                self.filter_redundant_in_pattern(p, redundant, counter),
            )),
            GraphPattern::Union(left, right) => GraphPattern::Union(
                Box::new(self.filter_redundant_in_pattern(left, redundant, counter)),
                Box::new(self.filter_redundant_in_pattern(right, redundant, counter)),
            ),
            GraphPattern::Filter { expr, pattern: p } => GraphPattern::Filter {
                expr: expr.clone(),
                pattern: Box::new(self.filter_redundant_in_pattern(p, redundant, counter)),
            },
            GraphPattern::Group(patterns) => {
                let filtered: Vec<_> = patterns
                    .iter()
                    .map(|p| self.filter_redundant_in_pattern(p, redundant, counter))
                    .collect();
                GraphPattern::Group(filtered)
            }
            other => other.clone(),
        }
    }

    /// Push down filter predicates closer to the data.
    fn pushdown_predicates(
        &self,
        query: &Query,
        _analysis: &AnalysisResult,
    ) -> Result<Option<Query>> {
        let mut optimized = query.clone();
        optimized.where_clause = self.pushdown_filters(&query.where_clause);

        // Check if anything changed
        if optimized.where_clause == query.where_clause {
            Ok(None)
        } else {
            Ok(Some(optimized))
        }
    }

    /// Recursively push down filters.
    fn pushdown_filters(&self, pattern: &GraphPattern) -> GraphPattern {
        match pattern {
            GraphPattern::Filter {
                expr,
                pattern: inner,
            } => {
                // Try to push the filter into the inner pattern
                match &**inner {
                    GraphPattern::Union(left, right) => {
                        // Push filter into both branches
                        GraphPattern::Union(
                            Box::new(GraphPattern::Filter {
                                expr: expr.clone(),
                                pattern: left.clone(),
                            }),
                            Box::new(GraphPattern::Filter {
                                expr: expr.clone(),
                                pattern: right.clone(),
                            }),
                        )
                    }
                    GraphPattern::Group(patterns) => {
                        // Try to find the earliest pattern that has all filter variables
                        let filter_vars = self.extract_filter_vars(expr);
                        let mut pushed = false;
                        let mut new_patterns = Vec::new();

                        for p in patterns {
                            let pattern_vars = self.extract_pattern_vars(p);
                            if !pushed && filter_vars.iter().all(|v| pattern_vars.contains(v)) {
                                // Push filter here
                                new_patterns.push(GraphPattern::Filter {
                                    expr: expr.clone(),
                                    pattern: Box::new(p.clone()),
                                });
                                pushed = true;
                            } else {
                                new_patterns.push(p.clone());
                            }
                        }

                        if pushed {
                            GraphPattern::Group(new_patterns)
                        } else {
                            // Can't push down, keep as is
                            GraphPattern::Filter {
                                expr: expr.clone(),
                                pattern: inner.clone(),
                            }
                        }
                    }
                    _ => GraphPattern::Filter {
                        expr: expr.clone(),
                        pattern: Box::new(self.pushdown_filters(inner)),
                    },
                }
            }
            GraphPattern::Optional(p) => GraphPattern::Optional(Box::new(self.pushdown_filters(p))),
            GraphPattern::Union(left, right) => GraphPattern::Union(
                Box::new(self.pushdown_filters(left)),
                Box::new(self.pushdown_filters(right)),
            ),
            GraphPattern::Group(patterns) => {
                GraphPattern::Group(patterns.iter().map(|p| self.pushdown_filters(p)).collect())
            }
            other => other.clone(),
        }
    }

    /// Extract variables from a filter expression.
    fn extract_filter_vars(&self, expr: &FilterExpr) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_filter_vars(expr, &mut vars);
        vars
    }

    /// Collect variables from filter expression.
    fn collect_filter_vars(&self, expr: &FilterExpr, vars: &mut Vec<String>) {
        match expr {
            FilterExpr::Var(v) => vars.push(v.name.clone()),
            FilterExpr::Bound(v) => vars.push(v.name.clone()),
            FilterExpr::Relational { left, right, .. } => {
                self.collect_filter_vars(left, vars);
                self.collect_filter_vars(right, vars);
            }
            FilterExpr::Logical { left, right, .. } => {
                self.collect_filter_vars(left, vars);
                self.collect_filter_vars(right, vars);
            }
            FilterExpr::Not(e) => self.collect_filter_vars(e, vars),
            FilterExpr::Function { args, .. } => {
                for arg in args {
                    self.collect_filter_vars(arg, vars);
                }
            }
            FilterExpr::Literal(_) => {}
        }
    }

    /// Extract variables from a graph pattern.
    fn extract_pattern_vars(&self, pattern: &GraphPattern) -> Vec<String> {
        let mut vars = Vec::new();
        pattern.collect_vars(&mut vars);
        vars.iter().map(|v| v.name.clone()).collect()
    }

    /// Eliminate unnecessary joins.
    fn eliminate_joins(&self, query: &Query, analysis: &AnalysisResult) -> Result<Option<Query>> {
        // Join elimination: if a pattern only appears in OPTIONAL and its variables
        // aren't used elsewhere, it can be removed
        let used_vars: FxHashSet<_> = analysis.construct_vars.iter().cloned().collect();

        let mut optimized = query.clone();
        optimized.where_clause = self.eliminate_unused_optionals(&query.where_clause, &used_vars);

        if optimized.where_clause == query.where_clause {
            Ok(None)
        } else {
            Ok(Some(optimized))
        }
    }

    /// Eliminate OPTIONAL patterns with unused variables.
    fn eliminate_unused_optionals(
        &self,
        pattern: &GraphPattern,
        used_vars: &FxHashSet<String>,
    ) -> GraphPattern {
        match pattern {
            GraphPattern::Optional(inner) => {
                let pattern_vars = self.extract_pattern_vars(inner);
                if pattern_vars.iter().any(|v| used_vars.contains(v)) {
                    // Keep this optional, some variables are used
                    GraphPattern::Optional(Box::new(
                        self.eliminate_unused_optionals(inner, used_vars),
                    ))
                } else {
                    // All variables unused, eliminate this optional
                    // Return an empty basic pattern
                    GraphPattern::Basic(Vec::new())
                }
            }
            GraphPattern::Union(left, right) => GraphPattern::Union(
                Box::new(self.eliminate_unused_optionals(left, used_vars)),
                Box::new(self.eliminate_unused_optionals(right, used_vars)),
            ),
            GraphPattern::Group(patterns) => {
                let filtered: Vec<_> = patterns
                    .iter()
                    .map(|p| self.eliminate_unused_optionals(p, used_vars))
                    .filter(|p| !matches!(p, GraphPattern::Basic(v) if v.is_empty()))
                    .collect();
                GraphPattern::Group(filtered)
            }
            other => other.clone(),
        }
    }

    /// Flatten nested subqueries.
    fn flatten_subqueries(&self, query: &Query) -> Result<Option<Query>> {
        let mut optimized = query.clone();
        optimized.where_clause = self.flatten_pattern(&query.where_clause);

        if optimized.where_clause == query.where_clause {
            Ok(None)
        } else {
            Ok(Some(optimized))
        }
    }

    /// Flatten a graph pattern.
    fn flatten_pattern(&self, pattern: &GraphPattern) -> GraphPattern {
        match pattern {
            GraphPattern::Group(patterns) => {
                let mut flattened = Vec::new();
                for p in patterns {
                    let flat = self.flatten_pattern(p);
                    match flat {
                        GraphPattern::Group(inner) => flattened.extend(inner),
                        other => flattened.push(other),
                    }
                }
                if flattened.len() == 1 {
                    flattened.into_iter().next().unwrap()
                } else {
                    GraphPattern::Group(flattened)
                }
            }
            GraphPattern::Optional(p) => GraphPattern::Optional(Box::new(self.flatten_pattern(p))),
            GraphPattern::Union(left, right) => GraphPattern::Union(
                Box::new(self.flatten_pattern(left)),
                Box::new(self.flatten_pattern(right)),
            ),
            GraphPattern::Filter { expr, pattern: p } => GraphPattern::Filter {
                expr: expr.clone(),
                pattern: Box::new(self.flatten_pattern(p)),
            },
            other => other.clone(),
        }
    }

    /// Decompose query for parallel execution using tensor product decomposition.
    fn decompose_for_parallelism(
        &self,
        _query: &Query,
        analysis: &AnalysisResult,
    ) -> Result<Option<Query>> {
        // If there are independent pattern groups, we can annotate them for parallel execution
        // In a real implementation, this would involve creating parallel execution hints
        // For now, we'll reorder patterns to group independent ones together

        if analysis.parallel_groups.len() <= 1 {
            return Ok(None);
        }

        // This is a simplified implementation - in practice you'd need more sophisticated
        // query plan restructuring
        Ok(None)
    }
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            enable_predicate_pushdown: true,
            enable_join_elimination: true,
            enable_subquery_flattening: true,
            enable_parallel_decomposition: true,
            enable_redundant_elimination: true,
            max_iterations: 5,
        }
    }
}

impl OptimizationResult {
    /// Get the speedup ratio.
    pub fn speedup(&self) -> f64 {
        if self.optimized_cost == 0.0 {
            return 1.0;
        }
        self.original_cost / self.optimized_cost
    }

    /// Check if optimization improved the query.
    pub fn is_improved(&self) -> bool {
        self.optimized_cost < self.original_cost
    }

    /// Get a summary of the optimization.
    pub fn summary(&self) -> String {
        format!(
            "Optimization: {:.2}x speedup, {} passes applied (cost: {:.2} → {:.2})",
            self.speedup(),
            self.passes_applied.len(),
            self.original_cost,
            self.optimized_cost
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_optimizer_default_config() {
        let config = OptimizerConfig::default();
        assert!(config.enable_predicate_pushdown);
        assert!(config.enable_join_elimination);
        assert_eq!(config.max_iterations, 5);
    }

    #[test]
    fn test_flatten_nested_groups() {
        let config = OptimizerConfig::default();
        let optimizer = Optimizer::new(config);

        let inner_group = GraphPattern::Group(vec![
            GraphPattern::Basic(vec![]),
            GraphPattern::Basic(vec![]),
        ]);
        let outer_group = GraphPattern::Group(vec![inner_group]);

        let flattened = optimizer.flatten_pattern(&outer_group);

        match flattened {
            GraphPattern::Group(patterns) => {
                assert_eq!(patterns.len(), 2);
            }
            _ => panic!("Expected flattened group"),
        }
    }

    #[test]
    fn test_optimization_result() {
        let query = Query::new(
            HashMap::new(),
            ConstructTemplate::new(vec![]),
            GraphPattern::Basic(vec![]),
        );

        let result = OptimizationResult {
            query,
            original_cost: 100.0,
            optimized_cost: 50.0,
            passes_applied: vec!["test".to_string()],
        };

        assert_eq!(result.speedup(), 2.0);
        assert!(result.is_improved());
    }

    #[test]
    fn test_extract_filter_vars() {
        let config = OptimizerConfig::default();
        let optimizer = Optimizer::new(config);

        let expr = FilterExpr::Var(Var {
            name: "x".to_string(),
        });

        let vars = optimizer.extract_filter_vars(&expr);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], "x");
    }
}
