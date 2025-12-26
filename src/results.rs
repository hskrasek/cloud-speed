//! Result data structures for speed test output.
//!
//! This module provides comprehensive data structures for representing
//! all speed test results, including metadata, latency, bandwidth,
//! packet loss, and AIM scores. All structures implement Serialize
//! for JSON output.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::cloudflare::tests::engine::{
    BandwidthResults as EngineBandwidthResults,
    LatencyResults as EngineLatencyResults,
    SizeMeasurement as EngineSizeMeasurement, SpeedTestOutput,
};
use crate::cloudflare::tests::packet_loss::PacketLossResult as EnginePacketLossResult;
use crate::scoring::{AimScores, ConnectionMetrics, QualityScore};

/// Complete results from a speed test run.
///
/// This struct contains all measurement results, metadata, and scores
/// from a complete speed test execution. It implements Serialize for
/// JSON output.
///
/// # Requirements
/// - Includes all measurement results, metadata, and scores
/// - Implements Serialize for JSON output
/// - _Requirements: 10.4_
///
/// # Example
/// ```no_run
/// use cloud_speed::results::SpeedTestResults;
///
/// let results = SpeedTestResults::new(
///     server_location,
///     connection_meta,
///     latency_results,
///     download_results,
///     upload_results,
///     packet_loss,
///     aim_scores,
/// );
///
/// // Serialize to JSON
/// let json = serde_json::to_string_pretty(&results)?;
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct SpeedTestResults {
    /// Timestamp when the test was completed
    pub timestamp: DateTime<Utc>,
    /// Server location information
    pub server: ServerLocation,
    /// Connection metadata (ISP, IP, etc.)
    pub connection: ConnectionMeta,
    /// Latency measurement results
    pub latency: LatencyResults,
    /// Download bandwidth results
    pub download: BandwidthResults,
    /// Upload bandwidth results
    pub upload: BandwidthResults,
    /// Packet loss measurement results (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_loss: Option<PacketLossResults>,
    /// AIM quality scores
    pub scores: AimScoresOutput,
}

impl SpeedTestResults {
    /// Create a new SpeedTestResults from component results.
    pub fn new(
        server: ServerLocation,
        connection: ConnectionMeta,
        latency: LatencyResults,
        download: BandwidthResults,
        upload: BandwidthResults,
        packet_loss: Option<PacketLossResults>,
        scores: AimScoresOutput,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            server,
            connection,
            latency,
            download,
            upload,
            packet_loss,
            scores,
        }
    }

    /// Create SpeedTestResults from engine output and additional data.
    pub fn from_engine_output(
        output: &SpeedTestOutput,
        server: ServerLocation,
        connection: ConnectionMeta,
        packet_loss: Option<&EnginePacketLossResult>,
    ) -> Self {
        let latency = LatencyResults::from_engine(&output.latency);
        let download = BandwidthResults::from_engine(&output.download);
        let upload = BandwidthResults::from_engine(&output.upload);

        let packet_loss_results = packet_loss
            .filter(|p| p.is_available())
            .map(PacketLossResults::from_engine);

        // Calculate AIM scores
        let metrics = ConnectionMetrics::new(
            download.speed_mbps,
            upload.speed_mbps,
            latency.idle_ms,
            latency.idle_jitter_ms.unwrap_or(0.0),
        )
        .with_loaded_latency(latency.loaded_down_ms, latency.loaded_up_ms);

        let metrics = if let Some(ref pl) = packet_loss_results {
            metrics.with_packet_loss(pl.ratio)
        } else {
            metrics
        };

        let aim_scores = crate::scoring::calculate_aim_scores(&metrics);
        let scores = AimScoresOutput::from_aim_scores(&aim_scores);

        Self {
            timestamp: Utc::now(),
            server,
            connection,
            latency,
            download,
            upload,
            packet_loss: packet_loss_results,
            scores,
        }
    }
}

/// Server location information.
#[derive(Debug, Clone, Serialize)]
pub struct ServerLocation {
    /// City name
    pub city: String,
    /// IATA airport code (e.g., "SFO", "LAX")
    pub iata: String,
}

impl ServerLocation {
    /// Create a new ServerLocation.
    pub fn new(city: String, iata: String) -> Self {
        Self { city, iata }
    }
}

/// Connection metadata.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionMeta {
    /// Client IP address
    pub ip: String,
    /// Country code (e.g., "US", "GB")
    pub country: String,
    /// ISP/Organization name
    pub isp: String,
    /// Autonomous System Number
    pub asn: i64,
}

