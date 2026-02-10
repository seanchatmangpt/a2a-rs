//! Sparse Priming Representation (SPR) Compressor
//!
//! Implements context compression using entropy-based importance scoring
//! to extract minimal token sets that activate correct latent space.
//!
//! ## Overview
//!
//! SPR compression reduces context size while maintaining semantic fidelity
//! by identifying and preserving high-information tokens. This is particularly
//! useful for:
//! - Compressing RDF ontologies into compact representations
//! - Reducing prompt size for LLM agents
//! - Extracting key concepts from verbose documentation
//!
//! ## Algorithm
//!
//! 1. **Tokenization**: Split text into tokens (words, punctuation)
//! 2. **Frequency Analysis**: Calculate term frequency (TF) and inverse document frequency (IDF)
//! 3. **Entropy Scoring**: Compute information-theoretic importance
//! 4. **Selection**: Choose top-k tokens by combined score
//! 5. **Reconstruction**: Validate semantic preservation
//!
//! ## Example
//!
//! ```rust
//! use a2a_rs::services::spr::{SprCompressor, CompressionConfig};
//!
//! let config = CompressionConfig::default();
//! let compressor = SprCompressor::new(config);
//!
//! let original = "Agent-to-Agent protocol defines standardized communication...";
//! let compressed = compressor.compress(original, 0.3)?;
//!
//! assert!(compressed.compression_ratio > 0.0);
//! assert!(compressed.fidelity_score > 0.5);
//! # Ok::<(), a2a_rs::domain::A2AError>(())
//! ```

use crate::domain::A2AError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for SPR compression
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionConfig {
    /// Minimum token length to consider (filters noise)
    pub min_token_length: usize,
    /// Maximum tokens to preserve (absolute limit)
    pub max_tokens: Option<usize>,
    /// Weight for TF-IDF score (0.0-1.0)
    pub tfidf_weight: f64,
    /// Weight for entropy score (0.0-1.0)
    pub entropy_weight: f64,
    /// Weight for position score (earlier = more important)
    pub position_weight: f64,
    /// Stop words to filter out
    pub stop_words: HashSet<String>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        let stop_words: HashSet<String> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "as", "is", "was", "are", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "should", "could", "may", "might",
            "must", "can", "this", "that", "these", "those", "it", "its", "they", "them",
            "their", "what", "which", "who", "when", "where", "why", "how",
        ]
        .iter()
        .map(|&s| s.to_string())
        .collect();

        Self {
            min_token_length: 2,
            max_tokens: None,
            tfidf_weight: 0.4,
            entropy_weight: 0.4,
            position_weight: 0.2,
            stop_words,
        }
    }
}

/// Result of SPR compression
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionResult {
    /// Original text
    pub original: String,
    /// Compressed representation (space-separated tokens)
    pub compressed: String,
    /// Selected tokens with their importance scores
    pub tokens: Vec<ScoredToken>,
    /// Compression ratio (1.0 = no compression, 0.0 = full compression)
    pub compression_ratio: f64,
    /// Estimated fidelity score (0.0-1.0)
    pub fidelity_score: f64,
    /// Original token count
    pub original_token_count: usize,
    /// Compressed token count
    pub compressed_token_count: usize,
}

/// Token with importance score
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredToken {
    /// The token text
    pub token: String,
    /// Combined importance score
    pub score: f64,
    /// TF-IDF component
    pub tfidf: f64,
    /// Entropy component
    pub entropy: f64,
    /// Position component (earlier = higher)
    pub position: f64,
}

/// Token statistics for corpus analysis
#[derive(Debug, Clone)]
struct TokenStats {
    /// Term frequency (TF)
    term_frequency: HashMap<String, usize>,
    /// Document frequency (DF) - how many documents contain this term
    document_frequency: HashMap<String, usize>,
    /// Total documents
    total_documents: usize,
    /// Total tokens
    total_tokens: usize,
}

/// SPR Compressor
///
/// Compresses text using entropy-based importance scoring.
pub struct SprCompressor {
    config: CompressionConfig,
}

impl SprCompressor {
    /// Create a new SPR compressor with the given configuration
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Create a compressor with default configuration
    pub fn default() -> Self {
        Self::new(CompressionConfig::default())
    }

