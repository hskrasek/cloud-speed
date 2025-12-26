use crate::cloudflare::tests::download::Download;
use crate::cloudflare::tests::upload::Upload;
use crate::cloudflare::tests::{Test, TestResults};
use crate::measurements::{
    aggregate_bandwidth, calculate_speed_mbps, jitter_f64, latency_f64,
    BandwidthMeasurement, LatencyDirection, LoadedLatencyCollector,
};
use crate::retry::{retry_async, RetryConfig, RetryResult};
use crate::stats::{median_f64, percentile_f64};
use log::{debug, info, warn};
use std::error::Error;
use tokio::sync::mpsc;

/// A data block configuration for bandwidth tests.
///
/// Defines the size and number of measurements for a specific
/// file size in the download or upload test sequence.
#[derive(Debug, Clone)]
pub struct DataBlock {
    /// Size of the data block in bytes
    pub bytes: u64,
    /// Number of measurements to perform at this size
    pub count: usize,
}

impl DataBlock {
    /// Create a new data block configuration.
    pub const fn new(bytes: u64, count: usize) -> Self {
        Self { bytes, count }
    }
}

/// Configuration for the test engine.
///
/// This struct contains all configurable parameters for the speed test,
/// including data block sizes, latency settings, and duration thresholds.
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Data block sizes and counts for download tests.
    /// Default: 100KB(10), 1MB(8), 10MB(6), 25MB(4), 100MB(3)
    pub download_sizes: Vec<DataBlock>,

    /// Data block sizes and counts for upload tests.
    /// Default: 100KB(8), 1MB(6), 10MB(4), 25MB(4), 50MB(3)
    pub upload_sizes: Vec<DataBlock>,

    /// Number of packets for idle latency measurement.
    /// Default: 20
    pub latency_packets: usize,

    /// Minimum interval between loaded latency measurements in ms.
    /// Default: 400ms
    pub loaded_latency_throttle_ms: u64,

    /// Duration threshold to stop testing larger file sizes (in ms).
    /// When a measurement reaches this duration, skip larger sizes.
    /// Default: 1000ms
    pub bandwidth_finish_duration_ms: f64,

    /// Minimum duration for a measurement to be included in
    /// bandwidth calculations (in ms).
    /// Default: 10ms
    pub bandwidth_min_duration_ms: f64,

    /// Minimum request duration to include loaded latency
    /// measurements (in ms).
    /// Default: 250ms
    pub loaded_request_min_duration_ms: f64,

    /// Percentile to use for final bandwidth calculation.
    /// Default: 0.9 (90th percentile)
    pub bandwidth_percentile: f64,

    /// Retry configuration for failed measurements.
    /// Default: 3 retries with exponential backoff
    pub retry_config: RetryConfig,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            // Download sizes per Cloudflare speed test:
            // 100KB: 10 measurements (with 1 initial estimation)
            // 1MB: 8 measurements
            // 10MB: 6 measurements
            // 25MB: 4 measurements
            // 100MB: 3 measurements
            download_sizes: vec![
                DataBlock::new(100_000, 10),    // 100KB
                DataBlock::new(1_000_000, 8),   // 1MB
                DataBlock::new(10_000_000, 6),  // 10MB
                DataBlock::new(25_000_000, 4),  // 25MB
                DataBlock::new(100_000_000, 3), // 100MB
            ],
            // Upload sizes per Cloudflare speed test:
            // 100KB: 8 measurements
            // 1MB: 6 measurements
            // 10MB: 4 measurements
            // 25MB: 4 measurements
            // 50MB: 3 measurements
            upload_sizes: vec![
                DataBlock::new(100_000, 8),    // 100KB
                DataBlock::new(1_000_000, 6),  // 1MB
                DataBlock::new(10_000_000, 4), // 10MB
                DataBlock::new(25_000_000, 4), // 25MB
                DataBlock::new(50_000_000, 3), // 50MB
            ],
            latency_packets: 20,
            loaded_latency_throttle_ms: 400,
            bandwidth_finish_duration_ms: 1000.0,
            bandwidth_min_duration_ms: 10.0,
            loaded_request_min_duration_ms: 250.0,
            bandwidth_percentile: 0.9,
            retry_config: RetryConfig::default(),
        }
    }
}