impl ConnectionMeta {
    /// Create a new ConnectionMeta.
    pub fn new(ip: String, country: String, isp: String, asn: i64) -> Self {
        Self { ip, country, isp, asn }
    }
}

/// Latency measurement results.
///
/// Contains idle and loaded latency/jitter measurements for both
/// download and upload directions.
///
/// # Requirements
/// - Include idle and loaded latency/jitter for both directions
/// - _Requirements: 2.4, 3.1, 6.6, 6.7_
#[derive(Debug, Clone, Serialize)]
pub struct LatencyResults {
    /// Idle latency (median) in milliseconds
    pub idle_ms: f64,
    /// Idle jitter in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_jitter_ms: Option<f64>,
    /// Loaded latency during downloads (median) in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_down_ms: Option<f64>,
    /// Loaded jitter during downloads in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_down_jitter_ms: Option<f64>,
    /// Loaded latency during uploads (median) in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_up_ms: Option<f64>,
    /// Loaded jitter during uploads in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_up_jitter_ms: Option<f64>,
}

impl LatencyResults {
    /// Create a new LatencyResults with all values.
    pub fn new(
        idle_ms: f64,
        idle_jitter_ms: Option<f64>,
        loaded_down_ms: Option<f64>,
        loaded_down_jitter_ms: Option<f64>,
        loaded_up_ms: Option<f64>,
        loaded_up_jitter_ms: Option<f64>,
    ) -> Self {
        Self {
            idle_ms,
            idle_jitter_ms,
            loaded_down_ms,
            loaded_down_jitter_ms,
            loaded_up_ms,
            loaded_up_jitter_ms,
        }
    }

    /// Create LatencyResults from engine output.
    pub fn from_engine(engine: &EngineLatencyResults) -> Self {
        Self {
            idle_ms: engine.idle_ms,
            idle_jitter_ms: engine.idle_jitter_ms,
            loaded_down_ms: engine.loaded_down_ms,
            loaded_down_jitter_ms: engine.loaded_down_jitter_ms,
            loaded_up_ms: engine.loaded_up_ms,
            loaded_up_jitter_ms: engine.loaded_up_jitter_ms,
        }
    }

    /// Create LatencyResults with only idle measurements.
    pub fn idle_only(idle_ms: f64, idle_jitter_ms: Option<f64>) -> Self {
        Self {
            idle_ms,
            idle_jitter_ms,
            loaded_down_ms: None,
            loaded_down_jitter_ms: None,
            loaded_up_ms: None,
            loaded_up_jitter_ms: None,
        }
    }
}

/// Bandwidth measurement results (download or upload).
///
/// Contains the final speed and per-size measurements.
///
/// # Requirements
/// - Include final speed and per-size measurements
/// - _Requirements: 4.7_
#[derive(Debug, Clone, Serialize)]
pub struct BandwidthResults {
    /// Final speed in Mbps (90th percentile of all measurements)
    pub speed_mbps: f64,
    /// Per-size measurement results
    pub measurements: Vec<SizeMeasurement>,
    /// Whether early termination was applied
    pub early_terminated: bool,
}

impl BandwidthResults {
    /// Create a new BandwidthResults.
    pub fn new(
        speed_mbps: f64,
        measurements: Vec<SizeMeasurement>,
        early_terminated: bool,
    ) -> Self {
        Self { speed_mbps, measurements, early_terminated }
    }

    /// Create BandwidthResults from engine output.
    pub fn from_engine(engine: &EngineBandwidthResults) -> Self {
        Self {
            speed_mbps: engine.speed_mbps,
            measurements: engine
                .measurements
                .iter()
                .map(SizeMeasurement::from_engine)
                .collect(),
            early_terminated: engine.early_terminated,
        }
    }
}

/// Results from a single bandwidth measurement set (one file size).
#[derive(Debug, Clone, Serialize)]
pub struct SizeMeasurement {
    /// Size of the data block in bytes
    pub bytes: u64,
    /// Calculated speed in Mbps for this size
    pub speed_mbps: f64,
    /// Number of measurements performed
    pub count: usize,
}

impl SizeMeasurement {
    /// Create a new SizeMeasurement.
    pub fn new(bytes: u64, speed_mbps: f64, count: usize) -> Self {
        Self { bytes, speed_mbps, count }
    }

