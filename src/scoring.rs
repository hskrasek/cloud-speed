//! AIM (Aggregated Internet Measurement) scoring module.
//!
//! This module provides functionality to calculate quality scores for different
//! use cases (streaming, gaming, video conferencing) based on network metrics.
//!
//! The scoring is based on the methodology used by Cloudflare's speed test at
//! speed.cloudflare.com.

use serde::Serialize;

/// Quality score categories for network performance.
///
/// Each category represents a level of quality for a specific use case,
/// based on the measured network metrics.
///
/// Variants are ordered from worst to best for correct derived Ord behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityScore {
    /// Poor performance - likely to experience significant issues
    Poor,
    /// Acceptable performance - may experience occasional issues
    Average,
    /// Good performance - suitable for the use case
    Good,
    /// Excellent performance - optimal for the use case
    Great,
}

impl QualityScore {
    /// Returns a human-readable description of the quality score.
    pub fn description(&self) -> &'static str {
        match self {
            QualityScore::Great => "Excellent",
            QualityScore::Good => "Good",
            QualityScore::Average => "Average",
            QualityScore::Poor => "Poor",
        }
    }

    /// Returns true if this score is better than or equal to the other score.
    pub fn is_at_least(&self, other: QualityScore) -> bool {
        *self >= other
    }
}

/// AIM (Aggregated Internet Measurement) scores for different use cases.
///
/// This struct contains quality scores for streaming, gaming, and video
/// conferencing, calculated based on the measured network metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AimScores {
    /// Quality score for video streaming (e.g., Netflix, YouTube)
    pub streaming: QualityScore,
    /// Quality score for online gaming
    pub gaming: QualityScore,
    /// Quality score for video conferencing (e.g., Zoom, Teams)
    pub video_conferencing: QualityScore,
}

impl AimScores {
    /// Creates a new AimScores instance with the given scores.
    pub fn new(
        streaming: QualityScore,
        gaming: QualityScore,
        video_conferencing: QualityScore,
    ) -> Self {
        Self { streaming, gaming, video_conferencing }
    }

    /// Returns the overall quality score (minimum of all scores).
    pub fn overall(&self) -> QualityScore {
        *[self.streaming, self.gaming, self.video_conferencing]
            .iter()
            .min()
            .unwrap()
    }
}

/// Connection metrics used as input for AIM score calculation.
///
/// All speed values are in Mbps, latency and jitter in milliseconds.
#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    /// Download speed in Mbps
    pub download_mbps: f64,
    /// Upload speed in Mbps
    pub upload_mbps: f64,
    /// Idle latency in milliseconds
    pub latency_ms: f64,
    /// Idle jitter in milliseconds
    pub jitter_ms: f64,
    /// Packet loss ratio (0.0 to 1.0), if measured
    pub packet_loss: Option<f64>,
    /// Loaded latency during downloads in milliseconds, if measured
    pub loaded_latency_down_ms: Option<f64>,
    /// Loaded latency during uploads in milliseconds, if measured
    pub loaded_latency_up_ms: Option<f64>,
}

impl ConnectionMetrics {
    /// Creates a new ConnectionMetrics instance with the given values.
    pub fn new(
        download_mbps: f64,
        upload_mbps: f64,
        latency_ms: f64,
        jitter_ms: f64,
    ) -> Self {
        Self {
            download_mbps,
            upload_mbps,
            latency_ms,
            jitter_ms,
            packet_loss: None,
            loaded_latency_down_ms: None,
            loaded_latency_up_ms: None,
        }
    }

    /// Sets the packet loss ratio.
    pub fn with_packet_loss(mut self, packet_loss: f64) -> Self {
        self.packet_loss = Some(packet_loss);
        self
    }

    /// Sets the loaded latency values.
    pub fn with_loaded_latency(
        mut self,
        down_ms: Option<f64>,
        up_ms: Option<f64>,
    ) -> Self {
        self.loaded_latency_down_ms = down_ms;
        self.loaded_latency_up_ms = up_ms;
        self
    }
}

// ============================================================================
// AIM Score Calculation
// ============================================================================

