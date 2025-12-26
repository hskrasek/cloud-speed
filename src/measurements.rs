use crate::stats::{median_f64, percentile_f64};
use std::collections::VecDeque;
use std::time::Duration;

/// Direction of network traffic for loaded latency measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyDirection {
    /// Latency measured during download tests
    Download,
    /// Latency measured during upload tests
    Upload,
}

/// A single loaded latency measurement with associated metadata.
#[derive(Debug, Clone)]
pub struct LoadedLatencyMeasurement {
    /// The latency value in milliseconds
    pub latency_ms: f64,
}

/// Collector for loaded latency measurements during bandwidth tests.
///
/// This struct maintains separate collections for download and upload
/// directions, enforcing a maximum capacity of 20 data points per direction.
/// When capacity is exceeded, older measurements are evicted in FIFO order.
///
/// # Requirements
/// - Maintains separate collections for download and upload directions
/// - Enforces maximum 20 data points per direction
/// - Implements FIFO eviction when capacity exceeded
/// - Filters out measurements taken during requests < 250ms
///
/// # Example
/// ```
/// let mut collector = LoadedLatencyCollector::new();
///
/// // Add a measurement during a download test
/// collector.add(LatencyDirection::Download, 15.5, 300.0);
///
/// // Get all download latencies
/// let download_latencies = collector.get_latencies(LatencyDirection::Download);
/// ```
#[derive(Debug, Clone)]
pub struct LoadedLatencyCollector {
    /// Download direction latency measurements (FIFO queue)
    download_measurements: VecDeque<LoadedLatencyMeasurement>,
    /// Upload direction latency measurements (FIFO queue)
    upload_measurements: VecDeque<LoadedLatencyMeasurement>,
    /// Maximum capacity per direction
    max_capacity: usize,
    /// Minimum request duration to include a latency measurement (in ms)
    min_request_duration_ms: f64,
}

impl Default for LoadedLatencyCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadedLatencyCollector {
    /// Default maximum capacity per direction (20 data points).
    pub const DEFAULT_MAX_CAPACITY: usize = 20;

    /// Default minimum request duration to include latency (250ms).
    pub const DEFAULT_MIN_REQUEST_DURATION_MS: f64 = 250.0;

    /// Create a new LoadedLatencyCollector with default settings.
    ///
    /// Uses a maximum capacity of 20 per direction and minimum
    /// request duration of 250ms.
    pub fn new() -> Self {
        Self {
            download_measurements: VecDeque::with_capacity(
                Self::DEFAULT_MAX_CAPACITY,
            ),
            upload_measurements: VecDeque::with_capacity(
                Self::DEFAULT_MAX_CAPACITY,
            ),
            max_capacity: Self::DEFAULT_MAX_CAPACITY,
            min_request_duration_ms: Self::DEFAULT_MIN_REQUEST_DURATION_MS,
        }
    }

    /// Add a latency measurement for the specified direction.
    ///
    /// The measurement is only added if the request duration meets the
    /// minimum threshold. If the collection is at capacity, the oldest
    /// measurement is evicted (FIFO).
    ///
    /// # Arguments
    /// * `direction` - Whether this is a download or upload measurement
    /// * `latency_ms` - The latency value in milliseconds
    /// * `request_duration_ms` - Duration of the request during measurement
    ///
    /// # Returns
    /// `true` if the measurement was added, `false` if it was filtered out
    pub fn add(
        &mut self,
        direction: LatencyDirection,
        latency_ms: f64,
        request_duration_ms: f64,
    ) -> bool {
        // Filter out measurements during short requests
        if request_duration_ms < self.min_request_duration_ms {
            return false;
        }

        let measurement = LoadedLatencyMeasurement { latency_ms };

        let queue = match direction {
            LatencyDirection::Download => &mut self.download_measurements,
            LatencyDirection::Upload => &mut self.upload_measurements,
        };

        // Evict oldest if at capacity (FIFO)
        if queue.len() >= self.max_capacity {
            queue.pop_front();
        }

        queue.push_back(measurement);
        true
    }

    /// Get all latency values for the specified direction.
    ///
    /// # Arguments
    /// * `direction` - The direction to get latencies for
    ///
    /// # Returns
    /// A vector of latency values in milliseconds
    pub fn get_latencies(&self, direction: LatencyDirection) -> Vec<f64> {
        let queue = match direction {
            LatencyDirection::Download => &self.download_measurements,
            LatencyDirection::Upload => &self.upload_measurements,
        };

        queue.iter().map(|m| m.latency_ms).collect()
    }

    /// Create a new LoadedLatencyCollector with custom settings (for testing).
    #[cfg(test)]
    pub fn with_config(
        max_capacity: usize,
        min_request_duration_ms: f64,
    ) -> Self {
        Self {
            download_measurements: VecDeque::with_capacity(max_capacity),
            upload_measurements: VecDeque::with_capacity(max_capacity),
            max_capacity,
            min_request_duration_ms,
        }
    }

    /// Get the number of measurements for the specified direction.
    #[cfg(test)]
    pub fn len(&self, direction: LatencyDirection) -> usize {
        match direction {
            LatencyDirection::Download => self.download_measurements.len(),
            LatencyDirection::Upload => self.upload_measurements.len(),
        }
    }

    /// Check if the collection is empty for the specified direction.
    #[cfg(test)]
    pub fn is_empty(&self, direction: LatencyDirection) -> bool {
        self.len(direction) == 0
    }

    /// Get the maximum capacity per direction.
    #[cfg(test)]
    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Clear all measurements for the specified direction.
    #[cfg(test)]
    pub fn clear(&mut self, direction: LatencyDirection) {
        match direction {
            LatencyDirection::Download => self.download_measurements.clear(),
            LatencyDirection::Upload => self.upload_measurements.clear(),
        }
    }

    /// Clear all measurements for both directions.
    #[cfg(test)]
    pub fn clear_all(&mut self) {
        self.download_measurements.clear();
        self.upload_measurements.clear();
    }
}

/// Parses the server-timing header to extract server processing time.
///
/// The Cloudflare speed test API returns a `server-timing` header in the format:
/// `cfRequestDuration;dur=X.XX` where X.XX is the server processing time in milliseconds.
///
/// # Arguments
/// * `header_value` - The value of the server-timing header
///
/// # Returns
/// * `Some(Duration)` - The extracted server processing time
/// * `None` - If the header is missing, malformed, or cannot be parsed
///
/// # Examples
/// ```
/// let duration = parse_server_timing("cfRequestDuration;dur=12.34");
/// assert_eq!(duration, Some(Duration::from_secs_f64(0.01234)));
///
/// let invalid = parse_server_timing("invalid header");
/// assert_eq!(invalid, None);
/// ```
pub fn parse_server_timing(header_value: &str) -> Option<Duration> {
    // Expected format: "cfRequestDuration;dur=X.XX"
    // Split by semicolon to get the duration part
    let parts: Vec<&str> = header_value.split(';').collect();

    // Find the part that starts with "dur="
    for part in parts {
        let trimmed = part.trim();
        if let Some(value_str) = trimmed.strip_prefix("dur=") {
            // Parse the milliseconds value
            if let Ok(ms) = value_str.parse::<f64>() {
                // Ensure non-negative duration
                if ms >= 0.0 && ms.is_finite() {
                    return Some(Duration::from_secs_f64(ms / 1000.0));
                }
            }
        }
    }

    None
}