/// Results from a single bandwidth measurement set (one file size).
#[derive(Debug, Clone)]
pub struct SizeMeasurement {
    /// Size of the data block in bytes
    pub bytes: u64,
    /// Calculated speed in Mbps for this size
    pub speed_mbps: f64,
    /// Number of measurements performed
    pub count: usize,
    /// Individual bandwidth measurements
    pub measurements: Vec<BandwidthMeasurement>,
    /// Whether early termination was triggered after this size
    pub triggered_early_termination: bool,
}

/// Results from latency measurements.
#[derive(Debug, Clone)]
pub struct LatencyResults {
    /// Idle latency (median) in milliseconds
    pub idle_ms: f64,
    /// Idle jitter in milliseconds
    pub idle_jitter_ms: Option<f64>,
    /// Loaded latency during downloads (median) in milliseconds
    pub loaded_down_ms: Option<f64>,
    /// Loaded jitter during downloads in milliseconds
    pub loaded_down_jitter_ms: Option<f64>,
    /// Loaded latency during uploads (median) in milliseconds
    pub loaded_up_ms: Option<f64>,
    /// Loaded jitter during uploads in milliseconds
    pub loaded_up_jitter_ms: Option<f64>,
}

/// Results from bandwidth measurements (download or upload).
#[derive(Debug, Clone)]
pub struct BandwidthResults {
    /// Final speed in Mbps (90th percentile of all measurements)
    pub speed_mbps: f64,
    /// Per-size measurement results
    pub measurements: Vec<SizeMeasurement>,
    /// Whether early termination was applied
    pub early_terminated: bool,
}

/// Complete results from a speed test run.
#[derive(Debug, Clone)]
pub struct SpeedTestOutput {
    /// Latency measurement results
    pub latency: LatencyResults,
    /// Download bandwidth results
    pub download: BandwidthResults,
    /// Upload bandwidth results
    pub upload: BandwidthResults,
}

/// The test engine that orchestrates all network measurements.
///
/// This struct manages the execution of the complete speed test sequence,
/// including latency measurements, download tests, upload tests, and
/// loaded latency collection.
///
/// # Example
/// ```no_run
/// use cloud_speed::cloudflare::tests::engine::{TestEngine, TestConfig};
///
/// #[tokio::main]
/// async fn main() {
///     let engine = TestEngine::new(TestConfig::default());
///     let results = engine.run().await.unwrap();
///     println!("Download: {:.2} Mbps", results.download.speed_mbps);
///     println!("Upload: {:.2} Mbps", results.upload.speed_mbps);
/// }
/// ```
pub struct TestEngine {
    config: TestConfig,
}

impl TestEngine {
    /// Create a new test engine with the given configuration.
    pub fn new(config: TestConfig) -> Self {
        Self { config }
    }

