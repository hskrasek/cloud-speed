//! TUI state management.
//!
//! Holds all state needed for rendering the TUI, including
//! connection metadata, test progress, and results.

use super::progress::{BandwidthDirection, ProgressEvent, TestPhase};
use crate::stats::median_f64;

/// Server location information.
#[derive(Debug, Clone, Default)]
pub struct ServerInfo {
    /// City name
    pub city: String,
    /// IATA airport code
    pub iata: String,
}

/// Connection metadata.
#[derive(Debug, Clone, Default)]
pub struct ConnectionInfo {
    /// Client IP address
    pub ip: String,
    /// Country code
    pub country: String,
    /// ISP name
    pub isp: String,
    /// Autonomous System Number
    pub asn: i64,
}

/// Error information for display.
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    /// Error message
    pub message: String,
    /// Optional suggestion for resolution
    pub suggestion: Option<String>,
}

/// Latency measurement state.
#[derive(Debug, Clone, Default)]
pub struct LatencyState {
    /// Individual latency measurements in ms
    pub measurements: Vec<f64>,
    /// Current measurement number
    pub current: usize,
    /// Total number of measurements
    pub total: usize,
    /// Calculated median latency in ms
    pub median_ms: Option<f64>,
    /// Calculated jitter in ms
    pub jitter_ms: Option<f64>,
    /// Loaded latency during download (ms)
    pub loaded_down_ms: Option<f64>,
    /// Loaded jitter during download (ms)
    pub loaded_down_jitter_ms: Option<f64>,
    /// Loaded latency during upload (ms)
    pub loaded_up_ms: Option<f64>,
    /// Loaded jitter during upload (ms)
    pub loaded_up_jitter_ms: Option<f64>,
}

impl LatencyState {
    /// Calculate jitter from measurements.
    ///
    /// Jitter is the mean of absolute differences between consecutive
    /// measurements. Requires at least 2 measurements.
    fn calculate_jitter(&self) -> Option<f64> {
        if self.measurements.len() < 2 {
            return None;
        }

        let jitters: Vec<f64> = self
            .measurements
            .windows(2)
            .map(|pair| (pair[0] - pair[1]).abs())
            .collect();

        Some(jitters.iter().sum::<f64>() / jitters.len() as f64)
    }
}

/// Single speed measurement for history tracking.
#[derive(Debug, Clone, Copy)]
pub struct SpeedSample {
    /// Speed in Mbps
    pub speed_mbps: f64,
    /// Timestamp (relative, for graph positioning)
    #[allow(dead_code)]
    pub timestamp: f64,
}

/// Bandwidth measurement state.
#[derive(Debug, Clone, Default)]
pub struct BandwidthState {
    /// Current speed in Mbps
    pub current_speed_mbps: Option<f64>,
    /// Current bytes transferred
    pub current_bytes: u64,
    /// Current measurement number
    pub current_measurement: usize,
    /// Total number of measurements
    pub total_measurements: usize,
    /// Final calculated speed in Mbps
    pub final_speed_mbps: Option<f64>,
    /// Whether this phase is completed
    pub completed: bool,
    /// Speed history for graph display
    pub speed_history: Vec<SpeedSample>,
    /// 90th percentile speed
    pub percentile_90: Option<f64>,
}

/// Quality score for a use case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityRating {
    Great,
    Good,
    Average,
    Poor,
}

impl QualityRating {
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityRating::Great => "Great",
            QualityRating::Good => "Good",
            QualityRating::Average => "Average",
            QualityRating::Poor => "Poor",
        }
    }
}

/// Network quality scores for different use cases.
#[derive(Debug, Clone, Default)]
pub struct QualityScores {
    pub streaming: Option<QualityRating>,
    pub gaming: Option<QualityRating>,
    pub video_conferencing: Option<QualityRating>,
}