/// Thresholds for streaming quality assessment.
///
/// Based on typical requirements for video streaming services:
/// - Great: 25+ Mbps download, low latency
/// - Good: 10+ Mbps download
/// - Average: 5+ Mbps download
/// - Poor: Below 5 Mbps
mod streaming_thresholds {
    /// Minimum download speed (Mbps) for Great quality
    pub const DOWNLOAD_GREAT: f64 = 25.0;
    /// Minimum download speed (Mbps) for Good quality
    pub const DOWNLOAD_GOOD: f64 = 10.0;
    /// Minimum download speed (Mbps) for Average quality
    pub const DOWNLOAD_AVERAGE: f64 = 5.0;

    /// Maximum latency (ms) for Great quality
    pub const LATENCY_GREAT: f64 = 100.0;
    /// Maximum latency (ms) for Good quality
    pub const LATENCY_GOOD: f64 = 200.0;
    /// Maximum latency (ms) for Average quality
    pub const LATENCY_AVERAGE: f64 = 400.0;
}

/// Thresholds for gaming quality assessment.
///
/// Gaming is highly sensitive to latency and jitter:
/// - Great: <30ms latency, <10ms jitter, <1% packet loss
/// - Good: <50ms latency, <20ms jitter, <2% packet loss
/// - Average: <100ms latency, <30ms jitter, <5% packet loss
/// - Poor: Above average thresholds
mod gaming_thresholds {
    /// Maximum latency (ms) for Great quality
    pub const LATENCY_GREAT: f64 = 30.0;
    /// Maximum latency (ms) for Good quality
    pub const LATENCY_GOOD: f64 = 50.0;
    /// Maximum latency (ms) for Average quality
    pub const LATENCY_AVERAGE: f64 = 100.0;

    /// Maximum jitter (ms) for Great quality
    pub const JITTER_GREAT: f64 = 10.0;
    /// Maximum jitter (ms) for Good quality
    pub const JITTER_GOOD: f64 = 20.0;
    /// Maximum jitter (ms) for Average quality
    pub const JITTER_AVERAGE: f64 = 30.0;

    /// Maximum packet loss (ratio) for Great quality
    pub const PACKET_LOSS_GREAT: f64 = 0.01;
    /// Maximum packet loss (ratio) for Good quality
    pub const PACKET_LOSS_GOOD: f64 = 0.02;
    /// Maximum packet loss (ratio) for Average quality
    pub const PACKET_LOSS_AVERAGE: f64 = 0.05;

    /// Minimum download speed (Mbps) for Great quality
    pub const DOWNLOAD_GREAT: f64 = 15.0;
    /// Minimum download speed (Mbps) for Good quality
    pub const DOWNLOAD_GOOD: f64 = 5.0;
    /// Minimum download speed (Mbps) for Average quality
    pub const DOWNLOAD_AVERAGE: f64 = 3.0;
}

/// Thresholds for video conferencing quality assessment.
///
/// Video conferencing requires balanced upload/download and low latency:
/// - Great: 10+ Mbps up/down, <50ms latency, <15ms jitter
/// - Good: 5+ Mbps up/down, <100ms latency, <30ms jitter
/// - Average: 2+ Mbps up/down, <200ms latency, <50ms jitter
/// - Poor: Below average thresholds
mod video_conferencing_thresholds {
    /// Minimum download speed (Mbps) for Great quality
    pub const DOWNLOAD_GREAT: f64 = 10.0;
    /// Minimum download speed (Mbps) for Good quality
    pub const DOWNLOAD_GOOD: f64 = 5.0;
    /// Minimum download speed (Mbps) for Average quality
    pub const DOWNLOAD_AVERAGE: f64 = 2.0;

    /// Minimum upload speed (Mbps) for Great quality
    pub const UPLOAD_GREAT: f64 = 10.0;
    /// Minimum upload speed (Mbps) for Good quality
    pub const UPLOAD_GOOD: f64 = 5.0;
    /// Minimum upload speed (Mbps) for Average quality
    pub const UPLOAD_AVERAGE: f64 = 2.0;