    /// Run the complete speed test sequence.
    ///
    /// Executes measurements in the following order:
    /// 1. Initial latency estimation (1 packet)
    /// 2. Initial download estimation (100KB, 1 request)
    /// 3. Full latency measurement (20 packets)
    /// 4. Download and upload tests (interleaved by similar sizes)
    ///
    /// Download and upload tests are interleaved to provide a more
    /// realistic measurement of connection performance under varying
    /// conditions.
    ///
    /// # Returns
    /// Complete speed test results including latency, download, and upload
    pub async fn run(&self) -> Result<SpeedTestOutput, Box<dyn Error>> {
        info!("Starting speed test sequence");

        // Step 1: Initial latency estimation (1 packet)
        debug!("Running initial latency estimation");
        let _ = self.run_latency(1).await?;

        // Step 2: Initial download estimation (100KB, 1 request)
        debug!("Running initial download estimation");
        let _ = self.run_download_single(100_000).await?;

        // Step 3: Full latency measurement
        debug!(
            "Running full latency measurement ({} packets)",
            self.config.latency_packets
        );
        let idle_latencies =
            self.run_latency(self.config.latency_packets).await?;

        let idle_ms = latency_f64(&idle_latencies).await;
        let idle_jitter_ms = jitter_f64(&idle_latencies).await;

        info!("Idle latency: {:.2} ms, jitter: {:?}", idle_ms, idle_jitter_ms);

        // Step 4: Interleaved download and upload tests with loaded latency
        let mut loaded_latency_collector = LoadedLatencyCollector::new();

        let (download, upload) = self
            .run_interleaved_bandwidth_tests(&mut loaded_latency_collector)
            .await?;

        // Calculate loaded latency results
        let loaded_down_latencies =
            loaded_latency_collector.get_latencies(LatencyDirection::Download);
        let loaded_up_latencies =
            loaded_latency_collector.get_latencies(LatencyDirection::Upload);

        let loaded_down_ms = if !loaded_down_latencies.is_empty() {
            let mut latencies = loaded_down_latencies.clone();
            median_f64(&mut latencies)
        } else {
            None
        };

        let loaded_down_jitter_ms = if loaded_down_latencies.len() >= 2 {
            jitter_f64(&loaded_down_latencies).await
        } else {
            None
        };

        let loaded_up_ms = if !loaded_up_latencies.is_empty() {
            let mut latencies = loaded_up_latencies.clone();
            median_f64(&mut latencies)
        } else {
            None
        };

        let loaded_up_jitter_ms = if loaded_up_latencies.len() >= 2 {
            jitter_f64(&loaded_up_latencies).await
        } else {
            None
        };

        let latency = LatencyResults {
            idle_ms,
            idle_jitter_ms,
            loaded_down_ms,
            loaded_down_jitter_ms,
            loaded_up_ms,
            loaded_up_jitter_ms,
        };

        info!(
            "Speed test complete: download={:.2} Mbps, upload={:.2} Mbps",
            download.speed_mbps, upload.speed_mbps
        );

        Ok(SpeedTestOutput { latency, download, upload })
    }