/// State for the TUI display.
#[derive(Debug, Clone)]
pub struct TuiState {
    /// Current test phase
    pub phase: TestPhase,
    /// Server location info
    pub server: Option<ServerInfo>,
    /// Connection metadata
    pub connection: Option<ConnectionInfo>,
    /// Latency measurements
    pub latency: LatencyState,
    /// Download progress and results
    pub download: BandwidthState,
    /// Upload progress and results
    pub upload: BandwidthState,
    /// Quality scores
    pub quality_scores: QualityScores,
    /// Error message if any
    pub error: Option<ErrorInfo>,
    /// Terminal width for layout
    pub terminal_width: u16,
    /// Terminal height for layout
    pub terminal_height: u16,
    /// Whether the test is complete and waiting for user to exit
    pub waiting_for_exit: bool,
    /// Timestamp when test started (for graph x-axis)
    pub test_start_time: std::time::Instant,
    /// Whether a retest has been requested
    pub retest_requested: bool,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            phase: TestPhase::Initializing,
            server: None,
            connection: None,
            latency: LatencyState::default(),
            download: BandwidthState::default(),
            upload: BandwidthState::default(),
            quality_scores: QualityScores::default(),
            error: None,
            terminal_width: 80,
            terminal_height: 24,
            waiting_for_exit: false,
            test_start_time: std::time::Instant::now(),
            retest_requested: false,
        }
    }
}

impl TuiState {
    /// Create a new TuiState with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set connection metadata for display.
    pub fn set_metadata(
        &mut self,
        server: ServerInfo,
        connection: ConnectionInfo,
    ) {
        self.server = Some(server);
        self.connection = Some(connection);
    }

    /// Set an error state with optional suggestion.
    pub fn set_error(&mut self, message: String, suggestion: Option<String>) {
        self.error = Some(ErrorInfo { message, suggestion });
    }

    /// Set quality scores from scoring results.
    pub fn set_quality_scores(
        &mut self,
        streaming: &str,
        gaming: &str,
        video_conferencing: &str,
    ) {
        self.quality_scores.streaming = Some(parse_quality_rating(streaming));
        self.quality_scores.gaming = Some(parse_quality_rating(gaming));
        self.quality_scores.video_conferencing =
            Some(parse_quality_rating(video_conferencing));
    }

    /// Update state from a progress event.
    pub fn update_from_event(&mut self, event: &ProgressEvent) {
        match event {
            ProgressEvent::PhaseChange(phase) => {
                self.phase = *phase;
            }
            ProgressEvent::LatencyMeasurement { value_ms, current, total } => {
                self.latency.measurements.push(*value_ms);
                self.latency.current = *current;
                self.latency.total = *total;
            }
            ProgressEvent::BandwidthMeasurement {
                direction,
                speed_mbps,
                bytes,
                current,
                total,
            } => {
                let state = match direction {
                    BandwidthDirection::Download => &mut self.download,
                    BandwidthDirection::Upload => &mut self.upload,
                };
                state.current_speed_mbps = Some(*speed_mbps);
                state.current_bytes = *bytes;
                state.current_measurement = *current;
                state.total_measurements = *total;

                // Add to speed history for graph
                let elapsed = self.test_start_time.elapsed().as_secs_f64();
                state.speed_history.push(SpeedSample {
                    speed_mbps: *speed_mbps,
                    timestamp: elapsed,
                });
            }
            ProgressEvent::PhaseComplete(phase) => {
                match phase {
                    TestPhase::Latency => {
                        let mut measurements =
                            self.latency.measurements.clone();
                        self.latency.median_ms = median_f64(&mut measurements);
                        self.latency.jitter_ms =
                            self.latency.calculate_jitter();
                    }
                    TestPhase::Download => {
                        self.download.completed = true;
                        self.download.final_speed_mbps =
                            self.download.current_speed_mbps;
                        // Calculate 90th percentile from history
                        if !self.download.speed_history.is_empty() {
                            let mut speeds: Vec<f64> = self
                                .download
                                .speed_history
                                .iter()
                                .map(|s| s.speed_mbps)
                                .collect();
                            speeds.sort_by(|a, b| a.total_cmp(b));
                            let idx = ((speeds.len() as f64 * 0.9).ceil() as usize)
                                .saturating_sub(1)
                                .min(speeds.len() - 1);
                            self.download.percentile_90 = Some(speeds[idx]);
                        } else if let Some(speed) = self.download.final_speed_mbps {
                            // Fallback to final speed if no history
                            self.download.percentile_90 = Some(speed);
                        }
                    }
                    TestPhase::Upload => {
                        self.upload.completed = true;
                        self.upload.final_speed_mbps =
                            self.upload.current_speed_mbps;
                        // Calculate 90th percentile from history
                        if !self.upload.speed_history.is_empty() {
                            let mut speeds: Vec<f64> = self
                                .upload
                                .speed_history
                                .iter()
                                .map(|s| s.speed_mbps)
                                .collect();
                            speeds.sort_by(|a, b| a.total_cmp(b));
                            let idx = ((speeds.len() as f64 * 0.9).ceil() as usize)
                                .saturating_sub(1)
                                .min(speeds.len() - 1);
                            self.upload.percentile_90 = Some(speeds[idx]);
                        } else if let Some(speed) = self.upload.final_speed_mbps {
                            // Fallback to final speed if no history
                            self.upload.percentile_90 = Some(speed);
                        }
                    }
                    _ => {}
                }
            }
            ProgressEvent::Error(message) => {
                self.set_error(message.clone(), None);
            }
        }
    }
}