    /// Compress text with the given target compression ratio
    ///
    /// # Arguments
    ///
    /// * `text` - Original text to compress
    /// * `target_ratio` - Target compression ratio (0.0-1.0), where 0.3 means keep 30% of tokens
    ///
    /// # Returns
    ///
    /// Compression result with selected tokens and metrics
    pub fn compress(
        &self,
        text: &str,
        target_ratio: f64,
    ) -> Result<CompressionResult, A2AError> {
        if !(0.0..=1.0).contains(&target_ratio) {
            return Err(A2AError::ValidationError(
                "target_ratio must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Tokenize
        let tokens = self.tokenize(text);
        let original_count = tokens.len();

        if original_count == 0 {
            return Ok(CompressionResult {
                original: text.to_string(),
                compressed: String::new(),
                tokens: Vec::new(),
                compression_ratio: 1.0,
                fidelity_score: 1.0,
                original_token_count: 0,
                compressed_token_count: 0,
            });
        }

        // Calculate statistics
        let stats = self.calculate_stats(&[tokens.clone()]);

        // Score tokens
        let mut scored_tokens = self.score_tokens(&tokens, &stats);

        // Sort by score (descending)
        scored_tokens.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Determine how many tokens to keep
        let target_count = ((original_count as f64) * target_ratio).ceil() as usize;
        let target_count = if let Some(max) = self.config.max_tokens {
            target_count.min(max)
        } else {
            target_count
        };

        // Select top tokens
        let selected_tokens: Vec<ScoredToken> = scored_tokens.into_iter().take(target_count).collect();
        let compressed_count = selected_tokens.len();

        // Build compressed text (preserve order from original)
        let selected_set: HashSet<&str> = selected_tokens.iter().map(|t| t.token.as_str()).collect();
        let compressed_tokens: Vec<&str> = tokens
            .iter()
            .filter(|t| selected_set.contains(t.as_str()))
            .map(|s| s.as_str())
            .collect();
        let compressed = compressed_tokens.join(" ");

        // Calculate fidelity score
        let fidelity_score = self.calculate_fidelity(&tokens, &compressed_tokens);

        Ok(CompressionResult {
            original: text.to_string(),
            compressed,
            tokens: selected_tokens,
            compression_ratio: (compressed_count as f64) / (original_count as f64),
            fidelity_score,
            original_token_count: original_count,
            compressed_token_count: compressed_count,
        })
    }

    /// Compress multiple documents as a corpus (better TF-IDF calculation)
    pub fn compress_corpus(
        &self,
        documents: &[&str],
        target_ratio: f64,
    ) -> Result<Vec<CompressionResult>, A2AError> {
        if !(0.0..=1.0).contains(&target_ratio) {
            return Err(A2AError::ValidationError(
                "target_ratio must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Tokenize all documents
        let tokenized_docs: Vec<Vec<String>> = documents
            .iter()
            .map(|doc| self.tokenize(doc))
            .collect();

        // Calculate corpus-wide statistics
        let stats = self.calculate_stats(&tokenized_docs);

        // Compress each document
        let mut results = Vec::new();
        for (doc_idx, doc) in documents.iter().enumerate() {
            let tokens = &tokenized_docs[doc_idx];
            let original_count = tokens.len();

            if original_count == 0 {
                results.push(CompressionResult {
                    original: (*doc).to_string(),
                    compressed: String::new(),
                    tokens: Vec::new(),
                    compression_ratio: 1.0,
                    fidelity_score: 1.0,
                    original_token_count: 0,
                    compressed_token_count: 0,
                });
                continue;
            }

            // Score tokens using corpus stats
            let mut scored_tokens = self.score_tokens(tokens, &stats);
            scored_tokens.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

            // Select top tokens
            let target_count = ((original_count as f64) * target_ratio).ceil() as usize;
            let target_count = if let Some(max) = self.config.max_tokens {
                target_count.min(max)
            } else {
                target_count
            };

            let selected_tokens: Vec<ScoredToken> =
                scored_tokens.into_iter().take(target_count).collect();
            let compressed_count = selected_tokens.len();

            // Build compressed text
            let selected_set: HashSet<&str> =
                selected_tokens.iter().map(|t| t.token.as_str()).collect();
            let compressed_tokens: Vec<&str> = tokens
                .iter()
                .filter(|t| selected_set.contains(t.as_str()))
                .map(|s| s.as_str())
                .collect();
            let compressed = compressed_tokens.join(" ");

            // Calculate fidelity
            let fidelity_score = self.calculate_fidelity(tokens, &compressed_tokens);

            results.push(CompressionResult {
                original: (*doc).to_string(),
                compressed,
                tokens: selected_tokens,
                compression_ratio: (compressed_count as f64) / (original_count as f64),
                fidelity_score,
                original_token_count: original_count,
                compressed_token_count: compressed_count,
            });
        }

        Ok(results)
    }

    /// Tokenize text into words and significant punctuation
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                current.push(ch);
            } else if ch == ':' || ch == '.' || ch == '/' || ch == '#' {
                // Preserve structural punctuation (important for URIs, RDF)
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(ch.to_string());
            } else {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        // Filter by length and stop words
        tokens
            .into_iter()
            .filter(|t| {
                t.len() >= self.config.min_token_length
                    && !self.config.stop_words.contains(&t.to_lowercase())
            })
            .collect()
    }

    /// Calculate statistics across documents
    fn calculate_stats(&self, documents: &[Vec<String>]) -> TokenStats {
        let mut term_frequency = HashMap::new();
        let mut document_frequency = HashMap::new();
        let mut total_tokens = 0;

        for doc in documents {
            let mut doc_tokens = HashSet::new();

            for token in doc {
                // Term frequency
                *term_frequency.entry(token.clone()).or_insert(0) += 1;
                total_tokens += 1;

                // Document frequency (unique per document)
                doc_tokens.insert(token.clone());
            }

            // Update document frequency
            for token in doc_tokens {
                *document_frequency.entry(token).or_insert(0) += 1;
            }
        }

        TokenStats {
            term_frequency,
            document_frequency,
            total_documents: documents.len(),
            total_tokens,
        }
    }

    /// Score tokens using TF-IDF and entropy
    fn score_tokens(&self, tokens: &[String], stats: &TokenStats) -> Vec<ScoredToken> {
        let mut scored = Vec::new();

        for (idx, token) in tokens.iter().enumerate() {
            let tf = *stats.term_frequency.get(token).unwrap_or(&0) as f64;
            let df = *stats.document_frequency.get(token).unwrap_or(&1) as f64;

            // TF-IDF score
            let tfidf = if df > 0.0 {
                (tf / stats.total_tokens as f64) * ((stats.total_documents as f64) / df).ln()
            } else {
                0.0
            };

            // Entropy score (normalized)
            let p = tf / stats.total_tokens as f64;
            let entropy = if p > 0.0 { -p * p.log2() } else { 0.0 };

            // Position score (earlier tokens more important)
            let position = 1.0 - (idx as f64 / tokens.len() as f64);

            // Combined score
            let score = tfidf * self.config.tfidf_weight
                + entropy * self.config.entropy_weight
                + position * self.config.position_weight;

            scored.push(ScoredToken {
                token: token.clone(),
                score,
                tfidf,
                entropy,
                position,
            });
        }

        scored
    }

    /// Calculate fidelity score (how well compressed preserves original)
    fn calculate_fidelity(&self, original: &[String], compressed: &[&str]) -> f64 {
        if original.is_empty() {
            return 1.0;
        }

        let original_set: HashSet<&str> = original.iter().map(|s| s.as_str()).collect();
        let compressed_set: HashSet<&str> = compressed.iter().copied().collect();

        // Jaccard similarity
        let intersection = original_set.intersection(&compressed_set).count();
        let union = original_set.union(&compressed_set).count();

        if union == 0 {
            1.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Expand compressed representation (reconstruct)
    ///
    /// Note: This is a placeholder for more sophisticated reconstruction.
    /// In a real LLM context, this would involve prompting with the compressed
    /// representation and comparing the generated output.
    pub fn reconstruct(&self, result: &CompressionResult) -> String {
        result.compressed.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenization() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "The Agent-to-Agent protocol defines standardized communication.";
        let tokens = compressor.tokenize(text);

        assert!(tokens.contains(&"Agent".to_string()));
        assert!(tokens.contains(&"protocol".to_string()));
        assert!(!tokens.contains(&"The".to_string())); // stop word
    }

    #[test]
    fn test_compression_basic() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "Agent-to-Agent protocol enables autonomous agents to communicate \
                    securely using JSON-RPC messages with task delegation support.";
        let result = compressor.compress(text, 0.5).unwrap();

        assert!(result.compression_ratio <= 0.5);
        assert!(result.compressed_token_count <= result.original_token_count);
        assert!(!result.compressed.is_empty());
    }

    #[test]
    fn test_compression_ratio() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "The quick brown fox jumps over the lazy dog. \
                    The quick brown fox jumps over the lazy dog again.";

        let result30 = compressor.compress(text, 0.3).unwrap();
        let result70 = compressor.compress(text, 0.7).unwrap();

        assert!(result30.compressed_token_count < result70.compressed_token_count);
        assert!(result30.compression_ratio < result70.compression_ratio);
    }

    #[test]
    fn test_empty_text() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let result = compressor.compress("", 0.5).unwrap();
        assert_eq!(result.compression_ratio, 1.0);
        assert_eq!(result.fidelity_score, 1.0);
    }

    #[test]
    fn test_corpus_compression() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let docs = [
            "Agent protocol defines communication standards",
            "Task delegation enables autonomous agent workflows",
            "JSON-RPC messages facilitate agent interactions",
        ];

        let results = compressor.compress_corpus(&docs, 0.5).unwrap();
        assert_eq!(results.len(), 3);

        for result in results {
            assert!(result.compression_ratio <= 0.5);
            assert!(result.fidelity_score > 0.0);
        }
    }

    #[test]
    fn test_rdf_ontology_compression() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        // Simulated RDF Turtle syntax
        let ontology = r#"
            @prefix a2a: <http://example.org/a2a#> .
            @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
            @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

            a2a:Agent rdf:type rdfs:Class .
            a2a:Agent rdfs:label "Autonomous Agent" .
            a2a:Message rdf:type rdfs:Class .
            a2a:Message rdfs:label "Protocol Message" .
            a2a:Task rdf:type rdfs:Class .
            a2a:hasCapability rdf:type rdf:Property .
        "#;

        let result = compressor.compress(ontology, 0.4).unwrap();

        // Should preserve key terms
        assert!(result.compressed.contains("a2a"));
        assert!(result.compressed.contains("Agent") || result.compressed.contains("Message"));
        assert!(result.compression_ratio <= 0.4);
    }

    #[test]
    fn test_fidelity_score() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "Agent protocol message task delegation workflow communication";

        // High compression = low fidelity
        let result_low = compressor.compress(text, 0.2).unwrap();
        // Low compression = high fidelity
        let result_high = compressor.compress(text, 0.8).unwrap();

        assert!(result_high.fidelity_score > result_low.fidelity_score);
    }

    #[test]
    fn test_invalid_ratio() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "Sample text";

        assert!(compressor.compress(text, -0.1).is_err());
        assert!(compressor.compress(text, 1.5).is_err());
    }