/// Represents a single bandwidth measurement with timing details.
///
/// This struct captures all the timing information needed to calculate
/// and filter bandwidth measurements according to the speed test methodology.
#[derive(Debug, Clone)]
pub struct BandwidthMeasurement {
    /// Number of bytes transferred
    pub bytes: u64,
    /// Calculated bandwidth in bits per second
    pub bandwidth_bps: f64,
    /// Total duration of the transfer in milliseconds
    pub duration_ms: f64,
    /// Server processing time in milliseconds (from server-timing header)
    pub server_time_ms: f64,
    /// Time to first byte in milliseconds
    pub ttfb_ms: f64,
}

/// Calculates bandwidth in bits per second.
///
/// # Arguments
/// * `bytes` - Number of bytes transferred
/// * `duration` - Total duration of the transfer
/// * `server_time` - Server processing time to subtract from duration
///
/// # Returns
/// Bandwidth in bits per second, or 0.0 if duration <= server_time
pub fn calculate_bandwidth_bps(
    bytes: u64,
    duration: Duration,
    server_time: Duration,
) -> f64 {
    let duration_secs = duration.as_secs_f64();
    let server_time_secs = server_time.as_secs_f64();

    // Handle edge case where duration <= server_time
    if duration_secs <= server_time_secs {
        return 0.0;
    }

    let transfer_time_secs = duration_secs - server_time_secs;
    (bytes as f64 * 8.0) / transfer_time_secs
}

/// Converts bandwidth from bits per second to megabits per second.
///
/// # Arguments
/// * `bandwidth_bps` - Bandwidth in bits per second
///
/// # Returns
/// Bandwidth in megabits per second (Mbps)
pub fn calculate_speed_mbps(bandwidth_bps: f64) -> f64 {
    bandwidth_bps / 1_000_000.0
}

pub async fn latency_f64(measurements: &Vec<f64>) -> f64 {
    let mut measurements = measurements.clone();

    median_f64(&mut measurements).unwrap()
}

pub async fn jitter_f64(measurements: &Vec<f64>) -> Option<f64> {
    // Require at least 2 measurements to calculate jitter
    if measurements.len() < 2 {
        return None;
    }

    let jitters: Vec<_> = measurements
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .collect();

    // Calculate mean of absolute differences
    Some(jitters.iter().sum::<f64>() / jitters.len() as f64)
}