    /// Create SizeMeasurement from engine output.
    pub fn from_engine(engine: &EngineSizeMeasurement) -> Self {
        Self {
            bytes: engine.bytes,
            speed_mbps: engine.speed_mbps,
            count: engine.count,
        }
    }
}

/// Packet loss measurement results.
#[derive(Debug, Clone, Serialize)]
pub struct PacketLossResults {
    /// Packet loss ratio (0.0 to 1.0)
    pub ratio: f64,
    /// Packet loss as percentage (0.0 to 100.0)
    pub percent: f64,
    /// Number of packets sent
    pub packets_sent: usize,
    /// Number of packets lost
    pub packets_lost: usize,
    /// Number of packets received
    pub packets_received: usize,
    /// Average round-trip time in milliseconds (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_rtt_ms: Option<f64>,
}

impl PacketLossResults {
    /// Create a new PacketLossResults.
    pub fn new(
        ratio: f64,
        packets_sent: usize,
        packets_lost: usize,
        packets_received: usize,
        avg_rtt_ms: Option<f64>,
    ) -> Self {
        Self {
            ratio,
            percent: ratio * 100.0,
            packets_sent,
            packets_lost,
            packets_received,
            avg_rtt_ms,
        }
    }

    /// Create PacketLossResults from engine output.
    pub fn from_engine(engine: &EnginePacketLossResult) -> Self {
        Self {
            ratio: engine.packet_loss_ratio,
            percent: engine.packet_loss_percent(),
            packets_sent: engine.packets_sent,
            packets_lost: engine.packets_lost,
            packets_received: engine.packets_received,
            avg_rtt_ms: engine.avg_rtt_ms,
        }
    }
}

/// AIM (Aggregated Internet Measurement) scores for JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct AimScoresOutput {
    /// Quality score for video streaming
    pub streaming: String,
    /// Quality score for online gaming
    pub gaming: String,
    /// Quality score for video conferencing
    pub video_conferencing: String,
    /// Overall quality score (minimum of all)
    pub overall: String,
}

impl AimScoresOutput {
    /// Create AimScoresOutput from AimScores.
    pub fn from_aim_scores(scores: &AimScores) -> Self {
        Self {
            streaming: quality_score_to_string(&scores.streaming),
            gaming: quality_score_to_string(&scores.gaming),
            video_conferencing: quality_score_to_string(
                &scores.video_conferencing,
            ),
            overall: quality_score_to_string(&scores.overall()),
        }
    }
}