fn parse_quality_rating(s: &str) -> QualityRating {
    match s.to_lowercase().as_str() {
        "great" => QualityRating::Great,
        "good" => QualityRating::Good,
        "average" => QualityRating::Average,
        _ => QualityRating::Poor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_set_metadata() {
        let mut state = TuiState::new();
        let server = ServerInfo {
            city: "San Francisco".to_string(),
            iata: "SFO".to_string(),
        };
        let connection = ConnectionInfo {
            ip: "203.0.113.1".to_string(),
            country: "US".to_string(),
            isp: "Comcast".to_string(),
            asn: 7922,
        };

        state.set_metadata(server.clone(), connection.clone());

        assert!(state.server.is_some());
        assert!(state.connection.is_some());
        assert_eq!(state.server.as_ref().unwrap().city, "San Francisco");
        assert_eq!(state.server.as_ref().unwrap().iata, "SFO");
        assert_eq!(state.connection.as_ref().unwrap().ip, "203.0.113.1");
        assert_eq!(state.connection.as_ref().unwrap().isp, "Comcast");
    }

    #[test]
    fn test_set_error() {
        let mut state = TuiState::new();
        state.set_error(
            "Connection failed".to_string(),
            Some("Check your internet connection".to_string()),
        );

        assert!(state.error.is_some());
        let error = state.error.as_ref().unwrap();
        assert_eq!(error.message, "Connection failed");
        assert_eq!(
            error.suggestion,
            Some("Check your internet connection".to_string())
        );
    }

    #[test]
    fn test_update_from_phase_change() {
        let mut state = TuiState::new();
        assert_eq!(state.phase, TestPhase::Initializing);

        state.update_from_event(&ProgressEvent::PhaseChange(
            TestPhase::Latency,
        ));
        assert_eq!(state.phase, TestPhase::Latency);

        state.update_from_event(&ProgressEvent::PhaseChange(
            TestPhase::Download,
        ));
        assert_eq!(state.phase, TestPhase::Download);
    }

    #[test]
    fn test_update_from_latency_measurement() {
        let mut state = TuiState::new();

        state.update_from_event(&ProgressEvent::LatencyMeasurement {
            value_ms: 15.5,
            current: 1,
            total: 10,
        });

        assert_eq!(state.latency.measurements.len(), 1);
        assert_eq!(state.latency.measurements[0], 15.5);
        assert_eq!(state.latency.current, 1);
        assert_eq!(state.latency.total, 10);
    }

    #[test]
    fn test_update_from_bandwidth_measurement() {
        let mut state = TuiState::new();

        state.update_from_event(&ProgressEvent::BandwidthMeasurement {
            direction: BandwidthDirection::Download,
            speed_mbps: 95.5,
            bytes: 10_000_000,
            current: 3,
            total: 8,
        });

        assert_eq!(state.download.current_speed_mbps, Some(95.5));
        assert_eq!(state.download.current_bytes, 10_000_000);
        assert_eq!(state.download.current_measurement, 3);
        assert_eq!(state.download.total_measurements, 8);
    }

    #[test]
    fn test_update_from_phase_complete_latency() {
        let mut state = TuiState::new();

        for value in [10.0, 15.0, 12.0, 18.0, 14.0] {
            state.update_from_event(&ProgressEvent::LatencyMeasurement {
                value_ms: value,
                current: 1,
                total: 5,
            });
        }

        state.update_from_event(&ProgressEvent::PhaseComplete(
            TestPhase::Latency,
        ));

        assert!(state.latency.median_ms.is_some());
        assert_eq!(state.latency.median_ms.unwrap(), 14.0);
        assert!(state.latency.jitter_ms.is_some());
    }

    #[test]
    fn test_update_from_phase_complete_download() {
        let mut state = TuiState::new();

        state.update_from_event(&ProgressEvent::BandwidthMeasurement {
            direction: BandwidthDirection::Download,
            speed_mbps: 95.5,
            bytes: 10_000_000,
            current: 8,
            total: 8,
        });

        state.update_from_event(&ProgressEvent::PhaseComplete(
            TestPhase::Download,
        ));

        assert!(state.download.completed);
        assert_eq!(state.download.final_speed_mbps, Some(95.5));
    }

    #[test]
    fn test_update_from_error() {
        let mut state = TuiState::new();

        state.update_from_event(&ProgressEvent::Error(
            "Network timeout".to_string(),
        ));

        assert!(state.error.is_some());
        assert_eq!(state.error.as_ref().unwrap().message, "Network timeout");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn progress_percentage_monotonicity(
            total in 1usize..100,
            num_events in 1usize..50
        ) {
            let mut state = TuiState::new();
            let mut last_percentage: f64 = 0.0;

            for i in 1..=num_events.min(total) {
                state.update_from_event(&ProgressEvent::LatencyMeasurement {
                    value_ms: 10.0 + i as f64,
                    current: i,
                    total,
                });

                let current_percentage =
                    state.latency.current as f64 / state.latency.total as f64;

                prop_assert!(
                    current_percentage >= last_percentage,
                    "Progress percentage should be monotonically non-decreasing"
                );

                last_percentage = current_percentage;
            }
        }

        #[test]
        fn bandwidth_progress_monotonicity(
            total in 1usize..50,
            direction in prop_oneof![
                Just(BandwidthDirection::Download),
                Just(BandwidthDirection::Upload)
            ]
        ) {
            let mut state = TuiState::new();
            let mut last_percentage: f64 = 0.0;

            for i in 1..=total {
                state.update_from_event(&ProgressEvent::BandwidthMeasurement {
                    direction,
                    speed_mbps: 50.0 + i as f64,
                    bytes: (i as u64) * 1_000_000,
                    current: i,
                    total,
                });

                let bandwidth_state = match direction {
                    BandwidthDirection::Download => &state.download,
                    BandwidthDirection::Upload => &state.upload,
                };

                let current_percentage = bandwidth_state.current_measurement
                    as f64
                    / bandwidth_state.total_measurements as f64;

                prop_assert!(
                    current_percentage >= last_percentage,
                    "Bandwidth progress should be monotonically non-decreasing"
                );

                last_percentage = current_percentage;
            }
        }

        #[test]
        fn error_state_preservation(
            num_latency_measurements in 0usize..20,
            num_download_measurements in 0usize..10,
            num_upload_measurements in 0usize..10,
            error_message in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let mut state = TuiState::new();

            for i in 0..num_latency_measurements {
                state.update_from_event(&ProgressEvent::LatencyMeasurement {
                    value_ms: 10.0 + i as f64,
                    current: i + 1,
                    total: num_latency_measurements.max(1),
                });
            }

            for i in 0..num_download_measurements {
                state.update_from_event(&ProgressEvent::BandwidthMeasurement {
                    direction: BandwidthDirection::Download,
                    speed_mbps: 50.0 + i as f64,
                    bytes: (i as u64 + 1) * 1_000_000,
                    current: i + 1,
                    total: num_download_measurements.max(1),
                });
            }

            for i in 0..num_upload_measurements {
                state.update_from_event(&ProgressEvent::BandwidthMeasurement {
                    direction: BandwidthDirection::Upload,
                    speed_mbps: 30.0 + i as f64,
                    bytes: (i as u64 + 1) * 500_000,
                    current: i + 1,
                    total: num_upload_measurements.max(1),
                });
            }

            let latency_count_before = state.latency.measurements.len();
            let download_measurement_before = state.download.current_measurement;
            let upload_measurement_before = state.upload.current_measurement;

            state.update_from_event(&ProgressEvent::Error(error_message.clone()));

            prop_assert!(state.error.is_some());
            prop_assert_eq!(
                &state.error.as_ref().unwrap().message,
                &error_message
            );
            prop_assert_eq!(
                state.latency.measurements.len(),
                latency_count_before
            );
            prop_assert_eq!(
                state.download.current_measurement,
                download_measurement_before
            );
            prop_assert_eq!(
                state.upload.current_measurement,
                upload_measurement_before
            );
        }
    }
}

impl TuiState {
    /// Reset state for a retest, preserving server/connection info.
    pub fn reset_for_retest(&mut self) {
        self.phase = TestPhase::Initializing;
        self.latency = LatencyState::default();
        self.download = BandwidthState::default();
        self.upload = BandwidthState::default();
        self.quality_scores = QualityScores::default();
        self.error = None;
        self.waiting_for_exit = false;
        self.test_start_time = std::time::Instant::now();
        self.retest_requested = false;
    }
}
