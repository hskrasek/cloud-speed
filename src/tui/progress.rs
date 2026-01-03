//! Progress event types and callback interface.
//!
//! Defines the events emitted by the test engine to update the TUI
//! and the callback trait for receiving these events.

/// Test phases during speed test execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestPhase {
    /// Initializing the test
    Initializing,
    /// Running latency tests
    Latency,
    /// Running download tests
    Download,
    /// Running upload tests
    Upload,
    /// All tests complete
    Complete,
}

/// Direction of bandwidth measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthDirection {
    /// Download test
    Download,
    /// Upload test
    Upload,
}

/// Progress events emitted during test execution.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Test phase has changed
    PhaseChange(TestPhase),
    /// Latency measurement completed
    LatencyMeasurement {
        /// Measured latency in milliseconds
        value_ms: f64,
        /// Current measurement number (1-indexed)
        current: usize,
        /// Total number of measurements
        total: usize,
    },
    /// Bandwidth measurement completed
    BandwidthMeasurement {
        /// Direction of the measurement
        direction: BandwidthDirection,
        /// Measured speed in Mbps
        speed_mbps: f64,
        /// Number of bytes transferred
        bytes: u64,
        /// Current measurement number (1-indexed)
        current: usize,
        /// Total number of measurements
        total: usize,
    },
    /// Phase completed with results
    PhaseComplete(TestPhase),
    /// Error occurred
    #[allow(dead_code)]
    Error(String),
}

/// Callback interface for progress updates.
///
/// Implementations must be non-blocking to avoid affecting
/// measurement accuracy.
pub trait ProgressCallback: Send + Sync {
    /// Called when a progress event occurs.
    fn on_progress(&self, event: ProgressEvent);
}