/// Convert QualityScore to a lowercase string for JSON output.
fn quality_score_to_string(score: &QualityScore) -> String {
    match score {
        QualityScore::Great => "great".to_string(),
        QualityScore::Good => "good".to_string(),
        QualityScore::Average => "average".to_string(),
        QualityScore::Poor => "poor".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_location_new() {
        let loc = ServerLocation::new(
            "San Francisco".to_string(),
            "SFO".to_string(),
        );
        assert_eq!(loc.city, "San Francisco");
        assert_eq!(loc.iata, "SFO");
    }

    #[test]
    fn test_connection_meta_new() {
        let meta = ConnectionMeta::new(
            "192.168.1.1".to_string(),
            "US".to_string(),
            "Example ISP".to_string(),
            12345,
        );
        assert_eq!(meta.ip, "192.168.1.1");
        assert_eq!(meta.country, "US");
        assert_eq!(meta.isp, "Example ISP");
        assert_eq!(meta.asn, 12345);
    }

    #[test]
    fn test_latency_results_new() {
        let latency = LatencyResults::new(
            15.5,
            Some(2.3),
            Some(25.0),
            Some(5.0),
            Some(30.0),
            Some(6.0),
        );
        assert!((latency.idle_ms - 15.5).abs() < 0.001);
        assert!((latency.idle_jitter_ms.unwrap() - 2.3).abs() < 0.001);
        assert!((latency.loaded_down_ms.unwrap() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_latency_results_idle_only() {
        let latency = LatencyResults::idle_only(15.5, Some(2.3));
        assert!((latency.idle_ms - 15.5).abs() < 0.001);
        assert!(latency.loaded_down_ms.is_none());
        assert!(latency.loaded_up_ms.is_none());
    }

    #[test]
    fn test_bandwidth_results_new() {
        let measurements = vec![
            SizeMeasurement::new(100_000, 50.0, 10),
            SizeMeasurement::new(1_000_000, 75.0, 8),
        ];
        let bandwidth = BandwidthResults::new(80.0, measurements, false);
        assert!((bandwidth.speed_mbps - 80.0).abs() < 0.001);
        assert_eq!(bandwidth.measurements.len(), 2);
        assert!(!bandwidth.early_terminated);
    }

    #[test]
    fn test_size_measurement_new() {
        let measurement = SizeMeasurement::new(100_000, 50.0, 10);
        assert_eq!(measurement.bytes, 100_000);
        assert!((measurement.speed_mbps - 50.0).abs() < 0.001);
        assert_eq!(measurement.count, 10);
    }

    #[test]
    fn test_packet_loss_results_new() {
        let pl = PacketLossResults::new(0.05, 1000, 50, 950, Some(15.5));
        assert!((pl.ratio - 0.05).abs() < 0.001);
        assert!((pl.percent - 5.0).abs() < 0.001);
        assert_eq!(pl.packets_sent, 1000);
        assert_eq!(pl.packets_lost, 50);
        assert_eq!(pl.packets_received, 950);
    }

    #[test]
    fn test_aim_scores_output() {
        let scores = AimScores::new(
            QualityScore::Great,
            QualityScore::Good,
            QualityScore::Average,
        );
        let output = AimScoresOutput::from_aim_scores(&scores);
        assert_eq!(output.streaming, "great");
        assert_eq!(output.gaming, "good");
        assert_eq!(output.video_conferencing, "average");
        assert_eq!(output.overall, "average");
    }

    #[test]
    fn test_quality_score_to_string() {
        assert_eq!(quality_score_to_string(&QualityScore::Great), "great");
        assert_eq!(quality_score_to_string(&QualityScore::Good), "good");
        assert_eq!(quality_score_to_string(&QualityScore::Average), "average");
        assert_eq!(quality_score_to_string(&QualityScore::Poor), "poor");
    }

    #[test]
    fn test_speed_test_results_serialization() {
        let server = ServerLocation::new(
            "San Francisco".to_string(),
            "SFO".to_string(),
        );
        let connection = ConnectionMeta::new(
            "192.168.1.1".to_string(),
            "US".to_string(),
            "Example ISP".to_string(),
            12345,
        );
        let latency = LatencyResults::idle_only(15.5, Some(2.3));
        let download = BandwidthResults::new(100.0, vec![], false);
        let upload = BandwidthResults::new(50.0, vec![], false);
        let scores = AimScoresOutput {
            streaming: "great".to_string(),
            gaming: "good".to_string(),
            video_conferencing: "good".to_string(),
            overall: "good".to_string(),
        };

        let results = SpeedTestResults::new(
            server, connection, latency, download, upload, None, scores,
        );

        // Test that it serializes without error
        let json = serde_json::to_string(&results);
        assert!(json.is_ok());

        // Verify JSON contains expected fields
        let json_str = json.unwrap();
        assert!(json_str.contains("\"timestamp\""));
        assert!(json_str.contains("\"server\""));
        assert!(json_str.contains("\"connection\""));
        assert!(json_str.contains("\"latency\""));
        assert!(json_str.contains("\"download\""));
        assert!(json_str.contains("\"upload\""));
        assert!(json_str.contains("\"scores\""));
        // packet_loss should be skipped when None
        assert!(!json_str.contains("\"packet_loss\""));
    }

    #[test]
    fn test_speed_test_results_with_packet_loss() {
        let server = ServerLocation::new(
            "San Francisco".to_string(),
            "SFO".to_string(),
        );
        let connection = ConnectionMeta::new(
            "192.168.1.1".to_string(),
            "US".to_string(),
            "Example ISP".to_string(),
            12345,
        );
        let latency = LatencyResults::idle_only(15.5, Some(2.3));
        let download = BandwidthResults::new(100.0, vec![], false);
        let upload = BandwidthResults::new(50.0, vec![], false);
        let packet_loss =
            Some(PacketLossResults::new(0.01, 1000, 10, 990, Some(15.0)));
        let scores = AimScoresOutput {
            streaming: "great".to_string(),
            gaming: "great".to_string(),
            video_conferencing: "great".to_string(),
            overall: "great".to_string(),
        };

        let results = SpeedTestResults::new(
            server,
            connection,
            latency,
            download,
            upload,
            packet_loss,
            scores,
        );

        let json = serde_json::to_string(&results).unwrap();
        // packet_loss should be present when Some
        assert!(json.contains("\"packet_loss\""));
        assert!(json.contains("\"ratio\""));
        assert!(json.contains("\"percent\""));
    }
}