    /// Run interleaved download and upload bandwidth tests.
    ///
    /// This method interleaves download and upload tests of similar sizes
    /// to provide more realistic measurements. Tests are paired by size
    /// and executed alternately (download then upload for each size).
    ///
    /// Early termination is tracked separately for each direction.
    async fn run_interleaved_bandwidth_tests(
        &self,
        loaded_latency_collector: &mut LoadedLatencyCollector,
    ) -> Result<(BandwidthResults, BandwidthResults), Box<dyn Error>> {
        let mut download_measurements: Vec<BandwidthMeasurement> = Vec::new();
        let mut upload_measurements: Vec<BandwidthMeasurement> = Vec::new();
        let mut download_size_results: Vec<SizeMeasurement> = Vec::new();
        let mut upload_size_results: Vec<SizeMeasurement> = Vec::new();
        let mut download_early_terminated = false;
        let mut upload_early_terminated = false;

        // Get the maximum number of size blocks between download and upload
        let max_blocks = self
            .config
            .download_sizes
            .len()
            .max(self.config.upload_sizes.len());

        for i in 0..max_blocks {
            // Run download test for this size (if available and not terminated)
            if let Some(block) = self.config.download_sizes.get(i) {
                if !download_early_terminated {
                    info!(
                        "Running download test: {} bytes x {} iterations",
                        block.bytes, block.count
                    );

                    let (measurements, triggered) = self
                        .run_bandwidth_block(
                            block,
                            true, // is_download
                            LatencyDirection::Download,
                            loaded_latency_collector,
                        )
                        .await?;

                    let speed_mbps = self.calculate_block_speed(&measurements);
                    info!("Download {}B: {:.2} Mbps", block.bytes, speed_mbps);

                    download_size_results.push(SizeMeasurement {
                        bytes: block.bytes,
                        speed_mbps,
                        count: measurements.len(),
                        measurements: measurements.clone(),
                        triggered_early_termination: triggered,
                    });

                    download_measurements.extend(measurements);

                    if triggered {
                        download_early_terminated = true;
                        info!("Early termination triggered for download at {} bytes",
                              block.bytes);
                    }
                } else {
                    debug!(
                        "Skipping download {}B due to early termination",
                        block.bytes
                    );
                }
            }

            // Run upload test for this size (if available and not terminated)
            if let Some(block) = self.config.upload_sizes.get(i) {
                if !upload_early_terminated {
                    info!(
                        "Running upload test: {} bytes x {} iterations",
                        block.bytes, block.count
                    );

                    let (measurements, triggered) = self
                        .run_bandwidth_block(
                            block,
                            false, // is_download
                            LatencyDirection::Upload,
                            loaded_latency_collector,
                        )
                        .await?;

                    let speed_mbps = self.calculate_block_speed(&measurements);
                    info!("Upload {}B: {:.2} Mbps", block.bytes, speed_mbps);

                    upload_size_results.push(SizeMeasurement {
                        bytes: block.bytes,
                        speed_mbps,
                        count: measurements.len(),
                        measurements: measurements.clone(),
                        triggered_early_termination: triggered,
                    });

                    upload_measurements.extend(measurements);

                    if triggered {
                        upload_early_terminated = true;
                        info!("Early termination triggered for upload at {} bytes",
                              block.bytes);
                    }
                } else {
                    debug!(
                        "Skipping upload {}B due to early termination",
                        block.bytes
                    );
                }
            }
        }

        // Calculate final speeds using 90th percentile of all measurements
        let download_speed_mbps = aggregate_bandwidth(
            &download_measurements,
            self.config.bandwidth_percentile,
            self.config.bandwidth_min_duration_ms,
        )
        .map(calculate_speed_mbps)
        .unwrap_or(0.0);

        let upload_speed_mbps = aggregate_bandwidth(
            &upload_measurements,
            self.config.bandwidth_percentile,
            self.config.bandwidth_min_duration_ms,
        )
        .map(calculate_speed_mbps)
        .unwrap_or(0.0);

        let download = BandwidthResults {
            speed_mbps: download_speed_mbps,
            measurements: download_size_results,
            early_terminated: download_early_terminated,
        };

        let upload = BandwidthResults {
            speed_mbps: upload_speed_mbps,
            measurements: upload_size_results,
            early_terminated: upload_early_terminated,
        };

        Ok((download, upload))
    }

    /// Calculate the speed in Mbps for a block of measurements.
    fn calculate_block_speed(
        &self,
        measurements: &[BandwidthMeasurement],
    ) -> f64 {
        let mut bandwidths: Vec<f64> = measurements
            .iter()
            .filter(|m| m.duration_ms >= self.config.bandwidth_min_duration_ms)
            .map(|m| m.bandwidth_bps)
            .collect();

        if !bandwidths.is_empty() {
            let bps = percentile_f64(
                &mut bandwidths,
                self.config.bandwidth_percentile,
            )
            .unwrap_or(0.0);
            calculate_speed_mbps(bps)
        } else {
            0.0
        }
    }

    /// Run latency measurements.
    ///
    /// # Arguments
    /// * `num_packets` - Number of latency measurements to perform
    ///
    /// # Returns
    /// Vector of latency values in milliseconds
    pub async fn run_latency(
        &self,
        num_packets: usize,
    ) -> Result<Vec<f64>, Box<dyn Error>> {
        let download = Download {};
        let mut latencies = Vec::with_capacity(num_packets);
        let mut failed_count = 0;

        for i in 0..num_packets {
            debug!("Latency measurement {}/{}", i + 1, num_packets);

            let operation_name =
                format!("latency measurement {}/{}", i + 1, num_packets);
            let result = retry_async(
                &self.config.retry_config,
                &operation_name,
                || async {
                    // Use small download (1000 bytes) to measure latency
                    download
                        .run(1000)
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))
                },
            )
            .await;

