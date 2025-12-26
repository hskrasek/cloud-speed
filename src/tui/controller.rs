//! TUI controller for managing the display lifecycle.
//!
//! The TuiController manages the TUI lifecycle, including initialization,
//! rendering, and cleanup. It also provides a progress callback for
//! the test engine to emit events.

use std::io::{self, Stdout};
use std::sync::{Arc, Mutex};

use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};

use super::display_mode::DisplayMode;
use super::progress::{ProgressCallback, ProgressEvent};
use super::renderer::render_frame;
use super::state::{ConnectionInfo, ServerInfo, TuiState};
use crate::results::SpeedTestResults;

/// Controller for the TUI display.
///
/// Manages the TUI lifecycle including initialization, rendering,
/// and cleanup. Provides a progress callback for the test engine.
pub struct TuiController {
    /// Current display mode
    mode: DisplayMode,
    /// Shared state for the TUI
    state: Arc<Mutex<TuiState>>,
    /// Terminal instance (only present in TUI mode)
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
    /// Whether the terminal has been initialized
    initialized: bool,
}

impl TuiController {
    /// Create a new TUI controller.
    ///
    /// # Arguments
    /// * `mode` - The display mode to use
    ///
    /// # Returns
    /// A new TuiController instance, or an error if initialization fails.
    ///
    /// # Requirements
    /// _Requirements: 1.4_
    pub fn new(mode: DisplayMode) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            mode,
            state: Arc::new(Mutex::new(TuiState::new())),
            terminal: None,
            initialized: false,
        })
    }

    /// Get current display mode.
    ///
    /// # Returns
    /// The current DisplayMode.
    ///
    /// # Requirements
    /// _Requirements: 1.4_
    pub fn mode(&self) -> DisplayMode {
        self.mode
    }

    /// Initialize the TUI.
    ///
    /// In TUI mode, this enters the alternate screen and hides the cursor.
    /// In other modes, this is a no-op.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if terminal initialization fails.
    ///
    /// # Requirements
    /// _Requirements: 8.2, 8.3_
    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        // Enable raw mode for terminal control
        enable_raw_mode()?;

        // Get stdout and enter alternate screen
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;

        // Create terminal backend
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        self.terminal = Some(terminal);
        self.initialized = true;

        // Update terminal width in state
        if let Some(ref terminal) = self.terminal {
            let size = terminal.size()?;
            if let Ok(mut state) = self.state.lock() {
                state.terminal_width = size.width;
            }
        }

        Ok(())
    }

    /// Clean up and restore terminal state.
    ///
    /// Restores the terminal to its original state by:
    /// - Leaving the alternate screen
    /// - Showing the cursor
    /// - Disabling raw mode
    ///
    /// # Returns
    /// Ok(()) on success, or an error if cleanup fails.
    ///
    /// # Requirements
    /// _Requirements: 8.2, 8.3_
    pub fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.initialized {
            return Ok(());
        }

        // Restore terminal state
        if let Some(ref mut terminal) = self.terminal {
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                cursor::Show
            )?;
        }

        // Disable raw mode
        disable_raw_mode()?;

        self.initialized = false;
        self.terminal = None;

        Ok(())
    }

    /// Set connection metadata for display.
    ///
    /// # Arguments
    /// * `server` - Server location information
    /// * `connection` - Connection metadata (IP, ISP, etc.)
    ///
    /// # Requirements
    /// _Requirements: 2.1, 2.2, 2.3_
    pub fn set_metadata(
        &mut self,
        server: ServerInfo,
        connection: ConnectionInfo,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.set_metadata(server, connection);
        }
    }

    /// Set an error state for display.
    ///
    /// This displays the error message prominently in the TUI with red styling.
    /// Any partial results collected before the error are preserved.
    ///
    /// # Arguments
    /// * `message` - The error message to display
    /// * `suggestion` - Optional suggestion for resolution
    ///
    /// # Requirements
    /// _Requirements: 7.1, 7.2, 7.3, 7.4_
    pub fn set_error(&mut self, message: String, suggestion: Option<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.set_error(message, suggestion);
        }
    }

    /// Render the current state to the terminal.
    ///
    /// In TUI mode, this renders the full TUI. In other modes, this is a
    /// no-op.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if rendering fails.
    ///
    /// # Requirements
    /// _Requirements: 4.3_
    pub fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        if let Some(ref mut terminal) = self.terminal {
            // Update terminal width in case of resize
            let size = terminal.size()?;
            if let Ok(mut state) = self.state.lock() {
                state.terminal_width = size.width;
            }

            // Clone state for rendering to avoid holding lock during draw
            let state = {
                let state_guard = self.state.lock().map_err(|e| {
                    Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to lock state: {}", e),
                    )) as Box<dyn std::error::Error>
                })?;
                state_guard.clone()
            };

            terminal.draw(|frame| {
                render_frame(frame, &state);
            })?;
        }

        Ok(())
    }

    /// Display final results.
    ///
    /// Updates the TUI state with final results and renders them.
    /// In JSON mode, this outputs the results as JSON.
    ///
    /// # Arguments
    /// * `results` - The speed test results to display
    ///
    /// # Returns
    /// Ok(()) on success, or an error if display fails.
    ///
    /// # Requirements
    /// _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_
    pub fn show_results(
        &mut self,
        results: &SpeedTestResults,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Update state with final results
        if let Ok(mut state) = self.state.lock() {
            // Update latency results
            state.latency.median_ms = Some(results.latency.idle_ms);
            state.latency.jitter_ms = results.latency.idle_jitter_ms;

            // Update download results
            state.download.final_speed_mbps = Some(results.download.speed_mbps);
            state.download.completed = true;

            // Update upload results
            state.upload.final_speed_mbps = Some(results.upload.speed_mbps);
            state.upload.completed = true;

            // Set phase to complete
            state.phase = super::progress::TestPhase::Complete;
        }

        // Render the final state
        self.render()?;

        Ok(())
    }

    /// Get a progress callback for the test engine.
    ///
    /// Returns an Arc-wrapped callback that can be passed to the test engine.
    /// The callback updates the shared TUI state in a non-blocking manner.
    ///
    /// # Returns
    /// An Arc<dyn ProgressCallback> that can be used by the test engine.
    ///
    /// # Requirements
    /// _Requirements: 9.1, 9.5_
    pub fn progress_callback(&self) -> Arc<dyn ProgressCallback> {
        Arc::new(TuiProgressCallback {
            state: Arc::clone(&self.state),
        })
    }

    /// Get a reference to the shared state.
    ///
    /// This is primarily useful for testing.
    #[cfg(test)]
    pub fn state(&self) -> Arc<Mutex<TuiState>> {
        Arc::clone(&self.state)
    }
}

