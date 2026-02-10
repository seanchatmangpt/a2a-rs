//! Compression port - defines the interface for message compression

use async_trait::async_trait;

use crate::domain::A2AError;

/// Compression algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// Gzip compression (RFC 1952)
    Gzip,
    /// Zstandard compression
    Zstd,
    /// No compression
    None,
}

impl CompressionAlgorithm {
    /// Get the content-encoding header value for this algorithm
    pub fn content_encoding(&self) -> Option<&'static str> {
        match self {
            CompressionAlgorithm::Gzip => Some("gzip"),
            CompressionAlgorithm::Zstd => Some("zstd"),
            CompressionAlgorithm::None => None,
        }
    }

    /// Parse from a content-encoding header value
    pub fn from_content_encoding(encoding: &str) -> Option<Self> {
        match encoding.to_lowercase().as_str() {
            "gzip" | "x-gzip" => Some(CompressionAlgorithm::Gzip),
            "zstd" => Some(CompressionAlgorithm::Zstd),
            "identity" | "" => Some(CompressionAlgorithm::None),
            _ => None,
        }
    }
}

/// Configuration for compression behavior
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Compression algorithm to use
    pub algorithm: CompressionAlgorithm,
    /// Compression level (algorithm-specific, typically 0-9 or 0-21)
    pub level: i32,
    /// Minimum size in bytes before compression is applied
    pub min_size: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Gzip,
            level: 6,       // Default compression level
            min_size: 1024, // Only compress messages >= 1KB
        }
    }
}

impl CompressionConfig {
    /// Create a new compression config with the given algorithm
    pub fn new(algorithm: CompressionAlgorithm) -> Self {
        Self {
            algorithm,
            ..Default::default()
        }
    }

    /// Set the compression level
    pub fn with_level(mut self, level: i32) -> Self {
        self.level = level;
        self
    }

    /// Set the minimum size threshold
    pub fn with_min_size(mut self, min_size: usize) -> Self {
        self.min_size = min_size;
        self
    }

    /// Create a fast compression config (lower compression, faster speed)
    pub fn fast(algorithm: CompressionAlgorithm) -> Self {
        Self {
            algorithm,
            level: match algorithm {
                CompressionAlgorithm::Gzip => 1,
                CompressionAlgorithm::Zstd => 1,
                CompressionAlgorithm::None => 0,
            },
            min_size: 1024,
        }
    }

    /// Create a high compression config (higher compression, slower speed)
    pub fn best(algorithm: CompressionAlgorithm) -> Self {
        Self {
            algorithm,
            level: match algorithm {
                CompressionAlgorithm::Gzip => 9,
                CompressionAlgorithm::Zstd => 21,
                CompressionAlgorithm::None => 0,
            },
            min_size: 512,
        }
    }
}

/// Port interface for message compression
#[async_trait]
pub trait MessageCompressor: Send + Sync {
    /// Compress data according to the configuration
    ///
    /// Returns the compressed data and the algorithm used.
    /// If the data is below the minimum size threshold, returns the original data
    /// with CompressionAlgorithm::None.
    async fn compress(
        &self,
        data: &[u8],
        config: &CompressionConfig,
    ) -> Result<(Vec<u8>, CompressionAlgorithm), A2AError>;

    /// Decompress data using the specified algorithm
    ///
    /// The algorithm parameter should be determined from the Content-Encoding header
    /// or other metadata.
    async fn decompress(
        &self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
    ) -> Result<Vec<u8>, A2AError>;

    /// Auto-detect compression algorithm from data and decompress
    ///
    /// Attempts to detect the compression algorithm by examining the data.
    /// Falls back to returning the original data if no compression is detected.
    async fn auto_decompress(&self, data: &[u8]) -> Result<Vec<u8>, A2AError>;
}
