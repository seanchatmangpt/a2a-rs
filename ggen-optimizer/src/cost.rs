//! Cost model for query optimization.
//!
//! Estimates the cost of executing queries based on:
//! - Triple pattern selectivity
//! - Join cardinality
//! - Operator costs (filter, bind, optional)

use crate::ast::*;
use crate::error::Result;
use rustc_hash::FxHashMap;

/// Cost model for queries.
#[derive(Debug, Clone)]
pub struct CostModel {
    /// Estimated cardinality of each predicate.
    predicate_stats: FxHashMap<String, PredicateStats>,
    /// Base cost for different operations.
    op_costs: OpCosts,
}

/// Statistics about a predicate.
#[derive(Debug, Clone)]
pub struct PredicateStats {
    /// Number of triples with this predicate.
    pub count: u64,
    /// Average number of distinct subjects.
    pub distinct_subjects: u64,
    /// Average number of distinct objects.
    pub distinct_objects: u64,
}

/// Base costs for different operations.
#[derive(Debug, Clone)]
pub struct OpCosts {
    /// Cost per triple pattern scan.
    pub scan: f64,
    /// Cost per join operation.
    pub join: f64,
    /// Cost per filter evaluation.
    pub filter: f64,
    /// Cost per optional pattern.
    pub optional: f64,
    /// Cost per union branch.
    pub union: f64,
    /// Cost per bind operation.
    pub bind: f64,
}

/// Estimated cost of a query.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryCost {
    /// Total estimated cost.
    pub total: f64,
    /// Estimated number of intermediate results.
    pub cardinality: u64,
    /// Per-operator costs.
    pub breakdown: Vec<(String, f64)>,
}

impl CostModel {
    /// Create a new cost model with default statistics.
    pub fn new() -> Self {
        Self {
            predicate_stats: FxHashMap::default(),
            op_costs: OpCosts::default(),
        }
    }

    /// Create a cost model with custom operation costs.
    pub fn with_op_costs(op_costs: OpCosts) -> Self {
        Self {
            predicate_stats: FxHashMap::default(),
            op_costs,
        }
    }

    /// Add predicate statistics.
    pub fn add_predicate_stats(&mut self, predicate: String, stats: PredicateStats) {
        self.predicate_stats.insert(predicate, stats);
    }

    /// Estimate the cost of a query.
    pub fn estimate_cost(&self, query: &Query) -> Result<QueryCost> {
        let mut breakdown = Vec::new();

        // Cost of CONSTRUCT template (negligible compared to WHERE)
        let construct_cost = self.op_costs.bind * query.construct.patterns.len() as f64;
        breakdown.push(("CONSTRUCT".to_string(), construct_cost));

        // Cost of WHERE clause
        let (where_cost, cardinality) = self.estimate_pattern_cost(&query.where_clause)?;
        breakdown.push(("WHERE".to_string(), where_cost));

        let total = construct_cost + where_cost;

        Ok(QueryCost {
            total,
            cardinality,
            breakdown,
        })
    }

    /// Estimate the cost of a graph pattern.
    fn estimate_pattern_cost(&self, pattern: &GraphPattern) -> Result<(f64, u64)> {
        match pattern {
            GraphPattern::Basic(patterns) => self.estimate_basic_pattern_cost(patterns),
            GraphPattern::Optional(p) => {
                let (cost, card) = self.estimate_pattern_cost(p)?;
                Ok((cost + self.op_costs.optional, card))
            }
            GraphPattern::Union(left, right) => {
                let (left_cost, left_card) = self.estimate_pattern_cost(left)?;
                let (right_cost, right_card) = self.estimate_pattern_cost(right)?;
                let total_cost = left_cost + right_cost + self.op_costs.union;
                let total_card = left_card + right_card;
                Ok((total_cost, total_card))
            }
            GraphPattern::Filter { pattern, .. } => {
                let (cost, card) = self.estimate_pattern_cost(pattern)?;
                // Filter reduces cardinality (assume 50% selectivity)
                let filtered_card = card / 2;
                Ok((cost + self.op_costs.filter, filtered_card))
            }
            GraphPattern::Group(patterns) => {
                let mut total_cost = 0.0;
                let mut cardinality = 1;

                for p in patterns {
                    let (cost, card) = self.estimate_pattern_cost(p)?;
                    total_cost += cost;
                    cardinality = (cardinality * card).min(1_000_000); // Cap to avoid overflow
                }

                Ok((total_cost, cardinality))
            }
            GraphPattern::Bind { .. } => Ok((self.op_costs.bind, 1)),
        }
    }

    /// Estimate the cost of a basic graph pattern (set of triple patterns).
    fn estimate_basic_pattern_cost(&self, patterns: &[TriplePattern]) -> Result<(f64, u64)> {
        if patterns.is_empty() {
            return Ok((0.0, 0));
        }

        // Estimate cost assuming left-deep join tree
        let mut total_cost = 0.0;
        let mut cardinality = self.estimate_triple_cardinality(&patterns[0]);

        // First pattern: scan cost
        total_cost += self.op_costs.scan;

        // Subsequent patterns: join cost
        for pattern in &patterns[1..] {
            let pattern_card = self.estimate_triple_cardinality(pattern);
            total_cost += self.op_costs.scan + self.op_costs.join * cardinality as f64;

            // Join cardinality (simplified: product divided by domain size)
            cardinality = (cardinality * pattern_card) / 100; // Assume join selectivity
            cardinality = cardinality.max(1);
        }

        Ok((total_cost, cardinality))
    }