impl Drop for TuiController {
    /// Automatically clean up terminal state when the controller is dropped.
    ///
    /// This ensures the terminal is restored even if cleanup() is not
    /// explicitly called.
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Progress callback implementation for the TUI.
///
/// This struct implements the ProgressCallback trait and updates
/// the shared TUI state when progress events are received.
struct TuiProgressCallback {
    /// Shared state with the TuiController
    state: Arc<Mutex<TuiState>>,
}

impl ProgressCallback for TuiProgressCallback {
    /// Handle a progress event by updating the TUI state.
    ///
    /// This method is non-blocking to avoid affecting measurement accuracy.
    ///
    /// # Arguments
    /// * `event` - The progress event to process
    fn on_progress(&self, event: ProgressEvent) {
        // Non-blocking: try to acquire lock, skip if unavailable
        if let Ok(mut state) = self.state.try_lock() {
            state.update_from_event(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::progress::{BandwidthDirection, TestPhase};

    #[test]
    fn test_new_controller() {
        let controller = TuiController::new(DisplayMode::Silent);
        assert!(controller.is_ok());
        let controller = controller.unwrap();
        assert_eq!(controller.mode(), DisplayMode::Silent);
    }

    #[test]
    fn test_mode_returns_correct_mode() {
        let controller = TuiController::new(DisplayMode::Json).unwrap();
        assert_eq!(controller.mode(), DisplayMode::Json);

        let controller = TuiController::new(DisplayMode::Silent).unwrap();
        assert_eq!(controller.mode(), DisplayMode::Silent);

        let controller = TuiController::new(DisplayMode::Tui).unwrap();
        assert_eq!(controller.mode(), DisplayMode::Tui);
    }

    #[test]
    fn test_set_metadata() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();

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

        controller.set_metadata(server, connection);

        let state = controller.state.lock().unwrap();
        assert!(state.server.is_some());
        assert!(state.connection.is_some());
        assert_eq!(state.server.as_ref().unwrap().city, "San Francisco");
        assert_eq!(state.connection.as_ref().unwrap().isp, "Comcast");
    }

    #[test]
    fn test_progress_callback_updates_state() {
        let controller = TuiController::new(DisplayMode::Silent).unwrap();
        let callback = controller.progress_callback();

        // Send a phase change event
        callback.on_progress(ProgressEvent::PhaseChange(TestPhase::Latency));

        let state = controller.state.lock().unwrap();
        assert_eq!(state.phase, TestPhase::Latency);
    }

    #[test]
    fn test_progress_callback_latency_measurement() {
        let controller = TuiController::new(DisplayMode::Silent).unwrap();
        let callback = controller.progress_callback();

        callback.on_progress(ProgressEvent::LatencyMeasurement {
            value_ms: 15.5,
            current: 1,
            total: 10,
        });

        let state = controller.state.lock().unwrap();
        assert_eq!(state.latency.measurements.len(), 1);
        assert_eq!(state.latency.measurements[0], 15.5);
        assert_eq!(state.latency.current, 1);
        assert_eq!(state.latency.total, 10);
    }

    #[test]
    fn test_progress_callback_bandwidth_measurement() {
        let controller = TuiController::new(DisplayMode::Silent).unwrap();
        let callback = controller.progress_callback();

        callback.on_progress(ProgressEvent::BandwidthMeasurement {
            direction: BandwidthDirection::Download,
            speed_mbps: 95.5,
            bytes: 10_000_000,
            current: 3,
            total: 8,
        });

        let state = controller.state.lock().unwrap();
        assert_eq!(state.download.current_speed_mbps, Some(95.5));
        assert_eq!(state.download.current_bytes, 10_000_000);
        assert_eq!(state.download.current_measurement, 3);
        assert_eq!(state.download.total_measurements, 8);
    }

    #[test]
    fn test_init_noop_for_non_tui_modes() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        assert!(controller.init().is_ok());
        assert!(controller.terminal.is_none());

        let mut controller = TuiController::new(DisplayMode::Json).unwrap();
        assert!(controller.init().is_ok());
        assert!(controller.terminal.is_none());
    }

    #[test]
    fn test_render_noop_for_non_tui_modes() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        assert!(controller.render().is_ok());

        let mut controller = TuiController::new(DisplayMode::Json).unwrap();
        assert!(controller.render().is_ok());
    }

    #[test]
    fn test_cleanup_noop_when_not_initialized() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        assert!(controller.cleanup().is_ok());
    }
}