/// Aggregates bandwidth measurements by filtering and calculating a percentile.
///
/// Filters out measurements with duration less than the minimum threshold,
/// then calculates the specified percentile of the remaining bandwidth values.
///
/// # Arguments
/// * `measurements` - Slice of bandwidth measurements to aggregate
/// * `percentile` - The percentile to calculate (0.0 to 1.0, e.g., 0.9 for 90th percentile)
/// * `min_duration_ms` - Minimum duration threshold in milliseconds (measurements below this are filtered out)
///
/// # Returns
/// * `Some(bandwidth_bps)` - The percentile bandwidth in bits per second
/// * `None` - If all measurements are filtered out or the slice is empty
///
/// # Example
/// ```
/// let measurements = vec![
///     BandwidthMeasurement { bytes: 100000, bandwidth_bps: 8000000.0, duration_ms: 15.0, server_time_ms: 1.0, ttfb_ms: 5.0 },
///     BandwidthMeasurement { bytes: 100000, bandwidth_bps: 9000000.0, duration_ms: 12.0, server_time_ms: 1.0, ttfb_ms: 4.0 },
/// ];
/// let result = aggregate_bandwidth(&measurements, 0.9, 10.0);
/// ```
pub fn aggregate_bandwidth(
    measurements: &[BandwidthMeasurement],
    percentile: f64,
    min_duration_ms: f64,
) -> Option<f64> {
    // Filter measurements by minimum duration
    let mut filtered_bandwidths: Vec<f64> = measurements
        .iter()
        .filter(|m| m.duration_ms >= min_duration_ms)
        .map(|m| m.bandwidth_bps)
        .collect();

    // Return None if all measurements were filtered out
    if filtered_bandwidths.is_empty() {
        return None;
    }

    // Calculate and return the percentile
    percentile_f64(&mut filtered_bandwidths, percentile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Tests for calculate_bandwidth_bps
    #[test]
    fn test_calculate_bandwidth_bps_basic() {
        // 1000 bytes in 1 second = 8000 bps
        let bytes = 1000;
        let duration = Duration::from_secs(1);
        let server_time = Duration::from_secs(0);
        let result = calculate_bandwidth_bps(bytes, duration, server_time);
        assert!((result - 8000.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_bandwidth_bps_with_server_time() {
        // 1000 bytes, 1 second total, 0.5 second server time = 0.5 second transfer
        // 8000 bits / 0.5 seconds = 16000 bps
        let bytes = 1000;
        let duration = Duration::from_secs(1);
        let server_time = Duration::from_millis(500);
        let result = calculate_bandwidth_bps(bytes, duration, server_time);
        assert!((result - 16000.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_bandwidth_bps_duration_equals_server_time() {
        // Edge case: duration == server_time should return 0.0
        let bytes = 1000;
        let duration = Duration::from_secs(1);
        let server_time = Duration::from_secs(1);
        let result = calculate_bandwidth_bps(bytes, duration, server_time);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_calculate_bandwidth_bps_duration_less_than_server_time() {
        // Edge case: duration < server_time should return 0.0
        let bytes = 1000;
        let duration = Duration::from_millis(500);
        let server_time = Duration::from_secs(1);
        let result = calculate_bandwidth_bps(bytes, duration, server_time);
        assert_eq!(result, 0.0);
    }

    // Tests for calculate_speed_mbps
    #[test]
    fn test_calculate_speed_mbps_basic() {
        // 1,000,000 bps = 1 Mbps
        let result = calculate_speed_mbps(1_000_000.0);
        assert!((result - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_speed_mbps_large_value() {
        // 100,000,000 bps = 100 Mbps
        let result = calculate_speed_mbps(100_000_000.0);
        assert!((result - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_speed_mbps_zero() {
        let result = calculate_speed_mbps(0.0);
        assert_eq!(result, 0.0);
    }

    // Tests for parse_server_timing
    #[test]
    fn test_parse_server_timing_valid() {
        // Standard format: cfRequestDuration;dur=12.34
        let result = parse_server_timing("cfRequestDuration;dur=12.34");
        assert!(result.is_some());
        let duration = result.unwrap();
        // 12.34 ms = 0.01234 seconds
        assert!((duration.as_secs_f64() - 0.01234).abs() < 0.0001);
    }

    #[test]
    fn test_parse_server_timing_integer_value() {
        // Integer milliseconds
        let result = parse_server_timing("cfRequestDuration;dur=100");
        assert!(result.is_some());
        let duration = result.unwrap();
        // 100 ms = 0.1 seconds
        assert!((duration.as_secs_f64() - 0.1).abs() < 0.0001);
    }

    #[test]
    fn test_parse_server_timing_zero() {
        // Zero duration
        let result = parse_server_timing("cfRequestDuration;dur=0");
        assert!(result.is_some());
        let duration = result.unwrap();
        assert_eq!(duration.as_secs_f64(), 0.0);
    }

    #[test]
    fn test_parse_server_timing_small_value() {
        // Very small duration
        let result = parse_server_timing("cfRequestDuration;dur=0.01");
        assert!(result.is_some());
        let duration = result.unwrap();
        // 0.01 ms = 0.00001 seconds
        assert!((duration.as_secs_f64() - 0.00001).abs() < 0.000001);
    }

    #[test]
    fn test_parse_server_timing_with_spaces() {
        // With spaces around dur=
        let result = parse_server_timing("cfRequestDuration; dur=12.34");
        assert!(result.is_some());
        let duration = result.unwrap();
        assert!((duration.as_secs_f64() - 0.01234).abs() < 0.0001);
    }

    #[test]
    fn test_parse_server_timing_empty_string() {
        // Empty string
        let result = parse_server_timing("");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_server_timing_missing_dur() {
        // Missing dur= part
        let result = parse_server_timing("cfRequestDuration");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_server_timing_invalid_value() {
        // Non-numeric value
        let result = parse_server_timing("cfRequestDuration;dur=abc");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_server_timing_negative_value() {
        // Negative duration (should be rejected)
        let result = parse_server_timing("cfRequestDuration;dur=-5.0");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_server_timing_infinity() {
        // Infinite value (should be rejected)
        let result = parse_server_timing("cfRequestDuration;dur=inf");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_server_timing_nan() {
        // NaN value (should be rejected)
        let result = parse_server_timing("cfRequestDuration;dur=NaN");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_server_timing_multiple_parts() {
        // Multiple semicolon-separated parts
        let result =
            parse_server_timing("cfRequestDuration;dur=15.5;other=value");
        assert!(result.is_some());
        let duration = result.unwrap();
        assert!((duration.as_secs_f64() - 0.0155).abs() < 0.0001);
    }

    // Tests for jitter_f64
    #[tokio::test]
    async fn test_jitter_f64_basic() {
        // Measurements: [10.0, 15.0, 12.0, 18.0]
        // Differences: |15-10|=5, |12-15|=3, |18-12|=6
        // Mean: (5 + 3 + 6) / 3 = 4.666...
        let measurements = vec![10.0, 15.0, 12.0, 18.0];
        let result = jitter_f64(&measurements).await.unwrap();
        assert!((result - 14.0 / 3.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_jitter_f64_two_measurements() {
        // Minimum case: 2 measurements
        let measurements = vec![10.0, 15.0];
        let result = jitter_f64(&measurements).await.unwrap();
        assert!((result - 5.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_jitter_f64_single_measurement() {
        // Should return None for single measurement
        let measurements = vec![10.0];
        let result = jitter_f64(&measurements).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_jitter_f64_empty() {
        // Should return None for empty measurements
        let measurements: Vec<f64> = vec![];
        let result = jitter_f64(&measurements).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_jitter_f64_constant_values() {
        // All same values = 0 jitter
        let measurements = vec![10.0, 10.0, 10.0, 10.0];
        let result = jitter_f64(&measurements).await.unwrap();
        assert_eq!(result, 0.0);
    }

    #[tokio::test]
    async fn test_jitter_f64_always_non_negative() {
        // Jitter should always be non-negative regardless of order
        let measurements = vec![20.0, 10.0, 30.0, 5.0];
        let result = jitter_f64(&measurements).await.unwrap();
        assert!(result >= 0.0);
    }

    // Tests for BandwidthMeasurement and aggregate_bandwidth
    #[test]
    fn test_aggregate_bandwidth_empty() {
        let measurements: Vec<BandwidthMeasurement> = vec![];
        assert_eq!(aggregate_bandwidth(&measurements, 0.9, 10.0), None);
    }

    #[test]
    fn test_aggregate_bandwidth_all_filtered() {
        let measurements = vec![
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 8_000_000.0,
                duration_ms: 5.0, // Below threshold
                server_time_ms: 1.0,
                ttfb_ms: 2.0,
            },
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 9_000_000.0,
                duration_ms: 8.0, // Below threshold
                server_time_ms: 1.0,
                ttfb_ms: 3.0,
            },
        ];
        assert_eq!(aggregate_bandwidth(&measurements, 0.9, 10.0), None);
    }

    #[test]
    fn test_aggregate_bandwidth_some_filtered() {
        let measurements = vec![
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 8_000_000.0,
                duration_ms: 5.0, // Below threshold - filtered out
                server_time_ms: 1.0,
                ttfb_ms: 2.0,
            },
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 10_000_000.0,
                duration_ms: 15.0, // Above threshold - included
                server_time_ms: 1.0,
                ttfb_ms: 3.0,
            },
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 12_000_000.0,
                duration_ms: 20.0, // Above threshold - included
                server_time_ms: 1.0,
                ttfb_ms: 4.0,
            },
        ];
        // Only 10_000_000 and 12_000_000 are included
        // 90th percentile of [10_000_000, 12_000_000] = 10_000_000 + 0.9 * (12_000_000 - 10_000_000) = 11_800_000
        let result = aggregate_bandwidth(&measurements, 0.9, 10.0).unwrap();
        assert!((result - 11_800_000.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregate_bandwidth_none_filtered() {
        let measurements = vec![
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 8_000_000.0,
                duration_ms: 15.0,
                server_time_ms: 1.0,
                ttfb_ms: 2.0,
            },
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 10_000_000.0,
                duration_ms: 12.0,
                server_time_ms: 1.0,
                ttfb_ms: 3.0,
            },
            BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps: 12_000_000.0,
                duration_ms: 20.0,
                server_time_ms: 1.0,
                ttfb_ms: 4.0,
            },
        ];
        // All measurements included: [8_000_000, 10_000_000, 12_000_000]
        // 50th percentile (median) = 10_000_000
        let result = aggregate_bandwidth(&measurements, 0.5, 10.0).unwrap();
        assert!((result - 10_000_000.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregate_bandwidth_exact_threshold() {
        let measurements = vec![BandwidthMeasurement {
            bytes: 100000,
            bandwidth_bps: 8_000_000.0,
            duration_ms: 10.0, // Exactly at threshold - should be included
            server_time_ms: 1.0,
            ttfb_ms: 2.0,
        }];
        let result = aggregate_bandwidth(&measurements, 0.5, 10.0).unwrap();
        assert!((result - 8_000_000.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregate_bandwidth_single_measurement() {
        let measurements = vec![BandwidthMeasurement {
            bytes: 100000,
            bandwidth_bps: 8_000_000.0,
            duration_ms: 15.0,
            server_time_ms: 1.0,
            ttfb_ms: 2.0,
        }];
        let result = aggregate_bandwidth(&measurements, 0.9, 10.0).unwrap();
        assert!((result - 8_000_000.0).abs() < 0.001);
    }

    // Property-based tests for jitter_f64
    // Feature: cloudflare-speedtest-parity, Property 2: Jitter Calculation Correctness
    // Validates: Requirements 3.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: Jitter SHALL be calculated as the mean of absolute differences
        /// between consecutive measurements: jitter = sum(|m[i+1] - m[i]|) / (n-1)
        #[test]
        fn jitter_formula_correctness(
            measurements in prop::collection::vec(
                // Use realistic latency values (0.1ms to 1000ms)
                0.1f64..1000.0f64,
                2..100  // At least 2 measurements required
            )
        ) {
            // Calculate expected jitter manually
            let expected_jitter: f64 = measurements
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs())
                .sum::<f64>() / (measurements.len() - 1) as f64;

            // Use tokio runtime to call async function
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(jitter_f64(&measurements));

            prop_assert!(result.is_some());
            let jitter = result.unwrap();

            // Allow small floating-point tolerance
            let tolerance = expected_jitter.abs() * 1e-10 + 1e-10;
            prop_assert!(
                (jitter - expected_jitter).abs() <= tolerance,
                "Jitter calculation mismatch: got {}, expected {} for measurements {:?}",
                jitter, expected_jitter, measurements
            );
        }

        /// Property: Jitter SHALL always be non-negative
        #[test]
        fn jitter_always_non_negative(
            measurements in prop::collection::vec(
                // Use any finite f64 values
                prop::num::f64::NORMAL | prop::num::f64::POSITIVE | prop::num::f64::NEGATIVE,
                2..100
            ).prop_filter("no NaN or infinite values", |v| v.iter().all(|x| x.is_finite()))
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(jitter_f64(&measurements));

            prop_assert!(result.is_some());
            let jitter = result.unwrap();
            prop_assert!(
                jitter >= 0.0,
                "Jitter should always be non-negative, but got {} for measurements {:?}",
                jitter, measurements
            );
        }

        /// Property: Jitter of constant values SHALL be zero
        #[test]
        fn jitter_constant_values_is_zero(
            value in prop::num::f64::NORMAL.prop_filter("finite", |x| x.is_finite()),
            len in 2usize..50
        ) {
            let measurements: Vec<f64> = vec![value; len];

            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(jitter_f64(&measurements));

            prop_assert!(result.is_some());
            let jitter = result.unwrap();
            prop_assert!(
                jitter.abs() < 1e-10,
                "Jitter of constant values should be 0, but got {} for {} copies of {}",
                jitter, len, value
            );
        }

        /// Property: Jitter SHALL return None for fewer than 2 measurements
        #[test]
        fn jitter_requires_minimum_two_measurements(
            value in prop::num::f64::NORMAL.prop_filter("finite", |x| x.is_finite())
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();

            // Empty vector
            let empty: Vec<f64> = vec![];
            let result_empty = rt.block_on(jitter_f64(&empty));
            prop_assert!(result_empty.is_none(), "Jitter of empty vec should be None");

            // Single element
            let single = vec![value];
            let result_single = rt.block_on(jitter_f64(&single));
            prop_assert!(result_single.is_none(), "Jitter of single element should be None");
        }

        /// Property: Jitter is independent of measurement order direction
        /// (reversing measurements should give the same jitter)
        #[test]
        fn jitter_independent_of_direction(
            measurements in prop::collection::vec(
                0.1f64..1000.0f64,
                2..50
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();

            let result_forward = rt.block_on(jitter_f64(&measurements));

            let mut reversed = measurements.clone();
            reversed.reverse();
            let result_reversed = rt.block_on(jitter_f64(&reversed));

            prop_assert!(result_forward.is_some());
            prop_assert!(result_reversed.is_some());

            let tolerance = result_forward.unwrap().abs() * 1e-10 + 1e-10;
            prop_assert!(
                (result_forward.unwrap() - result_reversed.unwrap()).abs() <= tolerance,
                "Jitter should be same for forward ({}) and reversed ({}) measurements",
                result_forward.unwrap(), result_reversed.unwrap()
            );
        }
    }

    // Property-based tests for calculate_bandwidth_bps
    // Feature: cloudflare-speedtest-parity, Property 3: Bandwidth Calculation Correctness
    // Validates: Requirements 4.2, 2.6, 5.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: bandwidth_bps = (bytes * 8) / (duration_seconds - server_time_seconds)
        /// The calculation SHALL exclude server processing time from the duration.
        #[test]
        fn bandwidth_calculation_formula_correctness(
            bytes in 1u64..1_000_000_000u64,  // 1 byte to 1GB
            duration_ms in 1u64..60_000u64,    // 1ms to 60 seconds
            server_time_ratio in 0.0f64..0.99f64  // Server time as ratio of duration (0-99%)
        ) {
            let duration = Duration::from_millis(duration_ms);
            let server_time_ms = (duration_ms as f64 * server_time_ratio) as u64;
            let server_time = Duration::from_millis(server_time_ms);

            let result = calculate_bandwidth_bps(bytes, duration, server_time);

            // Calculate expected value manually
            let duration_secs = duration.as_secs_f64();
            let server_time_secs = server_time.as_secs_f64();
            let transfer_time_secs = duration_secs - server_time_secs;
            let expected = (bytes as f64 * 8.0) / transfer_time_secs;

            // Allow small floating-point tolerance
            let tolerance = expected * 1e-10;
            prop_assert!(
                (result - expected).abs() <= tolerance,
                "Bandwidth calculation mismatch: got {}, expected {} (bytes={}, duration={:?}, server_time={:?})",
                result, expected, bytes, duration, server_time
            );
        }

        /// Property: bandwidth_bps SHALL be positive when duration > server_time
        #[test]
        fn bandwidth_positive_when_duration_exceeds_server_time(
            bytes in 1u64..1_000_000_000u64,  // At least 1 byte
            duration_ms in 2u64..60_000u64,    // At least 2ms
            server_time_offset in 1u64..60_000u64  // Offset to ensure duration > server_time
        ) {
            // Ensure duration > server_time by making server_time at most duration - 1
            let server_time_ms = duration_ms.saturating_sub(server_time_offset).min(duration_ms - 1);
            let duration = Duration::from_millis(duration_ms);
            let server_time = Duration::from_millis(server_time_ms);

            let result = calculate_bandwidth_bps(bytes, duration, server_time);

            prop_assert!(
                result > 0.0,
                "Bandwidth should be positive when duration ({:?}) > server_time ({:?}), but got {}",
                duration, server_time, result
            );
        }

        /// Property: bandwidth_bps SHALL be 0.0 when duration <= server_time
        #[test]
        fn bandwidth_zero_when_duration_not_exceeds_server_time(
            bytes in 1u64..1_000_000_000u64,
            base_ms in 1u64..60_000u64,
            extra_ms in 0u64..1000u64  // Extra time to add to server_time
        ) {
            // Make server_time >= duration
            let duration = Duration::from_millis(base_ms);
            let server_time = Duration::from_millis(base_ms + extra_ms);

            let result = calculate_bandwidth_bps(bytes, duration, server_time);

            prop_assert_eq!(
                result, 0.0,
                "Bandwidth should be 0.0 when duration ({:?}) <= server_time ({:?}), but got {}",
                duration, server_time, result
            );
        }

        /// Property: Bandwidth scales linearly with bytes (doubling bytes doubles bandwidth)
        #[test]
        fn bandwidth_scales_linearly_with_bytes(
            bytes in 1u64..500_000_000u64,  // Keep smaller to avoid overflow when doubling
            duration_ms in 10u64..60_000u64,
            server_time_ratio in 0.0f64..0.9f64
        ) {
            let duration = Duration::from_millis(duration_ms);
            let server_time_ms = (duration_ms as f64 * server_time_ratio) as u64;
            let server_time = Duration::from_millis(server_time_ms);

            let result1 = calculate_bandwidth_bps(bytes, duration, server_time);
            let result2 = calculate_bandwidth_bps(bytes * 2, duration, server_time);

            // Doubling bytes should double bandwidth
            let tolerance = result1 * 1e-10;
            prop_assert!(
                (result2 - result1 * 2.0).abs() <= tolerance,
                "Doubling bytes should double bandwidth: {} * 2 = {}, but got {}",
                result1, result1 * 2.0, result2
            );
        }

        /// Property: Bandwidth is inversely proportional to transfer time
        /// (halving transfer time doubles bandwidth)
        #[test]
        fn bandwidth_inversely_proportional_to_transfer_time(
            bytes in 1u64..1_000_000_000u64,
            transfer_time_ms in 10u64..30_000u64  // Keep reasonable to allow doubling
        ) {
            // Use zero server time for simplicity
            let server_time = Duration::from_millis(0);

            let duration1 = Duration::from_millis(transfer_time_ms);
            let duration2 = Duration::from_millis(transfer_time_ms * 2);

            let result1 = calculate_bandwidth_bps(bytes, duration1, server_time);
            let result2 = calculate_bandwidth_bps(bytes, duration2, server_time);

            // Doubling duration should halve bandwidth
            let tolerance = result1 * 1e-10;
            prop_assert!(
                (result2 - result1 / 2.0).abs() <= tolerance,
                "Doubling duration should halve bandwidth: {} / 2 = {}, but got {}",
                result1, result1 / 2.0, result2
            );
        }
    }

    // Property-based tests for aggregate_bandwidth minimum duration filtering
    // Feature: cloudflare-speedtest-parity, Property 5: Minimum Duration Filtering
    // Validates: Requirements 4.4, 5.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: Only measurements with duration >= min_duration_ms SHALL be included
        /// in bandwidth aggregation. Measurements with duration < min_duration_ms SHALL
        /// be excluded from the final calculation.
        #[test]
        fn minimum_duration_filtering_excludes_short_measurements(
            // Generate measurements with varying durations
            measurements in prop::collection::vec(
                (
                    1u64..1_000_000u64,      // bytes
                    1_000_000.0f64..100_000_000.0f64,  // bandwidth_bps
                    0.1f64..100.0f64,        // duration_ms (some below, some above threshold)
                    0.0f64..5.0f64,          // server_time_ms
                    0.1f64..10.0f64,         // ttfb_ms
                ),
                1..50
            ),
            min_duration_ms in 5.0f64..20.0f64,  // Variable threshold
            percentile in 0.1f64..0.99f64,
        ) {
            let measurements: Vec<BandwidthMeasurement> = measurements
                .into_iter()
                .map(|(bytes, bandwidth_bps, duration_ms, server_time_ms, ttfb_ms)| {
                    BandwidthMeasurement {
                        bytes,
                        bandwidth_bps,
                        duration_ms,
                        server_time_ms,
                        ttfb_ms,
                    }
                })
                .collect();

            // Calculate expected filtered bandwidths manually
            let expected_filtered: Vec<f64> = measurements
                .iter()
                .filter(|m| m.duration_ms >= min_duration_ms)
                .map(|m| m.bandwidth_bps)
                .collect();

            let result = aggregate_bandwidth(&measurements, percentile, min_duration_ms);

            if expected_filtered.is_empty() {
                // If all measurements are filtered out, result should be None
                prop_assert!(
                    result.is_none(),
                    "Result should be None when all measurements are filtered out"
                );
            } else {
                // Result should be Some and based only on filtered measurements
                prop_assert!(
                    result.is_some(),
                    "Result should be Some when there are valid measurements"
                );

                let result_val = result.unwrap();

                // The result should be within the range of filtered bandwidths
                let min_bw = expected_filtered.iter().cloned().min_by(|a, b| a.total_cmp(b)).unwrap();
                let max_bw = expected_filtered.iter().cloned().max_by(|a, b| a.total_cmp(b)).unwrap();

                prop_assert!(
                    result_val >= min_bw && result_val <= max_bw,
                    "Result {} should be within filtered bandwidth range [{}, {}]",
                    result_val, min_bw, max_bw
                );
            }
        }

        /// Property: Measurements with duration < min_duration_ms SHALL NOT affect the result.
        /// Adding short-duration measurements should not change the aggregated bandwidth.
        #[test]
        fn short_duration_measurements_do_not_affect_result(
            // Generate valid measurements (above threshold)
            valid_measurements in prop::collection::vec(
                (
                    1u64..1_000_000u64,      // bytes
                    1_000_000.0f64..100_000_000.0f64,  // bandwidth_bps
                    15.0f64..100.0f64,       // duration_ms (above 10ms threshold)
                    0.0f64..5.0f64,          // server_time_ms
                    0.1f64..10.0f64,         // ttfb_ms
                ),
                1..20
            ),
            // Generate invalid measurements (below threshold)
            invalid_measurements in prop::collection::vec(
                (
                    1u64..1_000_000u64,      // bytes
                    1_000_000.0f64..100_000_000.0f64,  // bandwidth_bps (different values)
                    0.1f64..9.9f64,          // duration_ms (below 10ms threshold)
                    0.0f64..5.0f64,          // server_time_ms
                    0.1f64..10.0f64,         // ttfb_ms
                ),
                0..20
            ),
            percentile in 0.1f64..0.99f64,
        ) {
            let min_duration_ms = 10.0;

            let valid: Vec<BandwidthMeasurement> = valid_measurements
                .into_iter()
                .map(|(bytes, bandwidth_bps, duration_ms, server_time_ms, ttfb_ms)| {
                    BandwidthMeasurement {
                        bytes,
                        bandwidth_bps,
                        duration_ms,
                        server_time_ms,
                        ttfb_ms,
                    }
                })
                .collect();

            let invalid: Vec<BandwidthMeasurement> = invalid_measurements
                .into_iter()
                .map(|(bytes, bandwidth_bps, duration_ms, server_time_ms, ttfb_ms)| {
                    BandwidthMeasurement {
                        bytes,
                        bandwidth_bps,
                        duration_ms,
                        server_time_ms,
                        ttfb_ms,
                    }
                })
                .collect();

            // Calculate result with only valid measurements
            let result_valid_only = aggregate_bandwidth(&valid, percentile, min_duration_ms);

            // Combine valid and invalid measurements
            let mut combined = valid.clone();
            combined.extend(invalid);

            // Calculate result with combined measurements
            let result_combined = aggregate_bandwidth(&combined, percentile, min_duration_ms);

            // Both results should be equal (invalid measurements should not affect result)
            match (result_valid_only, result_combined) {
                (Some(v1), Some(v2)) => {
                    let tolerance = v1.abs() * 1e-10 + 1e-10;
                    prop_assert!(
                        (v1 - v2).abs() <= tolerance,
                        "Adding short-duration measurements should not change result: {} vs {}",
                        v1, v2
                    );
                }
                (None, None) => {
                    // Both None is fine (no valid measurements)
                }
                _ => {
                    prop_assert!(
                        false,
                        "Results should match: valid_only={:?}, combined={:?}",
                        result_valid_only, result_combined
                    );
                }
            }
        }

        /// Property: Measurements with duration exactly at the threshold (>= min_duration_ms)
        /// SHALL be included in the aggregation.
        #[test]
        fn exact_threshold_measurements_are_included(
            bandwidth_bps in 1_000_000.0f64..100_000_000.0f64,
            min_duration_ms in 5.0f64..20.0f64,
        ) {
            // Create a single measurement exactly at the threshold
            let measurement = BandwidthMeasurement {
                bytes: 100000,
                bandwidth_bps,
                duration_ms: min_duration_ms,  // Exactly at threshold
                server_time_ms: 1.0,
                ttfb_ms: 2.0,
            };

            let result = aggregate_bandwidth(&[measurement], 0.5, min_duration_ms);

            prop_assert!(
                result.is_some(),
                "Measurement exactly at threshold ({} ms) should be included",
                min_duration_ms
            );
            prop_assert!(
                (result.unwrap() - bandwidth_bps).abs() < 0.001,
                "Result should equal the single measurement's bandwidth"
            );
        }

        /// Property: When all measurements are below the threshold, result SHALL be None.
        #[test]
        fn all_below_threshold_returns_none(
            measurements in prop::collection::vec(
                (
                    1u64..1_000_000u64,      // bytes
                    1_000_000.0f64..100_000_000.0f64,  // bandwidth_bps
                    0.1f64..9.9f64,          // duration_ms (all below 10ms)
                    0.0f64..5.0f64,          // server_time_ms
                    0.1f64..10.0f64,         // ttfb_ms
                ),
                1..20
            ),
        ) {
            let min_duration_ms = 10.0;

            let measurements: Vec<BandwidthMeasurement> = measurements
                .into_iter()
                .map(|(bytes, bandwidth_bps, duration_ms, server_time_ms, ttfb_ms)| {
                    BandwidthMeasurement {
                        bytes,
                        bandwidth_bps,
                        duration_ms,
                        server_time_ms,
                        ttfb_ms,
                    }
                })
                .collect();

            let result = aggregate_bandwidth(&measurements, 0.9, min_duration_ms);

            prop_assert!(
                result.is_none(),
                "Result should be None when all measurements ({}) are below threshold ({}ms)",
                measurements.len(), min_duration_ms
            );
        }
    }

    // Property-based tests for parse_server_timing
    // Feature: cloudflare-speedtest-parity, Property 11: Server-Timing Header Parsing
    // Validates: Requirements 12.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any valid server-timing header in the format
        /// `cfRequestDuration;dur=X.XX`, the parser SHALL extract the duration
        /// value as f64 milliseconds. Parsing then formatting SHALL produce
        /// an equivalent value (round-trip).
        ///
        /// Note: Duration uses nanosecond precision internally, so we allow
        /// for small floating-point precision loss during the conversion
        /// from milliseconds to Duration and back.
        #[test]
        fn server_timing_round_trip(
            // Generate realistic server processing times (0.01ms to 10000ms)
            ms_value in 0.01f64..10000.0f64
        ) {
            // Format the header value
            let header = format!("cfRequestDuration;dur={}", ms_value);

            // Parse it
            let result = parse_server_timing(&header);

            prop_assert!(
                result.is_some(),
                "Valid header '{}' should parse successfully",
                header
            );

            let duration = result.unwrap();
            let parsed_ms = duration.as_secs_f64() * 1000.0;

            // Allow for floating-point precision loss during Duration conversion.
            // Duration uses nanosecond precision (1e-9 seconds = 1e-6 ms), so we
            // allow tolerance of ~1e-6 ms relative to the value, plus a small
            // absolute tolerance for very small values.
            let tolerance = ms_value.abs() * 1e-6 + 1e-9;
            prop_assert!(
                (parsed_ms - ms_value).abs() <= tolerance,
                "Round-trip failed: input={}, parsed={}, diff={}",
                ms_value, parsed_ms, (parsed_ms - ms_value).abs()
            );
        }

        /// Property: The parsed duration SHALL always be non-negative
        /// for valid inputs.
        #[test]
        fn server_timing_non_negative_duration(
            ms_value in 0.0f64..10000.0f64
        ) {
            let header = format!("cfRequestDuration;dur={}", ms_value);
            let result = parse_server_timing(&header);

            prop_assert!(result.is_some());
            let duration = result.unwrap();

            prop_assert!(
                duration.as_secs_f64() >= 0.0,
                "Duration should be non-negative, got {:?}",
                duration
            );
        }

        /// Property: Invalid headers SHALL return None, not panic.
        /// This tests various malformed inputs.
        #[test]
        fn server_timing_invalid_returns_none(
            // Generate random strings that don't match the expected format
            random_str in "[a-zA-Z0-9;=.]{0,50}"
        ) {
            // Skip strings that accidentally match the valid format
            if random_str.contains("dur=") {
                // Check if it has a valid number after dur=
                if let Some(idx) = random_str.find("dur=") {
                    let after_dur = &random_str[idx + 4..];
                    let num_str: String = after_dur
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if num_str.parse::<f64>().is_ok() && !num_str.is_empty() {
                        // This might be a valid format, skip it
                        return Ok(());
                    }
                }
            }

            // The function should not panic on any input
            let result = parse_server_timing(&random_str);

            // For truly invalid inputs, result should be None
            // (we already filtered out potentially valid ones above)
            prop_assert!(
                result.is_none(),
                "Invalid header '{}' should return None",
                random_str
            );
        }

        /// Property: Negative duration values SHALL be rejected (return None).
        #[test]
        fn server_timing_rejects_negative(
            ms_value in -10000.0f64..-0.01f64
        ) {
            let header = format!("cfRequestDuration;dur={}", ms_value);
            let result = parse_server_timing(&header);

            prop_assert!(
                result.is_none(),
                "Negative duration '{}' should be rejected",
                header
            );
        }

        /// Property: The parser SHALL handle various valid formats with
        /// different precision levels.
        #[test]
        fn server_timing_various_precisions(
            // Integer part
            int_part in 0u32..1000u32,
            // Decimal places (0-6 digits)
            decimal_digits in 0usize..7usize
        ) {
            // Generate a number with specific decimal precision
            let divisor = 10f64.powi(decimal_digits as i32);
            let ms_value = int_part as f64 + (int_part as f64 % divisor) / divisor;

            let header = format!("cfRequestDuration;dur={:.prec$}", ms_value, prec = decimal_digits);
            let result = parse_server_timing(&header);

            prop_assert!(
                result.is_some(),
                "Header '{}' should parse successfully",
                header
            );
        }

        /// Property: Extra whitespace around the dur= part SHALL be handled.
        #[test]
        fn server_timing_handles_whitespace(
            ms_value in 0.01f64..1000.0f64,
            leading_spaces in 0usize..3usize,
            trailing_spaces in 0usize..3usize
        ) {
            let spaces_before = " ".repeat(leading_spaces);
            let spaces_after = " ".repeat(trailing_spaces);
            let header = format!(
                "cfRequestDuration;{}dur={}{}",
                spaces_before, ms_value, spaces_after
            );

            let result = parse_server_timing(&header);

            prop_assert!(
                result.is_some(),
                "Header with whitespace '{}' should parse successfully",
                header
            );
        }
    }

    // Unit tests for LoadedLatencyCollector
    #[test]
    fn test_loaded_latency_collector_new() {
        let collector = LoadedLatencyCollector::new();
        assert_eq!(
            collector.max_capacity(),
            LoadedLatencyCollector::DEFAULT_MAX_CAPACITY
        );
        assert!(collector.is_empty(LatencyDirection::Download));
        assert!(collector.is_empty(LatencyDirection::Upload));
    }

    #[test]
    fn test_loaded_latency_collector_add_valid() {
        let mut collector = LoadedLatencyCollector::new();

        // Add a measurement with duration above threshold (250ms)
        let added = collector.add(LatencyDirection::Download, 15.5, 300.0);
        assert!(added);
        assert_eq!(collector.len(LatencyDirection::Download), 1);
        assert_eq!(collector.len(LatencyDirection::Upload), 0);

        let latencies = collector.get_latencies(LatencyDirection::Download);
        assert_eq!(latencies.len(), 1);
        assert!((latencies[0] - 15.5).abs() < 0.001);
    }

    #[test]
    fn test_loaded_latency_collector_filter_short_duration() {
        let mut collector = LoadedLatencyCollector::new();

        // Add a measurement with duration below threshold (< 250ms)
        let added = collector.add(LatencyDirection::Download, 15.5, 200.0);
        assert!(!added);
        assert!(collector.is_empty(LatencyDirection::Download));
    }

    #[test]
    fn test_loaded_latency_collector_fifo_eviction() {
        let mut collector = LoadedLatencyCollector::with_config(3, 250.0);

        // Add 3 measurements
        collector.add(LatencyDirection::Download, 10.0, 300.0);
        collector.add(LatencyDirection::Download, 20.0, 300.0);
        collector.add(LatencyDirection::Download, 30.0, 300.0);
        assert_eq!(collector.len(LatencyDirection::Download), 3);

        // Add a 4th measurement - should evict the first one
        collector.add(LatencyDirection::Download, 40.0, 300.0);
        assert_eq!(collector.len(LatencyDirection::Download), 3);

        let latencies = collector.get_latencies(LatencyDirection::Download);
        assert_eq!(latencies, vec![20.0, 30.0, 40.0]);
    }

    #[test]
    fn test_loaded_latency_collector_separate_directions() {
        let mut collector = LoadedLatencyCollector::new();

        collector.add(LatencyDirection::Download, 10.0, 300.0);
        collector.add(LatencyDirection::Upload, 20.0, 300.0);

        assert_eq!(collector.len(LatencyDirection::Download), 1);
        assert_eq!(collector.len(LatencyDirection::Upload), 1);

        let download_latencies =
            collector.get_latencies(LatencyDirection::Download);
        let upload_latencies =
            collector.get_latencies(LatencyDirection::Upload);

        assert_eq!(download_latencies, vec![10.0]);
        assert_eq!(upload_latencies, vec![20.0]);
    }

    #[test]
    fn test_loaded_latency_collector_clear() {
        let mut collector = LoadedLatencyCollector::new();

        collector.add(LatencyDirection::Download, 10.0, 300.0);
        collector.add(LatencyDirection::Upload, 20.0, 300.0);

        collector.clear(LatencyDirection::Download);
        assert!(collector.is_empty(LatencyDirection::Download));
        assert!(!collector.is_empty(LatencyDirection::Upload));

        collector.clear_all();
        assert!(collector.is_empty(LatencyDirection::Download));
        assert!(collector.is_empty(LatencyDirection::Upload));
    }

    // Property-based tests for LoadedLatencyCollector
    // Feature: cloudflare-speedtest-parity, Property 8: Loaded Latency Capacity Constraint
    // Validates: Requirements 6.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: The collection SHALL contain at most max_capacity data points
        /// per direction. WHEN more than max_capacity measurements are available,
        /// only the most recent max_capacity SHALL be kept.
        #[test]
        fn loaded_latency_capacity_constraint(
            // Generate a variable number of measurements (more than capacity)
            num_measurements in 1usize..100usize,
            max_capacity in 1usize..30usize,
            // Generate latency values
            latencies in prop::collection::vec(0.1f64..1000.0f64, 1..100),
        ) {
            let mut collector =
                LoadedLatencyCollector::with_config(max_capacity, 250.0);

            // Add measurements (all with valid request duration)
            let actual_count = num_measurements.min(latencies.len());
            for i in 0..actual_count {
                collector.add(
                    LatencyDirection::Download,
                    latencies[i],
                    300.0, // Above threshold
                );
            }

            // Verify capacity constraint
            let len = collector.len(LatencyDirection::Download);
            prop_assert!(
                len <= max_capacity,
                "Collection length {} exceeds max capacity {}",
                len,
                max_capacity
            );

            // Verify we have the expected number of elements
            let expected_len = actual_count.min(max_capacity);
            prop_assert_eq!(
                len,
                expected_len,
                "Expected {} elements, got {}",
                expected_len,
                len
            );
        }

        /// Property: WHEN capacity is exceeded, older measurements SHALL be
        /// discarded in FIFO order, keeping only the most recent measurements.
        #[test]
        fn loaded_latency_fifo_eviction(
            max_capacity in 2usize..20usize,
            // Generate more measurements than capacity
            extra_measurements in 1usize..30usize,
        ) {
            let mut collector =
                LoadedLatencyCollector::with_config(max_capacity, 250.0);

            let total_measurements = max_capacity + extra_measurements;

            // Add measurements with sequential values for easy verification
            for i in 0..total_measurements {
                collector.add(
                    LatencyDirection::Download,
                    i as f64,
                    300.0, // Above threshold
                );
            }

            // Verify capacity constraint
            let len = collector.len(LatencyDirection::Download);
            prop_assert_eq!(
                len,
                max_capacity,
                "Collection should be at max capacity"
            );

            // Verify FIFO: should have the last max_capacity values
            let latencies =
                collector.get_latencies(LatencyDirection::Download);
            let expected_start = total_measurements - max_capacity;

            for (i, &latency) in latencies.iter().enumerate() {
                let expected = (expected_start + i) as f64;
                prop_assert!(
                    (latency - expected).abs() < 0.001,
                    "Expected latency {} at index {}, got {}",
                    expected,
                    i,
                    latency
                );
            }
        }

        /// Property: Capacity constraint SHALL apply independently to each
        /// direction (download and upload).
        #[test]
        fn loaded_latency_independent_direction_capacity(
            max_capacity in 2usize..15usize,
            download_count in 1usize..50usize,
            upload_count in 1usize..50usize,
        ) {
            let mut collector =
                LoadedLatencyCollector::with_config(max_capacity, 250.0);

            // Add download measurements
            for i in 0..download_count {
                collector.add(
                    LatencyDirection::Download,
                    i as f64,
                    300.0,
                );
            }

            // Add upload measurements
            for i in 0..upload_count {
                collector.add(
                    LatencyDirection::Upload,
                    (i + 1000) as f64, // Different values
                    300.0,
                );
            }

            // Verify each direction respects capacity independently
            let download_len = collector.len(LatencyDirection::Download);
            let upload_len = collector.len(LatencyDirection::Upload);

            prop_assert!(
                download_len <= max_capacity,
                "Download length {} exceeds capacity {}",
                download_len,
                max_capacity
            );
            prop_assert!(
                upload_len <= max_capacity,
                "Upload length {} exceeds capacity {}",
                upload_len,
                max_capacity
            );

            // Verify expected counts
            let expected_download = download_count.min(max_capacity);
            let expected_upload = upload_count.min(max_capacity);

            prop_assert_eq!(download_len, expected_download);
            prop_assert_eq!(upload_len, expected_upload);
        }
    }

    // Property-based tests for LoadedLatencyCollector duration filtering
    // Feature: cloudflare-speedtest-parity, Property 7: Loaded Latency Duration Filtering
    // Validates: Requirements 6.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: Only latency measurements taken during requests with
        /// duration >= min_request_duration_ms SHALL be included.
        /// Latency measurements during shorter requests SHALL be excluded.
        #[test]
        fn loaded_latency_duration_filtering(
            // Generate measurements with varying request durations
            measurements in prop::collection::vec(
                (
                    0.1f64..1000.0f64,  // latency_ms
                    0.0f64..500.0f64,   // request_duration_ms (some below, some above 250ms)
                ),
                1..50
            ),
            min_request_duration_ms in 100.0f64..400.0f64,
        ) {
            let mut collector = LoadedLatencyCollector::with_config(
                100, // Large capacity to avoid eviction affecting test
                min_request_duration_ms,
            );

            // Track which measurements should be included
            let mut expected_latencies: Vec<f64> = Vec::new();

            for (latency_ms, request_duration_ms) in &measurements {
                let added = collector.add(
                    LatencyDirection::Download,
                    *latency_ms,
                    *request_duration_ms,
                );

                // Verify add() returns correct value
                let should_be_added =
                    *request_duration_ms >= min_request_duration_ms;
                prop_assert_eq!(
                    added,
                    should_be_added,
                    "add() returned {} but should have returned {} for request_duration={}ms (threshold={}ms)",
                    added,
                    should_be_added,
                    request_duration_ms,
                    min_request_duration_ms
                );

                if should_be_added {
                    expected_latencies.push(*latency_ms);
                }
            }

            // Verify the collector contains only the expected latencies
            let actual_latencies =
                collector.get_latencies(LatencyDirection::Download);
            prop_assert_eq!(
                actual_latencies.len(),
                expected_latencies.len(),
                "Expected {} latencies, got {}",
                expected_latencies.len(),
                actual_latencies.len()
            );

            // Verify values match
            for (i, (actual, expected)) in actual_latencies
                .iter()
                .zip(expected_latencies.iter())
                .enumerate()
            {
                prop_assert!(
                    (actual - expected).abs() < 0.001,
                    "Latency mismatch at index {}: expected {}, got {}",
                    i,
                    expected,
                    actual
                );
            }
        }

        /// Property: Measurements with request duration exactly at the threshold
        /// (>= min_request_duration_ms) SHALL be included.
        #[test]
        fn loaded_latency_exact_threshold_included(
            latency_ms in 0.1f64..1000.0f64,
            min_request_duration_ms in 100.0f64..400.0f64,
        ) {
            let mut collector = LoadedLatencyCollector::with_config(
                20,
                min_request_duration_ms,
            );

            // Add measurement exactly at threshold
            let added = collector.add(
                LatencyDirection::Download,
                latency_ms,
                min_request_duration_ms, // Exactly at threshold
            );

            prop_assert!(
                added,
                "Measurement at exact threshold ({} ms) should be included",
                min_request_duration_ms
            );

            let latencies =
                collector.get_latencies(LatencyDirection::Download);
            prop_assert_eq!(latencies.len(), 1);
            prop_assert!(
                (latencies[0] - latency_ms).abs() < 0.001,
                "Latency value should match"
            );
        }

        /// Property: Measurements with request duration below threshold
        /// SHALL be excluded (add() returns false, collection unchanged).
        #[test]
        fn loaded_latency_below_threshold_excluded(
            latency_ms in 0.1f64..1000.0f64,
            min_request_duration_ms in 100.0f64..400.0f64,
            // Request duration below threshold
            below_threshold_offset in 0.01f64..99.0f64,
        ) {
            let request_duration_ms =
                (min_request_duration_ms - below_threshold_offset).max(0.0);

            // Skip if we accidentally hit the threshold
            if request_duration_ms >= min_request_duration_ms {
                return Ok(());
            }

            let mut collector = LoadedLatencyCollector::with_config(
                20,
                min_request_duration_ms,
            );

            // Add measurement below threshold
            let added = collector.add(
                LatencyDirection::Download,
                latency_ms,
                request_duration_ms,
            );

            prop_assert!(
                !added,
                "Measurement below threshold ({} ms < {} ms) should be excluded",
                request_duration_ms,
                min_request_duration_ms
            );

            prop_assert!(
                collector.is_empty(LatencyDirection::Download),
                "Collection should be empty after rejected measurement"
            );
        }

        /// Property: Duration filtering SHALL apply independently to each
        /// direction (download and upload).
        #[test]
        fn loaded_latency_duration_filtering_independent_directions(
            download_latency in 0.1f64..1000.0f64,
            upload_latency in 0.1f64..1000.0f64,
            download_duration in 0.0f64..500.0f64,
            upload_duration in 0.0f64..500.0f64,
            min_request_duration_ms in 100.0f64..400.0f64,
        ) {
            let mut collector = LoadedLatencyCollector::with_config(
                20,
                min_request_duration_ms,
            );

            // Add download measurement
            let download_added = collector.add(
                LatencyDirection::Download,
                download_latency,
                download_duration,
            );

            // Add upload measurement
            let upload_added = collector.add(
                LatencyDirection::Upload,
                upload_latency,
                upload_duration,
            );

            // Verify each direction filtered independently
            let download_should_be_added =
                download_duration >= min_request_duration_ms;
            let upload_should_be_added =
                upload_duration >= min_request_duration_ms;

            prop_assert_eq!(download_added, download_should_be_added);
            prop_assert_eq!(upload_added, upload_should_be_added);

            // Verify collection state
            let download_len = collector.len(LatencyDirection::Download);
            let upload_len = collector.len(LatencyDirection::Upload);

            prop_assert_eq!(
                download_len,
                if download_should_be_added { 1 } else { 0 }
            );
            prop_assert_eq!(
                upload_len,
                if upload_should_be_added { 1 } else { 0 }
            );
        }

        /// Property: Short-duration measurements SHALL NOT affect the
        /// collection state (no side effects from rejected measurements).
        #[test]
        fn loaded_latency_short_duration_no_side_effects(
            // Valid measurements
            valid_measurements in prop::collection::vec(
                (0.1f64..1000.0f64, 300.0f64..500.0f64),
                1..10
            ),
            // Invalid measurements (below threshold)
            invalid_measurements in prop::collection::vec(
                (0.1f64..1000.0f64, 0.0f64..249.0f64),
                1..10
            ),
        ) {
            let min_request_duration_ms = 250.0;
            let mut collector = LoadedLatencyCollector::with_config(
                100,
                min_request_duration_ms,
            );

            // Add valid measurements first
            for (latency_ms, request_duration_ms) in &valid_measurements {
                collector.add(
                    LatencyDirection::Download,
                    *latency_ms,
                    *request_duration_ms,
                );
            }

            let state_before =
                collector.get_latencies(LatencyDirection::Download);

            // Try to add invalid measurements
            for (latency_ms, request_duration_ms) in &invalid_measurements {
                let added = collector.add(
                    LatencyDirection::Download,
                    *latency_ms,
                    *request_duration_ms,
                );
                prop_assert!(!added, "Invalid measurement should be rejected");
            }

            let state_after =
                collector.get_latencies(LatencyDirection::Download);

            // State should be unchanged
            prop_assert_eq!(
                state_before.len(),
                state_after.len(),
                "Collection length should be unchanged after rejected measurements"
            );

            for (i, (before, after)) in
                state_before.iter().zip(state_after.iter()).enumerate()
            {
                prop_assert!(
                    (before - after).abs() < 0.001,
                    "Latency at index {} changed from {} to {}",
                    i,
                    before,
                    after
                );
            }
        }
    }
}