    /// Maximum latency (ms) for Great quality
    pub const LATENCY_GREAT: f64 = 50.0;
    /// Maximum latency (ms) for Good quality
    pub const LATENCY_GOOD: f64 = 100.0;
    /// Maximum latency (ms) for Average quality
    pub const LATENCY_AVERAGE: f64 = 200.0;

    /// Maximum jitter (ms) for Great quality
    pub const JITTER_GREAT: f64 = 15.0;
    /// Maximum jitter (ms) for Good quality
    pub const JITTER_GOOD: f64 = 30.0;
    /// Maximum jitter (ms) for Average quality
    pub const JITTER_AVERAGE: f64 = 50.0;

    /// Maximum packet loss (ratio) for Great quality
    pub const PACKET_LOSS_GREAT: f64 = 0.01;
    /// Maximum packet loss (ratio) for Good quality
    pub const PACKET_LOSS_GOOD: f64 = 0.03;
    /// Maximum packet loss (ratio) for Average quality
    pub const PACKET_LOSS_AVERAGE: f64 = 0.05;
}

/// Calculates AIM (Aggregated Internet Measurement) scores based on connection
/// metrics.
///
/// This function evaluates the connection quality for three use cases:
/// - Streaming: Primarily based on download speed and latency
/// - Gaming: Highly sensitive to latency, jitter, and packet loss
/// - Video Conferencing: Requires balanced upload/download and low latency
///
/// # Arguments
/// * `metrics` - The connection metrics to evaluate
///
/// # Returns
/// An `AimScores` struct containing quality scores for each use case.
///
/// # Example
/// ```
/// let metrics = ConnectionMetrics::new(100.0, 50.0, 15.0, 2.0);
/// let scores = calculate_aim_scores(&metrics);
/// assert_eq!(scores.streaming, QualityScore::Great);
/// ```
pub fn calculate_aim_scores(metrics: &ConnectionMetrics) -> AimScores {
    AimScores {
        streaming: calculate_streaming_score(metrics),
        gaming: calculate_gaming_score(metrics),
        video_conferencing: calculate_video_conferencing_score(metrics),
    }
}

/// Calculates the streaming quality score.
///
/// Streaming is primarily dependent on download speed, with latency being
/// a secondary factor. Upload speed and jitter have minimal impact.
fn calculate_streaming_score(metrics: &ConnectionMetrics) -> QualityScore {
    use streaming_thresholds::*;

    // Evaluate download speed
    let download_score = if metrics.download_mbps >= DOWNLOAD_GREAT {
        QualityScore::Great
    } else if metrics.download_mbps >= DOWNLOAD_GOOD {
        QualityScore::Good
    } else if metrics.download_mbps >= DOWNLOAD_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate latency (use loaded latency if available, otherwise idle)
    let effective_latency =
        metrics.loaded_latency_down_ms.unwrap_or(metrics.latency_ms);

    let latency_score = if effective_latency <= LATENCY_GREAT {
        QualityScore::Great
    } else if effective_latency <= LATENCY_GOOD {
        QualityScore::Good
    } else if effective_latency <= LATENCY_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Return the minimum of the two scores
    std::cmp::min(download_score, latency_score)
}

