//! Load predictor using exponential weighted moving average
//!
//! Tracks system metrics (latency, error rate, throughput) using EWMA
//! to smooth out short-term fluctuations and provide stable signals
//! for adaptive rate limiting decisions.

use std::time::Duration;

/// Exponential Weighted Moving Average calculator
#[derive(Debug, Clone)]
pub struct Ewma {
    /// Current smoothed value
    value: f64,
    /// Smoothing factor (0.0 to 1.0)
    alpha: f64,
    /// Number of observations
    count: u64,
}

impl Ewma {
    /// Create a new EWMA with the given smoothing factor
    ///
    /// # Arguments
    /// * `alpha` - Smoothing factor (0.0 to 1.0). Higher values give more weight to recent observations.
    ///             Typical values: 0.1 (slow), 0.3 (medium), 0.5 (fast)
    pub fn new(alpha: f64) -> Self {
        Self {
            value: 0.0,
            alpha: alpha.clamp(0.0, 1.0),
            count: 0,
        }
    }

    /// Update the EWMA with a new observation
    pub fn update(&mut self, observation: f64) {
        if self.count == 0 {
            // First observation - initialize with the observed value
            self.value = observation;
        } else {
            // EWMA formula: S_t = α * Y_t + (1 - α) * S_{t-1}
            self.value = self.alpha * observation + (1.0 - self.alpha) * self.value;
        }
        self.count += 1;
    }

    /// Get the current smoothed value
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Get the number of observations
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset the EWMA
    pub fn reset(&mut self) {
        self.value = 0.0;
        self.count = 0;
    }
}

/// Tracks P99 latency using a simple histogram approach
#[derive(Debug, Clone)]
pub struct LatencyTracker {
    /// Recent latency samples (circular buffer)
    samples: Vec<Duration>,
    /// Current write position
    position: usize,
    /// Window size for P99 calculation
    window_size: usize,
}

impl LatencyTracker {
    /// Create a new latency tracker with the given window size
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: Vec::with_capacity(window_size),
            position: 0,
            window_size,
        }
    }

    /// Add a latency sample
    pub fn record(&mut self, latency: Duration) {
        if self.samples.len() < self.window_size {
            self.samples.push(latency);
        } else {
            self.samples[self.position] = latency;
            self.position = (self.position + 1) % self.window_size;
        }
    }

    /// Calculate P99 latency from recent samples
    pub fn p99(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::from_millis(0);
        }

        let mut sorted = self.samples.clone();
        sorted.sort();

        let index = ((sorted.len() as f64) * 0.99).ceil() as usize;
        sorted[index.saturating_sub(1).min(sorted.len() - 1)]
    }

    /// Get the number of samples
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Reset the tracker
    pub fn reset(&mut self) {
        self.samples.clear();
        self.position = 0;
    }
}

/// Load predictor using exponential smoothing for adaptive rate limiting
#[derive(Debug, Clone)]
pub struct LoadPredictor {
    /// Latency tracker for P99 calculation
    latency_tracker: LatencyTracker,
    /// EWMA for error rate
    error_rate: Ewma,
    /// EWMA for throughput (requests per second)
    throughput: Ewma,
    /// Total requests processed
    total_requests: u64,
    /// Failed requests
    failed_requests: u64,
}

impl LoadPredictor {
    /// Create a new load predictor
    ///
    /// # Arguments
    /// * `latency_window` - Number of samples to track for P99 calculation
    /// * `error_alpha` - Smoothing factor for error rate (0.0 to 1.0)
    /// * `throughput_alpha` - Smoothing factor for throughput (0.0 to 1.0)
    pub fn new(latency_window: usize, error_alpha: f64, throughput_alpha: f64) -> Self {
        Self {
            latency_tracker: LatencyTracker::new(latency_window),
            error_rate: Ewma::new(error_alpha),
            throughput: Ewma::new(throughput_alpha),
            total_requests: 0,
            failed_requests: 0,
        }
    }

    /// Create a default load predictor with sensible defaults
    pub fn default() -> Self {
        Self::new(
            100,  // Track last 100 samples for P99
            0.3,  // Medium smoothing for error rate
            0.2,  // Slower smoothing for throughput (more stable)
        )
    }