    /// Estimate the cardinality (number of results) of a triple pattern.
    fn estimate_triple_cardinality(&self, pattern: &TriplePattern) -> u64 {
        // Get predicate name if it's an IRI or prefixed name
        let pred_name = match &pattern.predicate {
            Term::Iri(iri) => Some(iri.iri.clone()),
            Term::PrefixedName { prefix, local } => Some(format!("{}:{}", prefix, local)),
            _ => None,
        };

        // Look up statistics
        if let Some(name) = pred_name {
            if let Some(stats) = self.predicate_stats.get(&name) {
                return self.estimate_with_stats(pattern, stats);
            }
        }

        // Default estimates based on pattern structure
        self.estimate_default_cardinality(pattern)
    }

    /// Estimate cardinality using predicate statistics.
    fn estimate_with_stats(&self, pattern: &TriplePattern, stats: &PredicateStats) -> u64 {
        match (&pattern.subject, &pattern.object) {
            (Term::Var(_), Term::Var(_)) => stats.count,
            (Term::Var(_), _) => stats.distinct_subjects,
            (_, Term::Var(_)) => stats.distinct_objects,
            _ => 1, // Ground triple
        }
    }

    /// Estimate cardinality without statistics (heuristic).
    fn estimate_default_cardinality(&self, pattern: &TriplePattern) -> u64 {
        let var_count = [&pattern.subject, &pattern.predicate, &pattern.object]
            .iter()
            .filter(|t| matches!(t, Term::Var(_)))
            .count();

        match var_count {
            0 => 1,      // Ground triple
            1 => 100,    // One variable
            2 => 1_000,  // Two variables
            3 => 10_000, // Three variables
            _ => 100_000,
        }
    }

    /// Compare two query costs.
    pub fn compare_costs(&self, cost1: &QueryCost, cost2: &QueryCost) -> std::cmp::Ordering {
        cost1
            .total
            .partial_cmp(&cost2.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    }

    /// Estimate the speedup from parallelization.
    pub fn estimate_parallel_speedup(&self, num_groups: usize) -> f64 {
        if num_groups <= 1 {
            return 1.0;
        }

        // Amdahl's law with assumed 80% parallelizable fraction
        let p = 0.8;
        let n = num_groups as f64;
        1.0 / ((1.0 - p) + (p / n))
    }
}

impl Default for CostModel {
    fn default() -> Self {
        Self::new()
    }
}

impl OpCosts {
    /// Default operation costs.
    pub fn new() -> Self {
        Self {
            scan: 1.0,
            join: 10.0,
            filter: 0.5,
            optional: 5.0,
            union: 2.0,
            bind: 0.1,
        }
    }
}

impl Default for OpCosts {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryCost {
    /// Create a new query cost.
    pub fn new(total: f64, cardinality: u64) -> Self {
        Self {
            total,
            cardinality,
            breakdown: Vec::new(),
        }
    }

    /// Check if this cost is less than another.
    pub fn is_cheaper_than(&self, other: &QueryCost) -> bool {
        self.total < other.total
    }

    /// Get the cost improvement ratio.
    pub fn improvement_ratio(&self, other: &QueryCost) -> f64 {
        if other.total == 0.0 {
            return 1.0;
        }
        other.total / self.total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_model_default() {
        let model = CostModel::new();
        assert_eq!(model.op_costs.scan, 1.0);
        assert_eq!(model.op_costs.join, 10.0);
    }

    #[test]
    fn test_cardinality_estimation() {
        let model = CostModel::new();

        // Ground triple
        let p1 = TriplePattern::new(
            Term::iri("http://example.org/s"),
            Term::iri("http://example.org/p"),
            Term::iri("http://example.org/o"),
        );
        assert_eq!(model.estimate_triple_cardinality(&p1), 1);

        // All variables
        let p2 = TriplePattern::new(Term::var("s"), Term::var("p"), Term::var("o"));
        assert_eq!(model.estimate_triple_cardinality(&p2), 10_000);
    }

    #[test]
    fn test_parallel_speedup() {
        let model = CostModel::new();

        let speedup_1 = model.estimate_parallel_speedup(1);
        assert_eq!(speedup_1, 1.0);

        let speedup_4 = model.estimate_parallel_speedup(4);
        assert!(speedup_4 > 1.0 && speedup_4 < 4.0);
    }

    #[test]
    fn test_cost_comparison() {
        let cost1 = QueryCost::new(100.0, 1000);
        let cost2 = QueryCost::new(200.0, 2000);

        assert!(cost1.is_cheaper_than(&cost2));
        assert!(!cost2.is_cheaper_than(&cost1));
        assert_eq!(cost1.improvement_ratio(&cost2), 2.0);
    }

    #[test]
    fn test_predicate_stats() {
        let mut model = CostModel::new();
        model.add_predicate_stats(
            "ex:name".to_string(),
            PredicateStats {
                count: 1000,
                distinct_subjects: 500,
                distinct_objects: 300,
            },
        );

        let pattern =
            TriplePattern::new(Term::var("s"), Term::prefixed("ex", "name"), Term::var("o"));

        let card = model.estimate_triple_cardinality(&pattern);
        assert_eq!(card, 1000);
    }
}
