//! Static analysis for SPARQL CONSTRUCT queries.
//!
//! Detects optimization opportunities such as:
//! - Redundant graph traversals
//! - Unused variables
//! - Subquery independence (for parallelization)
//! - Join patterns

use crate::ast::*;
use crate::error::Result;
use indexmap::IndexSet;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::Dfs;
use rustc_hash::FxHashMap;

/// Static analyzer for queries.
#[allow(dead_code)]
pub struct Analyzer {
    /// Dependency graph of variables (reserved for future use).
    var_deps: DiGraph<String, ()>,
    /// Map from variable names to graph node indices (reserved for future use).
    var_indices: FxHashMap<String, NodeIndex>,
}

/// Results of static analysis.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Variables that are bound but never used.
    pub unused_vars: Vec<String>,
    /// Triple patterns that are redundant.
    pub redundant_patterns: Vec<usize>,
    /// Groups of patterns that can be executed in parallel.
    pub parallel_groups: Vec<Vec<usize>>,
    /// Variables used in the CONSTRUCT template.
    pub construct_vars: IndexSet<String>,
    /// Variables bound in the WHERE clause.
    pub bound_vars: IndexSet<String>,
    /// Join graph showing which patterns share variables.
    pub join_graph: DiGraph<usize, String>,
    /// Estimated selectivity for each pattern (0.0 = highly selective, 1.0 = not selective).
    pub selectivities: Vec<f64>,
}