    /// Record a request completion
    ///
    /// # Arguments
    /// * `latency` - Request latency
    /// * `success` - Whether the request succeeded
    pub fn record_request(&mut self, latency: Duration, success: bool) {
        self.latency_tracker.record(latency);
        self.total_requests += 1;

        if !success {
            self.failed_requests += 1;
        }

        // Update error rate
        let current_error_rate = if self.total_requests > 0 {
            self.failed_requests as f64 / self.total_requests as f64
        } else {
            0.0
        };
        self.error_rate.update(current_error_rate);
    }

    /// Update throughput measurement
    ///
    /// # Arguments
    /// * `requests_per_second` - Current throughput measurement
    pub fn update_throughput(&mut self, requests_per_second: f64) {
        self.throughput.update(requests_per_second);
    }

    /// Get current P99 latency
    pub fn p99_latency(&self) -> Duration {
        self.latency_tracker.p99()
    }

    /// Get current smoothed error rate
    pub fn error_rate(&self) -> f64 {
        self.error_rate.value()
    }

    /// Get current smoothed throughput
    pub fn throughput(&self) -> f64 {
        self.throughput.value()
    }

    /// Get total requests processed
    pub fn total_requests(&self) -> u64 {
        self.total_requests
    }

    /// Get failed requests
    pub fn failed_requests(&self) -> u64 {
        self.failed_requests
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        self.latency_tracker.reset();
        self.error_rate.reset();
        self.throughput.reset();
        self.total_requests = 0;
        self.failed_requests = 0;
    }

    /// Check if the system is experiencing high load
    ///
    /// # Arguments
    /// * `latency_threshold` - P99 latency threshold
    /// * `error_threshold` - Error rate threshold (0.0 to 1.0)
    pub fn is_high_load(&self, latency_threshold: Duration, error_threshold: f64) -> bool {
        self.p99_latency() > latency_threshold || self.error_rate() > error_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewma_basic() {
        let mut ewma = Ewma::new(0.3);

        ewma.update(10.0);
        assert_eq!(ewma.value(), 10.0); // First value

        ewma.update(20.0);
        // 0.3 * 20.0 + 0.7 * 10.0 = 6.0 + 7.0 = 13.0
        assert!((ewma.value() - 13.0).abs() < 0.001);

        assert_eq!(ewma.count(), 2);
    }

    #[test]
    fn test_ewma_clamping() {
        let ewma_low = Ewma::new(-0.5);
        assert_eq!(ewma_low.alpha, 0.0);

        let ewma_high = Ewma::new(1.5);
        assert_eq!(ewma_high.alpha, 1.0);
    }

    #[test]
    fn test_latency_tracker_p99() {
        let mut tracker = LatencyTracker::new(10);

        // Add samples
        for i in 1..=10 {
            tracker.record(Duration::from_millis(i * 10));
        }

        // P99 should be close to the 99th percentile
        let p99 = tracker.p99();
        assert!(p99 >= Duration::from_millis(90));
        assert!(p99 <= Duration::from_millis(100));
    }

    #[test]
    fn test_load_predictor() {
        let mut predictor = LoadPredictor::default();

        // Record some successful requests
        predictor.record_request(Duration::from_millis(50), true);
        predictor.record_request(Duration::from_millis(60), true);
        predictor.record_request(Duration::from_millis(55), true);

        assert_eq!(predictor.total_requests(), 3);
        assert_eq!(predictor.failed_requests(), 0);
        assert!(predictor.error_rate() < 0.01);

        // Record a failure
        predictor.record_request(Duration::from_millis(100), false);
        assert_eq!(predictor.failed_requests(), 1);
        assert!(predictor.error_rate() > 0.0);
    }

    #[test]
    fn test_load_predictor_high_load() {
        let mut predictor = LoadPredictor::default();

        // Record high latency requests
        for _ in 0..10 {
            predictor.record_request(Duration::from_millis(500), true);
        }

        assert!(predictor.is_high_load(
            Duration::from_millis(200),
            0.1
        ));

        // Record high error rate
        let mut predictor2 = LoadPredictor::default();
        for _ in 0..10 {
            predictor2.record_request(Duration::from_millis(50), false);
        }

        assert!(predictor2.is_high_load(
            Duration::from_millis(1000),
            0.05
        ));
    }
}