/// Calculates the gaming quality score.
///
/// Gaming is highly sensitive to latency, jitter, and packet loss.
/// Download speed is less critical but still considered.
fn calculate_gaming_score(metrics: &ConnectionMetrics) -> QualityScore {
    use gaming_thresholds::*;

    // Evaluate latency (use loaded latency if available for more realistic gaming
    // scenario)
    let effective_latency = metrics
        .loaded_latency_down_ms
        .or(metrics.loaded_latency_up_ms)
        .unwrap_or(metrics.latency_ms);

    let latency_score = if effective_latency <= LATENCY_GREAT {
        QualityScore::Great
    } else if effective_latency <= LATENCY_GOOD {
        QualityScore::Good
    } else if effective_latency <= LATENCY_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate jitter
    let jitter_score = if metrics.jitter_ms <= JITTER_GREAT {
        QualityScore::Great
    } else if metrics.jitter_ms <= JITTER_GOOD {
        QualityScore::Good
    } else if metrics.jitter_ms <= JITTER_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate packet loss (if available)
    let packet_loss_score = match metrics.packet_loss {
        Some(loss) if loss <= PACKET_LOSS_GREAT => QualityScore::Great,
        Some(loss) if loss <= PACKET_LOSS_GOOD => QualityScore::Good,
        Some(loss) if loss <= PACKET_LOSS_AVERAGE => QualityScore::Average,
        Some(_) => QualityScore::Poor,
        // If packet loss is not measured, don't penalize
        None => QualityScore::Great,
    };

    // Evaluate download speed
    let download_score = if metrics.download_mbps >= DOWNLOAD_GREAT {
        QualityScore::Great
    } else if metrics.download_mbps >= DOWNLOAD_GOOD {
        QualityScore::Good
    } else if metrics.download_mbps >= DOWNLOAD_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Return the minimum of all scores
    [latency_score, jitter_score, packet_loss_score, download_score]
        .into_iter()
        .min()
        .unwrap()
}

