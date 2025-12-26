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
    /// Error message if any
    pub error: Option<ErrorInfo>,
    /// Terminal width for layout
    pub terminal_width: u16,
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
            error: None,
            terminal_width: 80,
        }
    }
}

impl TuiState {
    /// Create a new TuiState with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set connection metadata for display.
    ///
    /// # Arguments
    /// * `server` - Server location information
    /// * `connection` - Connection metadata (IP, ISP, etc.)
    pub fn set_metadata(
        &mut self,
        server: ServerInfo,
        connection: ConnectionInfo,
    ) {
        self.server = Some(server);
        self.connection = Some(connection);
    }

    /// Set an error state with optional suggestion.
    ///
    /// This preserves any partial results collected before the error.
    ///
    /// # Arguments
    /// * `message` - The error message to display
    /// * `suggestion` - Optional suggestion for resolution
    pub fn set_error(&mut self, message: String, suggestion: Option<String>) {
        self.error = Some(ErrorInfo {
            message,
            suggestion,
        });
    }

    /// Update state from a progress event.
    ///
    /// This method processes progress events emitted by the test engine
    /// and updates the appropriate state fields.
    ///
    /// # Arguments
    /// * `event` - The progress event to process
    pub fn update_from_event(&mut self, event: &ProgressEvent) {
        match event {
            ProgressEvent::PhaseChange(phase) => {
                self.phase = *phase;
            }
            ProgressEvent::LatencyMeasurement {
                value_ms,
                current,
                total,
            } => {
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
            }
            ProgressEvent::PhaseComplete(phase) => {
                match phase {
                    TestPhase::Latency => {
                        // Calculate median and jitter when latency phase
                        // completes
                        let mut measurements =
                            self.latency.measurements.clone();
                        self.latency.median_ms = median_f64(&mut measurements);
                        self.latency.jitter_ms =
                            self.latency.calculate_jitter();
                    }
                    TestPhase::Download => {
                        self.download.completed = true;
                        // Final speed is the last measured speed
                        self.download.final_speed_mbps =
                            self.download.current_speed_mbps;
                    }
                    TestPhase::Upload => {
                        self.upload.completed = true;
                        // Final speed is the last measured speed
                        self.upload.final_speed_mbps =
                            self.upload.current_speed_mbps;
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


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Unit tests for state update methods
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

        state.update_from_event(&ProgressEvent::PhaseChange(TestPhase::Latency));
        assert_eq!(state.phase, TestPhase::Latency);

        state
            .update_from_event(&ProgressEvent::PhaseChange(TestPhase::Download));
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

        // Add some latency measurements
        for value in [10.0, 15.0, 12.0, 18.0, 14.0] {
            state.update_from_event(&ProgressEvent::LatencyMeasurement {
                value_ms: value,
                current: 1,
                total: 5,
            });
        }

        // Complete the latency phase
        state.update_from_event(&ProgressEvent::PhaseComplete(TestPhase::Latency));

        // Median should be calculated (14.0 for [10, 12, 14, 15, 18])
        assert!(state.latency.median_ms.is_some());
        assert_eq!(state.latency.median_ms.unwrap(), 14.0);

        // Jitter should be calculated
        assert!(state.latency.jitter_ms.is_some());
    }

    #[test]
    fn test_update_from_phase_complete_download() {
        let mut state = TuiState::new();

        // Add a bandwidth measurement
        state.update_from_event(&ProgressEvent::BandwidthMeasurement {
            direction: BandwidthDirection::Download,
            speed_mbps: 95.5,
            bytes: 10_000_000,
            current: 8,
            total: 8,
        });

        // Complete the download phase
        state
            .update_from_event(&ProgressEvent::PhaseComplete(TestPhase::Download));

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

    // Feature: tui-progress-display, Property 3: Progress Percentage Monotonicity
    // Validates: Requirements 3.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any sequence of ProgressEvents for a single test phase,
        /// the completion percentage (current/total) SHALL be monotonically
        /// non-decreasing.
        #[test]
        fn progress_percentage_monotonicity(
            total in 1usize..100,
            num_events in 1usize..50
        ) {
            let mut state = TuiState::new();
            let mut last_percentage: f64 = 0.0;

            // Generate a sequence of events with increasing current values
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
                    "Progress percentage should be monotonically non-decreasing: \
                     {} >= {} (current={}, total={})",
                    current_percentage,
                    last_percentage,
                    state.latency.current,
                    state.latency.total
                );

                last_percentage = current_percentage;
            }
        }

        /// Property: For bandwidth measurements, progress is monotonically
        /// non-decreasing within a phase.
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
                    "Bandwidth progress should be monotonically non-decreasing: \
                     {} >= {}",
                    current_percentage,
                    last_percentage
                );

                last_percentage = current_percentage;
            }
        }
    }

    // Feature: tui-progress-display, Property 10: Error State Preservation
    // Validates: Requirements 7.1, 7.3, 7.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any TuiState that receives an Error event after
        /// collecting N measurements, the state SHALL:
        /// - Have error set to Some(ErrorInfo)
        /// - Preserve all N previously collected measurements
        #[test]
        fn error_state_preservation(
            num_latency_measurements in 0usize..20,
            num_download_measurements in 0usize..10,
            num_upload_measurements in 0usize..10,
            error_message in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let mut state = TuiState::new();

            // Collect latency measurements
            for i in 0..num_latency_measurements {
                state.update_from_event(&ProgressEvent::LatencyMeasurement {
                    value_ms: 10.0 + i as f64,
                    current: i + 1,
                    total: num_latency_measurements.max(1),
                });
            }

            // Collect download measurements
            for i in 0..num_download_measurements {
                state.update_from_event(&ProgressEvent::BandwidthMeasurement {
                    direction: BandwidthDirection::Download,
                    speed_mbps: 50.0 + i as f64,
                    bytes: (i as u64 + 1) * 1_000_000,
                    current: i + 1,
                    total: num_download_measurements.max(1),
                });
            }

            // Collect upload measurements
            for i in 0..num_upload_measurements {
                state.update_from_event(&ProgressEvent::BandwidthMeasurement {
                    direction: BandwidthDirection::Upload,
                    speed_mbps: 30.0 + i as f64,
                    bytes: (i as u64 + 1) * 500_000,
                    current: i + 1,
                    total: num_upload_measurements.max(1),
                });
            }

            // Record state before error
            let latency_count_before = state.latency.measurements.len();
            let download_measurement_before = state.download.current_measurement;
            let upload_measurement_before = state.upload.current_measurement;

            // Trigger error
            state.update_from_event(&ProgressEvent::Error(error_message.clone()));

            // Verify error is set
            prop_assert!(
                state.error.is_some(),
                "Error should be set after Error event"
            );
            prop_assert_eq!(
                &state.error.as_ref().unwrap().message,
                &error_message,
                "Error message should match"
            );

            // Verify measurements are preserved
            prop_assert_eq!(
                state.latency.measurements.len(),
                latency_count_before,
                "Latency measurements should be preserved after error"
            );
            prop_assert_eq!(
                state.download.current_measurement,
                download_measurement_before,
                "Download measurement count should be preserved after error"
            );
            prop_assert_eq!(
                state.upload.current_measurement,
                upload_measurement_before,
                "Upload measurement count should be preserved after error"
            );
        }

        /// Property: Error state preserves partial results including any
        /// calculated values (median, jitter, final speeds).
        #[test]
        fn error_preserves_calculated_values(
            latency_values in prop::collection::vec(1.0f64..100.0, 2..10),
            download_speed in 10.0f64..200.0,
            error_message in "[a-zA-Z0-9 ]{1,30}"
        ) {
            let mut state = TuiState::new();

            // Add latency measurements
            let total = latency_values.len();
            for (i, value) in latency_values.iter().enumerate() {
                state.update_from_event(&ProgressEvent::LatencyMeasurement {
                    value_ms: *value,
                    current: i + 1,
                    total,
                });
            }

            // Complete latency phase to calculate median/jitter
            state.update_from_event(&ProgressEvent::PhaseComplete(
                TestPhase::Latency,
            ));

            // Add download measurement
            state.update_from_event(&ProgressEvent::BandwidthMeasurement {
                direction: BandwidthDirection::Download,
                speed_mbps: download_speed,
                bytes: 10_000_000,
                current: 1,
                total: 1,
            });

            // Record calculated values before error
            let median_before = state.latency.median_ms;
            let jitter_before = state.latency.jitter_ms;
            let download_speed_before = state.download.current_speed_mbps;

            // Trigger error
            state.update_from_event(&ProgressEvent::Error(error_message));

            // Verify calculated values are preserved
            prop_assert_eq!(
                state.latency.median_ms,
                median_before,
                "Median should be preserved after error"
            );
            prop_assert_eq!(
                state.latency.jitter_ms,
                jitter_before,
                "Jitter should be preserved after error"
            );
            prop_assert_eq!(
                state.download.current_speed_mbps,
                download_speed_before,
                "Download speed should be preserved after error"
            );
        }
    }
}