impl Analyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        Self {
            var_deps: DiGraph::new(),
            var_indices: FxHashMap::default(),
        }
    }

    /// Analyze a query and return optimization opportunities.
    pub fn analyze(&mut self, query: &Query) -> Result<AnalysisResult> {
        // Extract variables from CONSTRUCT and WHERE
        let construct_vars = self.extract_construct_vars(&query.construct);
        let bound_vars = self.extract_bound_vars(&query.where_clause);

        // Find unused variables
        let unused_vars = self.find_unused_vars(&construct_vars, &bound_vars);

        // Analyze WHERE clause patterns
        let patterns = self.extract_basic_patterns(&query.where_clause);
        let redundant_patterns = self.find_redundant_patterns(&patterns);
        let parallel_groups = self.find_parallel_groups(&patterns);
        let join_graph = self.build_join_graph(&patterns);
        let selectivities = self.estimate_selectivities(&patterns);

        Ok(AnalysisResult {
            unused_vars,
            redundant_patterns,
            parallel_groups,
            construct_vars,
            bound_vars,
            join_graph,
            selectivities,
        })
    }

    /// Extract variables used in CONSTRUCT template.
    fn extract_construct_vars(&self, template: &ConstructTemplate) -> IndexSet<String> {
        let mut vars = IndexSet::new();
        for pattern in &template.patterns {
            self.collect_pattern_vars(pattern, &mut vars);
        }
        vars
    }

    /// Extract variables bound in WHERE clause.
    fn extract_bound_vars(&self, pattern: &GraphPattern) -> IndexSet<String> {
        let mut vars = IndexSet::new();
        self.collect_graph_pattern_vars(pattern, &mut vars);
        vars
    }

    /// Collect variables from a triple pattern.
    fn collect_pattern_vars(&self, pattern: &TriplePattern, vars: &mut IndexSet<String>) {
        if let Some(v) = pattern.subject.as_var() {
            vars.insert(v.name.clone());
        }
        if let Some(v) = pattern.predicate.as_var() {
            vars.insert(v.name.clone());
        }
        if let Some(v) = pattern.object.as_var() {
            vars.insert(v.name.clone());
        }
    }

    /// Collect variables from a graph pattern recursively.
    fn collect_graph_pattern_vars(&self, pattern: &GraphPattern, vars: &mut IndexSet<String>) {
        match pattern {
            GraphPattern::Basic(patterns) => {
                for p in patterns {
                    self.collect_pattern_vars(p, vars);
                }
            }
            GraphPattern::Optional(p) => self.collect_graph_pattern_vars(p, vars),
            GraphPattern::Union(left, right) => {
                self.collect_graph_pattern_vars(left, vars);
                self.collect_graph_pattern_vars(right, vars);
            }
            GraphPattern::Filter { pattern, .. } => {
                self.collect_graph_pattern_vars(pattern, vars);
            }
            GraphPattern::Group(patterns) => {
                for p in patterns {
                    self.collect_graph_pattern_vars(p, vars);
                }
            }
            GraphPattern::Bind { var, .. } => {
                vars.insert(var.name.clone());
            }
        }
    }

    /// Find variables that are bound but never used in CONSTRUCT.
    fn find_unused_vars(
        &self,
        construct_vars: &IndexSet<String>,
        bound_vars: &IndexSet<String>,
    ) -> Vec<String> {
        bound_vars
            .iter()
            .filter(|v| !construct_vars.contains(*v))
            .cloned()
            .collect()
    }

    /// Extract all basic triple patterns from WHERE clause.
    fn extract_basic_patterns(&self, pattern: &GraphPattern) -> Vec<TriplePattern> {
        let mut patterns = Vec::new();
        self.extract_patterns_recursive(pattern, &mut patterns);
        patterns
    }

    /// Recursively extract triple patterns.
    fn extract_patterns_recursive(&self, pattern: &GraphPattern, result: &mut Vec<TriplePattern>) {
        match pattern {
            GraphPattern::Basic(patterns) => result.extend(patterns.clone()),
            GraphPattern::Optional(p) => self.extract_patterns_recursive(p, result),
            GraphPattern::Union(left, right) => {
                self.extract_patterns_recursive(left, result);
                self.extract_patterns_recursive(right, result);
            }
            GraphPattern::Filter { pattern, .. } => {
                self.extract_patterns_recursive(pattern, result);
            }
            GraphPattern::Group(patterns) => {
                for p in patterns {
                    self.extract_patterns_recursive(p, result);
                }
            }
            GraphPattern::Bind { .. } => {}
        }
    }

    /// Find redundant triple patterns (patterns that are subsumed by others).
    fn find_redundant_patterns(&self, patterns: &[TriplePattern]) -> Vec<usize> {
        let mut redundant = Vec::new();

        for (i, p1) in patterns.iter().enumerate() {
            for (j, p2) in patterns.iter().enumerate() {
                if i != j && self.is_subsumed(p1, p2) {
                    redundant.push(i);
                    break;
                }
            }
        }

        redundant
    }

    /// Check if pattern p1 is subsumed by pattern p2.
    /// A pattern is subsumed if it matches fewer triples than another pattern with same variables.
    fn is_subsumed(&self, p1: &TriplePattern, p2: &TriplePattern) -> bool {
        // Simple heuristic: if patterns are identical, one is redundant
        p1 == p2
    }

    /// Find groups of patterns that can be executed in parallel.
    /// Patterns can run in parallel if they don't share variables.
    fn find_parallel_groups(&self, patterns: &[TriplePattern]) -> Vec<Vec<usize>> {
        let mut groups = Vec::new();
        let mut assigned = vec![false; patterns.len()];

        for i in 0..patterns.len() {
            if assigned[i] {
                continue;
            }

            let mut group = vec![i];
            assigned[i] = true;

            for j in (i + 1)..patterns.len() {
                if assigned[j] {
                    continue;
                }

                let vars_j = self.pattern_vars(&patterns[j]);

                // Check if j shares variables with any pattern in the group
                let mut independent = true;
                for &k in &group {
                    let vars_k = self.pattern_vars(&patterns[k]);
                    if vars_k.iter().any(|v| vars_j.contains(v)) {
                        independent = false;
                        break;
                    }
                }

                if independent {
                    group.push(j);
                    assigned[j] = true;
                }
            }

            groups.push(group);
        }

        groups
    }

    /// Get all variables in a pattern.
    fn pattern_vars(&self, pattern: &TriplePattern) -> Vec<String> {
        let mut vars = Vec::new();
        if let Some(v) = pattern.subject.as_var() {
            vars.push(v.name.clone());
        }
        if let Some(v) = pattern.predicate.as_var() {
            vars.push(v.name.clone());
        }
        if let Some(v) = pattern.object.as_var() {
            vars.push(v.name.clone());
        }
        vars
    }

    /// Build a join graph showing which patterns share variables.
    fn build_join_graph(&self, patterns: &[TriplePattern]) -> DiGraph<usize, String> {
        let mut graph = DiGraph::new();
        let mut nodes = Vec::new();

        // Create a node for each pattern
        for i in 0..patterns.len() {
            nodes.push(graph.add_node(i));
        }

        // Add edges for shared variables
        for i in 0..patterns.len() {
            let vars_i = self.pattern_vars(&patterns[i]);
            for j in (i + 1)..patterns.len() {
                let vars_j = self.pattern_vars(&patterns[j]);

                for var in &vars_i {
                    if vars_j.contains(var) {
                        graph.add_edge(nodes[i], nodes[j], var.clone());
                    }
                }
            }
        }

        graph
    }

    /// Estimate selectivity for each pattern.
    /// Lower selectivity = more selective = should be executed first.
    fn estimate_selectivities(&self, patterns: &[TriplePattern]) -> Vec<f64> {
        patterns
            .iter()
            .map(|p| self.estimate_pattern_selectivity(p))
            .collect()
    }

    /// Estimate selectivity of a single pattern.
    fn estimate_pattern_selectivity(&self, pattern: &TriplePattern) -> f64 {
        // Heuristic: fewer variables = more selective
        let var_count = [&pattern.subject, &pattern.predicate, &pattern.object]
            .iter()
            .filter(|t| matches!(t, Term::Var(_)))
            .count();

        match var_count {
            0 => 0.01, // Ground triple - highly selective
            1 => 0.1,  // One variable
            2 => 0.4,  // Two variables
            3 => 0.9,  // Three variables - least selective
            _ => 1.0,
        }
    }

    /// Check if the query has tensor product structure (independent subqueries).
    pub fn find_tensor_decomposition(&self, patterns: &[TriplePattern]) -> Vec<Vec<usize>> {
        // Build connected components in the join graph
        let join_graph = self.build_join_graph(patterns);
        let mut components = Vec::new();
        let mut visited = vec![false; patterns.len()];

        for i in 0..patterns.len() {
            if visited[i] {
                continue;
            }

            let mut component = Vec::new();
            let mut dfs = Dfs::new(&join_graph, NodeIndex::new(i));

            while let Some(node) = dfs.next(&join_graph) {
                let idx = join_graph[node];
                if !visited[idx] {
                    visited[idx] = true;
                    component.push(idx);
                }
            }

            components.push(component);
        }

        components
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisResult {
    /// Check if there are any optimization opportunities.
    pub fn has_optimizations(&self) -> bool {
        !self.unused_vars.is_empty()
            || !self.redundant_patterns.is_empty()
            || self.parallel_groups.len() > 1
    }

    /// Get a summary of the analysis.
    pub fn summary(&self) -> String {
        format!(
            "Analysis: {} unused vars, {} redundant patterns, {} parallel groups, {} joins",
            self.unused_vars.len(),
            self.redundant_patterns.len(),
            self.parallel_groups.len(),
            self.join_graph.edge_count()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_vars() {
        let pattern = TriplePattern::new(
            Term::var("s"),
            Term::iri("http://example.org/pred"),
            Term::var("o"),
        );

        let analyzer = Analyzer::new();
        let vars = analyzer.pattern_vars(&pattern);
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&"s".to_string()));
        assert!(vars.contains(&"o".to_string()));
    }

    #[test]
    fn test_selectivity_estimation() {
        let analyzer = Analyzer::new();

        // Ground triple (no variables)
        let p1 = TriplePattern::new(
            Term::iri("http://example.org/s"),
            Term::iri("http://example.org/p"),
            Term::iri("http://example.org/o"),
        );
        assert!(analyzer.estimate_pattern_selectivity(&p1) < 0.1);

        // All variables
        let p2 = TriplePattern::new(Term::var("s"), Term::var("p"), Term::var("o"));
        assert!(analyzer.estimate_pattern_selectivity(&p2) > 0.8);
    }

    #[test]
    fn test_parallel_groups() {
        let analyzer = Analyzer::new();

        // Two independent patterns
        let p1 = TriplePattern::new(
            Term::var("s1"),
            Term::iri("http://example.org/p"),
            Term::var("o1"),
        );
        let p2 = TriplePattern::new(
            Term::var("s2"),
            Term::iri("http://example.org/p"),
            Term::var("o2"),
        );

        let patterns = vec![p1, p2];
        let groups = analyzer.find_parallel_groups(&patterns);

        // Should be in the same group since they're independent
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }
}