            match result {
                RetryResult::Success(test_result) => {
                    // Use TCP handshake time as latency measurement
                    let latency_ms =
                        test_result.tcp_duration.as_secs_f64() * 1000.0;
                    latencies.push(latency_ms);
                    debug!("Latency: {:.2} ms", latency_ms);
                }
                RetryResult::Failed { last_error, attempts } => {
                    failed_count += 1;
                    warn!(
                        "Latency measurement {}/{} failed after {} attempts: {}",
                        i + 1, num_packets, attempts, last_error
                    );
                    // Continue with remaining measurements
                }
            }
        }

        if latencies.is_empty() {
            return Err(format!(
                "All {} latency measurements failed",
                num_packets
            )
            .into());
        }

        if failed_count > 0 {
            warn!(
                "{} of {} latency measurements failed, continuing with {} successful",
                failed_count, num_packets, latencies.len()
            );
        }

        Ok(latencies)
    }

    /// Run a single download measurement with retry logic.
    async fn run_download_single(
        &self,
        bytes: u64,
    ) -> Result<TestResults, Box<dyn Error>> {
        let download = Download {};
        let operation_name = format!("download estimation ({}B)", bytes);

        let result = retry_async(
            &self.config.retry_config,
            &operation_name,
            || async {
                download
                    .run(bytes)
                    .await
                    .map_err(|e| std::io::Error::other(e.to_string()))
            },
        )
        .await;

        match result {
            RetryResult::Success(test_result) => Ok(test_result),
            RetryResult::Failed { last_error, attempts } => Err(format!(
                "{} failed after {} attempts: {}",
                operation_name, attempts, last_error
            )
            .into()),
        }
    }

    /// Run a single bandwidth block (one file size, multiple iterations).
    ///
    /// Returns the measurements and whether early termination was triggered.
    /// Individual measurement failures are retried, and if all retries fail,
    /// the measurement is skipped and the test continues with remaining iterations.
    async fn run_bandwidth_block(
        &self,
        block: &DataBlock,
        is_download: bool,
        latency_direction: LatencyDirection,
        loaded_latency_collector: &mut LoadedLatencyCollector,
    ) -> Result<(Vec<BandwidthMeasurement>, bool), Box<dyn Error>> {
        let mut measurements = Vec::with_capacity(block.count);
        let mut triggered_early_termination = false;
        let mut failed_count = 0;

        // Create channel for loaded latency measurements
        let (latency_tx, mut latency_rx) = mpsc::channel::<f64>(100);

        let test_type = if is_download { "download" } else { "upload" };

        for i in 0..block.count {
            debug!(
                "  Iteration {}/{} for {} bytes",
                i + 1,
                block.count,
                block.bytes
            );

            let operation_name = format!(
                "{} {}B iteration {}/{}",
                test_type,
                block.bytes,
                i + 1,
                block.count
            );

            let latency_tx_clone = latency_tx.clone();
            let throttle_ms = self.config.loaded_latency_throttle_ms;
            let min_duration_ms =
                self.config.loaded_request_min_duration_ms as u64;
            let bytes = block.bytes;

            let result = if is_download {
                retry_async(&self.config.retry_config, &operation_name, || {
                    let latency_tx = latency_tx_clone.clone();
                    async move {
                        let download = Download {};
                        download
                            .run_with_loaded_latency(
                                bytes,
                                latency_tx,
                                throttle_ms,
                                min_duration_ms,
                            )
                            .await
                            .map_err(|e| std::io::Error::other(e.to_string()))
                    }
                })
                .await
            } else {
                retry_async(&self.config.retry_config, &operation_name, || {
                    let latency_tx = latency_tx_clone.clone();
                    async move {
                        let upload = Upload::new(bytes);
                        upload
                            .run_with_loaded_latency(
                                latency_tx,
                                throttle_ms,
                                min_duration_ms,
                            )
                            .await
                            .map_err(|e| std::io::Error::other(e.to_string()))
                    }
                })
                .await
            };

            match result {
                RetryResult::Success(test_result) => {
                    let measurement = test_result.to_bandwidth_measurement();
                    let duration_ms = measurement.duration_ms;

                    measurements.push(measurement);

                    // Check for early termination
                    if duration_ms >= self.config.bandwidth_finish_duration_ms
                    {
                        triggered_early_termination = true;
                        debug!(
                            "Duration {:.2}ms >= threshold {:.2}ms, triggering early termination",
                            duration_ms, self.config.bandwidth_finish_duration_ms
                        );
                    }
                }
                RetryResult::Failed { last_error, attempts } => {
                    failed_count += 1;
                    warn!(
                        "{} failed after {} attempts: {}. Continuing with remaining iterations.",
                        operation_name, attempts, last_error
                    );
                    // Continue with remaining iterations
                }
            }
        }

        // Drop the sender to close the channel
        drop(latency_tx);

        // Collect loaded latency measurements from channel
        while let Ok(latency_ms) = latency_rx.try_recv() {
            // Get the duration of the most recent measurement
            let request_duration_ms =
                measurements.last().map(|m| m.duration_ms).unwrap_or(0.0);

            loaded_latency_collector.add(
                latency_direction,
                latency_ms,
                request_duration_ms,
            );
        }

        if failed_count > 0 {
            warn!(
                "{} {}B: {} of {} measurements failed, {} successful",
                test_type,
                block.bytes,
                failed_count,
                block.count,
                measurements.len()
            );
        }

        Ok((measurements, triggered_early_termination))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests for TestConfig
    #[test]
    fn test_config_default() {
        let config = TestConfig::default();
        assert_eq!(config.latency_packets, 20);
        assert_eq!(config.loaded_latency_throttle_ms, 400);
        assert!((config.bandwidth_finish_duration_ms - 1000.0).abs() < 0.001);
        assert!((config.bandwidth_min_duration_ms - 10.0).abs() < 0.001);
        assert!((config.loaded_request_min_duration_ms - 250.0).abs() < 0.001);
        assert!((config.bandwidth_percentile - 0.9).abs() < 0.001);
        assert_eq!(config.download_sizes.len(), 5);
        assert_eq!(config.upload_sizes.len(), 5);
    }

    #[test]
    fn test_data_block_new() {
        let block = DataBlock::new(100_000, 10);
        assert_eq!(block.bytes, 100_000);
        assert_eq!(block.count, 10);
    }

    // Unit tests for calculate_block_speed
    #[test]
    fn test_calculate_block_speed_empty() {
        let engine = TestEngine::new(TestConfig::default());
        let measurements: Vec<BandwidthMeasurement> = vec![];
        let speed = engine.calculate_block_speed(&measurements);
        assert!((speed - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_block_speed_all_filtered() {
        let engine = TestEngine::new(TestConfig::default());
        let measurements = vec![BandwidthMeasurement {
            bytes: 100_000,
            bandwidth_bps: 8_000_000.0,
            duration_ms: 5.0, // Below 10ms threshold
            server_time_ms: 1.0,
            ttfb_ms: 2.0,
        }];
        let speed = engine.calculate_block_speed(&measurements);
        assert!((speed - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_block_speed_single_measurement() {
        let engine = TestEngine::new(TestConfig::default());
        let measurements = vec![BandwidthMeasurement {
            bytes: 100_000,
            bandwidth_bps: 10_000_000.0, // 10 Mbps
            duration_ms: 15.0,
            server_time_ms: 1.0,
            ttfb_ms: 5.0,
        }];
        let speed = engine.calculate_block_speed(&measurements);
        // 10_000_000 bps = 10 Mbps
        assert!((speed - 10.0).abs() < 0.001);
    }
}