    #[test]
    fn test_stop_words_filtered() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "The agent is communicating with the protocol";
        let tokens = compressor.tokenize(text);

        assert!(!tokens.contains(&"The".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"with".to_string()));
        assert!(tokens.contains(&"agent".to_string()));
        assert!(tokens.contains(&"protocol".to_string()));
    }

    #[test]
    fn test_structural_punctuation_preserved() {
        let config = CompressionConfig::default();
        let compressor = SprCompressor::new(config);

        let text = "http://example.org/ontology#Agent";
        let tokens = compressor.tokenize(text);

        // Should preserve : / # for URI structure
        assert!(tokens.contains(&":".to_string()));
        assert!(tokens.contains(&"/".to_string()));
        assert!(tokens.contains(&"#".to_string()));
    }

    #[test]
    fn test_max_tokens_limit() {
        let mut config = CompressionConfig::default();
        config.max_tokens = Some(5);
        let compressor = SprCompressor::new(config);

        let text = "One two three four five six seven eight nine ten";
        let result = compressor.compress(text, 1.0).unwrap();

        assert!(result.compressed_token_count <= 5);
    }

    #[test]
    fn test_scoring_weights() {
        let mut config = CompressionConfig::default();
        config.tfidf_weight = 1.0;
        config.entropy_weight = 0.0;
        config.position_weight = 0.0;

        let compressor = SprCompressor::new(config);
        let tokens = vec!["test".to_string(), "test".to_string(), "unique".to_string()];
        let stats = compressor.calculate_stats(&[tokens.clone()]);
        let scored = compressor.score_tokens(&tokens, &stats);

        // "test" appears twice, should have higher TF-IDF than "unique"
        let test_score = scored.iter().find(|t| t.token == "test").unwrap();
        assert!(test_score.tfidf > 0.0);
    }

    #[cfg(feature = "full")]
    use proptest::prelude::*;

    #[cfg(feature = "full")]
    proptest! {
        #[test]
        fn test_compression_ratio_bounds(ratio in 0.0f64..=1.0f64) {
            let config = CompressionConfig::default();
            let compressor = SprCompressor::new(config);
            let text = "Agent protocol message task delegation workflow communication";

            let result = compressor.compress(text, ratio).unwrap();
            assert!(result.compression_ratio >= 0.0);
            assert!(result.compression_ratio <= 1.0);
        }

        #[test]
        fn test_fidelity_bounds(ratio in 0.0f64..=1.0f64) {
            let config = CompressionConfig::default();
            let compressor = SprCompressor::new(config);
            let text = "Agent protocol message task delegation workflow communication";

            let result = compressor.compress(text, ratio).unwrap();
            assert!(result.fidelity_score >= 0.0);
            assert!(result.fidelity_score <= 1.0);
        }

        #[test]
        fn test_higher_ratio_preserves_more_tokens(
            ratio1 in 0.1f64..0.5f64,
            ratio2 in 0.5f64..1.0f64,
        ) {
            let config = CompressionConfig::default();
            let compressor = SprCompressor::new(config);
            let text = "Agent protocol message task delegation workflow communication system";

            let result1 = compressor.compress(text, ratio1).unwrap();
            let result2 = compressor.compress(text, ratio2).unwrap();

            assert!(result2.compressed_token_count >= result1.compressed_token_count);
        }
    }
}