/// Calculates the video conferencing quality score.
///
/// Video conferencing requires balanced upload and download speeds,
/// low latency, and low jitter for smooth two-way communication.
fn calculate_video_conferencing_score(
    metrics: &ConnectionMetrics,
) -> QualityScore {
    use video_conferencing_thresholds::*;

    // Evaluate download speed
    let download_score = if metrics.download_mbps >= DOWNLOAD_GREAT {
        QualityScore::Great
    } else if metrics.download_mbps >= DOWNLOAD_GOOD {
        QualityScore::Good
    } else if metrics.download_mbps >= DOWNLOAD_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate upload speed
    let upload_score = if metrics.upload_mbps >= UPLOAD_GREAT {
        QualityScore::Great
    } else if metrics.upload_mbps >= UPLOAD_GOOD {
        QualityScore::Good
    } else if metrics.upload_mbps >= UPLOAD_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate latency (use loaded latency if available)
    let effective_latency = metrics
        .loaded_latency_up_ms
        .or(metrics.loaded_latency_down_ms)
        .unwrap_or(metrics.latency_ms);

    let latency_score = if effective_latency <= LATENCY_GREAT {
        QualityScore::Great
    } else if effective_latency <= LATENCY_GOOD {
        QualityScore::Good
    } else if effective_latency <= LATENCY_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate jitter
    let jitter_score = if metrics.jitter_ms <= JITTER_GREAT {
        QualityScore::Great
    } else if metrics.jitter_ms <= JITTER_GOOD {
        QualityScore::Good
    } else if metrics.jitter_ms <= JITTER_AVERAGE {
        QualityScore::Average
    } else {
        QualityScore::Poor
    };

    // Evaluate packet loss (if available)
    let packet_loss_score = match metrics.packet_loss {
        Some(loss) if loss <= PACKET_LOSS_GREAT => QualityScore::Great,
        Some(loss) if loss <= PACKET_LOSS_GOOD => QualityScore::Good,
        Some(loss) if loss <= PACKET_LOSS_AVERAGE => QualityScore::Average,
        Some(_) => QualityScore::Poor,
        // If packet loss is not measured, don't penalize
        None => QualityScore::Great,
    };

    // Return the minimum of all scores
    [
        download_score,
        upload_score,
        latency_score,
        jitter_score,
        packet_loss_score,
    ]
    .into_iter()
    .min()
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Unit tests for QualityScore
    // ========================================================================

    #[test]
    fn test_quality_score_ordering() {
        assert!(QualityScore::Great > QualityScore::Good);
        assert!(QualityScore::Good > QualityScore::Average);
        assert!(QualityScore::Average > QualityScore::Poor);
    }

    #[test]
    fn test_quality_score_is_at_least() {
        assert!(QualityScore::Great.is_at_least(QualityScore::Great));
        assert!(QualityScore::Great.is_at_least(QualityScore::Good));
        assert!(QualityScore::Great.is_at_least(QualityScore::Average));
        assert!(QualityScore::Great.is_at_least(QualityScore::Poor));

        assert!(!QualityScore::Poor.is_at_least(QualityScore::Average));
        assert!(!QualityScore::Average.is_at_least(QualityScore::Good));
        assert!(!QualityScore::Good.is_at_least(QualityScore::Great));
    }

    #[test]
    fn test_quality_score_description() {
        assert_eq!(QualityScore::Great.description(), "Excellent");
        assert_eq!(QualityScore::Good.description(), "Good");
        assert_eq!(QualityScore::Average.description(), "Average");
        assert_eq!(QualityScore::Poor.description(), "Poor");
    }

    // ========================================================================
    // Unit tests for AimScores
    // ========================================================================

    #[test]
    fn test_aim_scores_overall() {
        let scores = AimScores::new(
            QualityScore::Great,
            QualityScore::Good,
            QualityScore::Average,
        );
        assert_eq!(scores.overall(), QualityScore::Average);

        let all_great = AimScores::new(
            QualityScore::Great,
            QualityScore::Great,
            QualityScore::Great,
        );
        assert_eq!(all_great.overall(), QualityScore::Great);
    }

    // ========================================================================
    // Unit tests for streaming score
    // ========================================================================

    #[test]
    fn test_streaming_great_score() {
        // High download, low latency
        let metrics = ConnectionMetrics::new(100.0, 50.0, 20.0, 5.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.streaming, QualityScore::Great);
    }

    #[test]
    fn test_streaming_good_score() {
        // Good download (10-25 Mbps), acceptable latency
        let metrics = ConnectionMetrics::new(15.0, 10.0, 50.0, 10.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.streaming, QualityScore::Good);
    }

    #[test]
    fn test_streaming_average_score() {
        // Average download (5-10 Mbps)
        let metrics = ConnectionMetrics::new(7.0, 5.0, 100.0, 15.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.streaming, QualityScore::Average);
    }

    #[test]
    fn test_streaming_poor_score() {
        // Low download (<5 Mbps)
        let metrics = ConnectionMetrics::new(3.0, 2.0, 50.0, 10.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.streaming, QualityScore::Poor);
    }

    #[test]
    fn test_streaming_limited_by_latency() {
        // Great download but poor latency
        let metrics = ConnectionMetrics::new(100.0, 50.0, 500.0, 5.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.streaming, QualityScore::Poor);
    }

    // ========================================================================
    // Unit tests for gaming score
    // ========================================================================

    #[test]
    fn test_gaming_great_score() {
        // Low latency, low jitter, no packet loss
        let metrics = ConnectionMetrics::new(50.0, 20.0, 20.0, 5.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.gaming, QualityScore::Great);
    }

    #[test]
    fn test_gaming_good_score() {
        // Moderate latency and jitter
        let metrics = ConnectionMetrics::new(20.0, 10.0, 40.0, 15.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.gaming, QualityScore::Good);
    }

    #[test]
    fn test_gaming_poor_due_to_latency() {
        // High latency
        let metrics = ConnectionMetrics::new(100.0, 50.0, 150.0, 5.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.gaming, QualityScore::Poor);
    }

    #[test]
    fn test_gaming_poor_due_to_jitter() {
        // High jitter
        let metrics = ConnectionMetrics::new(100.0, 50.0, 20.0, 50.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.gaming, QualityScore::Poor);
    }

    #[test]
    fn test_gaming_poor_due_to_packet_loss() {
        // High packet loss
        let metrics = ConnectionMetrics::new(100.0, 50.0, 20.0, 5.0)
            .with_packet_loss(0.1);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.gaming, QualityScore::Poor);
    }

    #[test]
    fn test_gaming_with_acceptable_packet_loss() {
        // Low packet loss should still be great
        let metrics = ConnectionMetrics::new(50.0, 20.0, 20.0, 5.0)
            .with_packet_loss(0.005);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.gaming, QualityScore::Great);
    }

    // ========================================================================
    // Unit tests for video conferencing score
    // ========================================================================

    #[test]
    fn test_video_conferencing_great_score() {
        // High upload/download, low latency and jitter
        let metrics = ConnectionMetrics::new(50.0, 30.0, 30.0, 10.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.video_conferencing, QualityScore::Great);
    }

    #[test]
    fn test_video_conferencing_good_score() {
        // Moderate speeds and latency
        let metrics = ConnectionMetrics::new(8.0, 6.0, 80.0, 20.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.video_conferencing, QualityScore::Good);
    }

    #[test]
    fn test_video_conferencing_limited_by_upload() {
        // Great download but poor upload
        let metrics = ConnectionMetrics::new(100.0, 1.0, 30.0, 10.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.video_conferencing, QualityScore::Poor);
    }

    #[test]
    fn test_video_conferencing_limited_by_jitter() {
        // Good speeds but high jitter
        let metrics = ConnectionMetrics::new(50.0, 30.0, 30.0, 60.0);
        let scores = calculate_aim_scores(&metrics);
        assert_eq!(scores.video_conferencing, QualityScore::Poor);
    }

    #[test]
    fn test_video_conferencing_with_loaded_latency() {
        // Use loaded latency when available
        let metrics = ConnectionMetrics::new(50.0, 30.0, 30.0, 10.0)
            .with_loaded_latency(None, Some(250.0));
        let scores = calculate_aim_scores(&metrics);
        // Loaded latency of 250ms should result in Poor
        assert_eq!(scores.video_conferencing, QualityScore::Poor);
    }

    // ========================================================================
    // Unit tests for ConnectionMetrics builder
    // ========================================================================

    #[test]
    fn test_connection_metrics_builder() {
        let metrics = ConnectionMetrics::new(100.0, 50.0, 15.0, 2.0)
            .with_packet_loss(0.01)
            .with_loaded_latency(Some(20.0), Some(25.0));

        assert_eq!(metrics.download_mbps, 100.0);
        assert_eq!(metrics.upload_mbps, 50.0);
        assert_eq!(metrics.latency_ms, 15.0);
        assert_eq!(metrics.jitter_ms, 2.0);
        assert_eq!(metrics.packet_loss, Some(0.01));
        assert_eq!(metrics.loaded_latency_down_ms, Some(20.0));
        assert_eq!(metrics.loaded_latency_up_ms, Some(25.0));
    }

    // ========================================================================
    // Property-based tests for AIM score categorization
    // Feature: cloudflare-speedtest-parity, Property 10: AIM Score Categorization
    // Validates: Requirements 8.3
    // ========================================================================

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: Each AIM score SHALL be exactly one of: Great, Good, Average, Poor.
        /// The categorization SHALL be deterministic (same inputs always produce same outputs).
        #[test]
        fn aim_scores_are_valid_categories(
            download_mbps in 0.1f64..1000.0f64,
            upload_mbps in 0.1f64..500.0f64,
            latency_ms in 1.0f64..500.0f64,
            jitter_ms in 0.1f64..100.0f64,
            packet_loss in proptest::option::of(0.0f64..0.5f64),
            loaded_latency_down in proptest::option::of(1.0f64..500.0f64),
            loaded_latency_up in proptest::option::of(1.0f64..500.0f64),
        ) {
            let metrics = ConnectionMetrics {
                download_mbps,
                upload_mbps,
                latency_ms,
                jitter_ms,
                packet_loss,
                loaded_latency_down_ms: loaded_latency_down,
                loaded_latency_up_ms: loaded_latency_up,
            };

            let scores = calculate_aim_scores(&metrics);

            // Verify each score is a valid category
            let valid_scores = [
                QualityScore::Great,
                QualityScore::Good,
                QualityScore::Average,
                QualityScore::Poor,
            ];

            prop_assert!(
                valid_scores.contains(&scores.streaming),
                "Streaming score {:?} is not a valid category",
                scores.streaming
            );
            prop_assert!(
                valid_scores.contains(&scores.gaming),
                "Gaming score {:?} is not a valid category",
                scores.gaming
            );
            prop_assert!(
                valid_scores.contains(&scores.video_conferencing),
                "Video conferencing score {:?} is not a valid category",
                scores.video_conferencing
            );

            // Verify determinism: same inputs produce same outputs
            let scores2 = calculate_aim_scores(&metrics);
            prop_assert_eq!(
                scores.streaming, scores2.streaming,
                "Streaming score is not deterministic"
            );
            prop_assert_eq!(
                scores.gaming, scores2.gaming,
                "Gaming score is not deterministic"
            );
            prop_assert_eq!(
                scores.video_conferencing, scores2.video_conferencing,
                "Video conferencing score is not deterministic"
            );
        }

        /// Property: Better metrics SHALL never produce a worse score than poorer metrics.
        /// Specifically: higher download speed should never decrease the streaming score.
        #[test]
        fn better_download_never_decreases_streaming_score(
            base_download in 1.0f64..100.0f64,
            improvement in 0.1f64..100.0f64,
            upload_mbps in 0.1f64..100.0f64,
            latency_ms in 1.0f64..200.0f64,
            jitter_ms in 0.1f64..50.0f64,
        ) {
            let base_metrics = ConnectionMetrics::new(
                base_download,
                upload_mbps,
                latency_ms,
                jitter_ms,
            );

            let improved_metrics = ConnectionMetrics::new(
                base_download + improvement,
                upload_mbps,
                latency_ms,
                jitter_ms,
            );

            let base_scores = calculate_aim_scores(&base_metrics);
            let improved_scores = calculate_aim_scores(&improved_metrics);

            prop_assert!(
                improved_scores.streaming >= base_scores.streaming,
                "Higher download ({} -> {}) should not decrease streaming score ({:?} -> {:?})",
                base_download, base_download + improvement,
                base_scores.streaming, improved_scores.streaming
            );
        }

        /// Property: Better metrics SHALL never produce a worse score than poorer metrics.
        /// Specifically: lower latency should never decrease the gaming score.
        #[test]
        fn lower_latency_never_decreases_gaming_score(
            download_mbps in 10.0f64..100.0f64,
            upload_mbps in 5.0f64..50.0f64,
            base_latency in 10.0f64..200.0f64,
            latency_reduction in 1.0f64..50.0f64,
            jitter_ms in 0.1f64..20.0f64,
        ) {
            // Ensure improved latency is still positive
            let improved_latency = (base_latency - latency_reduction).max(1.0);

            let base_metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                base_latency,
                jitter_ms,
            );

            let improved_metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                improved_latency,
                jitter_ms,
            );

            let base_scores = calculate_aim_scores(&base_metrics);
            let improved_scores = calculate_aim_scores(&improved_metrics);

            prop_assert!(
                improved_scores.gaming >= base_scores.gaming,
                "Lower latency ({} -> {}) should not decrease gaming score ({:?} -> {:?})",
                base_latency, improved_latency,
                base_scores.gaming, improved_scores.gaming
            );
        }

        /// Property: Better metrics SHALL never produce a worse score than poorer metrics.
        /// Specifically: higher upload speed should never decrease the video conferencing score.
        #[test]
        fn better_upload_never_decreases_video_conferencing_score(
            download_mbps in 10.0f64..100.0f64,
            base_upload in 1.0f64..50.0f64,
            improvement in 0.1f64..50.0f64,
            latency_ms in 1.0f64..100.0f64,
            jitter_ms in 0.1f64..30.0f64,
        ) {
            let base_metrics = ConnectionMetrics::new(
                download_mbps,
                base_upload,
                latency_ms,
                jitter_ms,
            );

            let improved_metrics = ConnectionMetrics::new(
                download_mbps,
                base_upload + improvement,
                latency_ms,
                jitter_ms,
            );

            let base_scores = calculate_aim_scores(&base_metrics);
            let improved_scores = calculate_aim_scores(&improved_metrics);

            prop_assert!(
                improved_scores.video_conferencing >= base_scores.video_conferencing,
                "Higher upload ({} -> {}) should not decrease video conferencing score ({:?} -> {:?})",
                base_upload, base_upload + improvement,
                base_scores.video_conferencing, improved_scores.video_conferencing
            );
        }

        /// Property: Better metrics SHALL never produce a worse score than poorer metrics.
        /// Specifically: lower jitter should never decrease any score.
        #[test]
        fn lower_jitter_never_decreases_scores(
            download_mbps in 10.0f64..100.0f64,
            upload_mbps in 5.0f64..50.0f64,
            latency_ms in 1.0f64..100.0f64,
            base_jitter in 5.0f64..80.0f64,
            jitter_reduction in 1.0f64..30.0f64,
        ) {
            // Ensure improved jitter is still positive
            let improved_jitter = (base_jitter - jitter_reduction).max(0.1);

            let base_metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                latency_ms,
                base_jitter,
            );

            let improved_metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                latency_ms,
                improved_jitter,
            );

            let base_scores = calculate_aim_scores(&base_metrics);
            let improved_scores = calculate_aim_scores(&improved_metrics);

            prop_assert!(
                improved_scores.gaming >= base_scores.gaming,
                "Lower jitter ({} -> {}) should not decrease gaming score ({:?} -> {:?})",
                base_jitter, improved_jitter,
                base_scores.gaming, improved_scores.gaming
            );

            prop_assert!(
                improved_scores.video_conferencing >= base_scores.video_conferencing,
                "Lower jitter ({} -> {}) should not decrease video conferencing score ({:?} -> {:?})",
                base_jitter, improved_jitter,
                base_scores.video_conferencing, improved_scores.video_conferencing
            );
        }

        /// Property: Better metrics SHALL never produce a worse score than poorer metrics.
        /// Specifically: lower packet loss should never decrease any score.
        #[test]
        fn lower_packet_loss_never_decreases_scores(
            download_mbps in 10.0f64..100.0f64,
            upload_mbps in 5.0f64..50.0f64,
            latency_ms in 1.0f64..100.0f64,
            jitter_ms in 0.1f64..30.0f64,
            base_packet_loss in 0.01f64..0.2f64,
            packet_loss_reduction in 0.001f64..0.1f64,
        ) {
            // Ensure improved packet loss is still non-negative
            let improved_packet_loss = (base_packet_loss - packet_loss_reduction).max(0.0);

            let base_metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                latency_ms,
                jitter_ms,
            ).with_packet_loss(base_packet_loss);

            let improved_metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                latency_ms,
                jitter_ms,
            ).with_packet_loss(improved_packet_loss);

            let base_scores = calculate_aim_scores(&base_metrics);
            let improved_scores = calculate_aim_scores(&improved_metrics);

            prop_assert!(
                improved_scores.gaming >= base_scores.gaming,
                "Lower packet loss ({} -> {}) should not decrease gaming score ({:?} -> {:?})",
                base_packet_loss, improved_packet_loss,
                base_scores.gaming, improved_scores.gaming
            );

            prop_assert!(
                improved_scores.video_conferencing >= base_scores.video_conferencing,
                "Lower packet loss ({} -> {}) should not decrease video conferencing score ({:?} -> {:?})",
                base_packet_loss, improved_packet_loss,
                base_scores.video_conferencing, improved_scores.video_conferencing
            );
        }

        /// Property: Overall score should be the minimum of all individual scores.
        #[test]
        fn overall_score_is_minimum(
            download_mbps in 0.1f64..200.0f64,
            upload_mbps in 0.1f64..100.0f64,
            latency_ms in 1.0f64..300.0f64,
            jitter_ms in 0.1f64..80.0f64,
        ) {
            let metrics = ConnectionMetrics::new(
                download_mbps,
                upload_mbps,
                latency_ms,
                jitter_ms,
            );

            let scores = calculate_aim_scores(&metrics);
            let overall = scores.overall();

            let min_score = [scores.streaming, scores.gaming, scores.video_conferencing]
                .into_iter()
                .min()
                .unwrap();

            prop_assert_eq!(
                overall, min_score,
                "Overall score {:?} should equal minimum of individual scores {:?}",
                overall, min_score
            );
        }
    }
}
