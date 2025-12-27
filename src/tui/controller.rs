//! TUI controller for managing the display lifecycle.
//!
//! The TuiController manages the TUI lifecycle, including initialization,
//! rendering, and cleanup. It also provides a progress callback for
//! the test engine to emit events.

use std::io::{self, Stdout};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::{
    cursor,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
        KeyEventKind,
    },
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

        // Handle any pending resize events before rendering
        self.handle_pending_events()?;

        if let Some(ref mut terminal) = self.terminal {
            // Update terminal width in case of resize
            let size = terminal.size()?;
            if let Ok(mut state) = self.state.lock() {
                state.terminal_width = size.width;
            }

            // Clone state for rendering to avoid holding lock during draw
            let state = {
                let state_guard = self.state.lock().map_err(|e| {
                    Box::new(io::Error::other(format!(
                        "Failed to lock state: {}",
                        e
                    ))) as Box<dyn std::error::Error>
                })?;
                state_guard.clone()
            };

            terminal.draw(|frame| {
                render_frame(frame, &state);
            })?;
        }

        Ok(())
    }

    /// Handle pending terminal events (resize, etc.).
    ///
    /// This method polls for terminal events without blocking and handles
    /// resize events by updating the terminal width in state. This allows
    /// the TUI to adapt to terminal size changes in real-time.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if event handling fails.
    ///
    /// # Requirements
    /// _Requirements: 8.1, 8.4_
    pub fn handle_pending_events(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        // Poll for events with zero timeout (non-blocking)
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Resize(width, _height) => {
                    // Update terminal width in state
                    if let Ok(mut state) = self.state.lock() {
                        state.terminal_width = width;
                    }
                }
                Event::Key(key_event) => {
                    // Handle key events (e.g., Ctrl+C is handled by signal
                    // handler) Only handle key press events, not release
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                // User wants to quit - this will be handled
                                // by the main loop checking shutdown flag
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    // Ignore other events (mouse, focus, paste, etc.)
                }
            }
        }

        Ok(())
    }

    /// Force a re-render after a resize event.
    ///
    /// This method should be called when a resize event is detected to
    /// immediately update the display with the new dimensions.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if re-rendering fails.
    ///
    /// # Requirements
    /// _Requirements: 8.1, 8.4_
    #[allow(dead_code)]
    pub fn handle_resize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        if let Some(ref mut terminal) = self.terminal {
            // Get new terminal size
            let size = terminal.size()?;

            // Update terminal width in state
            if let Ok(mut state) = self.state.lock() {
                state.terminal_width = size.width;
            }

            // Force a re-render with new dimensions
            self.render()?;
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

    /// Get partial results collected so far.
    ///
    /// This is useful for printing partial results when the test is
    /// interrupted by the user.
    ///
    /// # Returns
    /// A summary of partial results, or None if no results are available.
    pub fn get_partial_results(&self) -> Option<PartialResults> {
        let state = self.state.lock().ok()?;

        // Only return partial results if we have some data
        if state.latency.measurements.is_empty()
            && state.download.current_speed_mbps.is_none()
            && state.upload.current_speed_mbps.is_none()
        {
            return None;
        }

        Some(PartialResults {
            server: state.server.clone(),
            connection: state.connection.clone(),
            latency_median_ms: state.latency.median_ms,
            latency_jitter_ms: state.latency.jitter_ms,
            latency_measurements: state.latency.measurements.len(),
            download_speed_mbps: state
                .download
                .final_speed_mbps
                .or(state.download.current_speed_mbps),
            download_completed: state.download.completed,
            upload_speed_mbps: state
                .upload
                .final_speed_mbps
                .or(state.upload.current_speed_mbps),
            upload_completed: state.upload.completed,
            phase: state.phase,
        })
    }
}

/// Partial results collected during an interrupted test.
#[derive(Debug, Clone)]
pub struct PartialResults {
    /// Server location info
    #[allow(dead_code)]
    pub server: Option<ServerInfo>,
    /// Connection metadata
    #[allow(dead_code)]
    pub connection: Option<ConnectionInfo>,
    /// Median latency in ms (if calculated)
    pub latency_median_ms: Option<f64>,
    /// Jitter in ms (if calculated)
    pub latency_jitter_ms: Option<f64>,
    /// Number of latency measurements collected
    #[allow(dead_code)]
    pub latency_measurements: usize,
    /// Download speed in Mbps (final or current)
    pub download_speed_mbps: Option<f64>,
    /// Whether download phase completed
    pub download_completed: bool,
    /// Upload speed in Mbps (final or current)
    pub upload_speed_mbps: Option<f64>,
    /// Whether upload phase completed
    pub upload_completed: bool,
    /// Current test phase when interrupted
    pub phase: super::progress::TestPhase,
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

    #[test]
    fn test_handle_pending_events_noop_for_non_tui_modes() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        assert!(controller.handle_pending_events().is_ok());

        let mut controller = TuiController::new(DisplayMode::Json).unwrap();
        assert!(controller.handle_pending_events().is_ok());
    }

    #[test]
    fn test_handle_resize_noop_for_non_tui_modes() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        assert!(controller.handle_resize().is_ok());

        let mut controller = TuiController::new(DisplayMode::Json).unwrap();
        assert!(controller.handle_resize().is_ok());
    }

    #[test]
    fn test_terminal_width_default() {
        let controller = TuiController::new(DisplayMode::Silent).unwrap();
        let state = controller.state.lock().unwrap();
        // Default terminal width should be 80
        assert_eq!(state.terminal_width, 80);
    }

    #[test]
    fn test_terminal_width_can_be_updated() {
        let controller = TuiController::new(DisplayMode::Silent).unwrap();

        // Manually update terminal width (simulating resize)
        {
            let mut state = controller.state.lock().unwrap();
            state.terminal_width = 120;
        }

        let state = controller.state.lock().unwrap();
        assert_eq!(state.terminal_width, 120);
    }

    #[test]
    fn test_minimal_mode_triggered_by_narrow_width() {
        use crate::tui::renderer::is_minimal_mode;

        let controller = TuiController::new(DisplayMode::Silent).unwrap();

        // Set narrow width (below threshold of 60)
        {
            let mut state = controller.state.lock().unwrap();
            state.terminal_width = 50;
        }

        let state = controller.state.lock().unwrap();
        assert!(is_minimal_mode(state.terminal_width));
    }

    #[test]
    fn test_normal_mode_for_wide_terminal() {
        use crate::tui::renderer::is_minimal_mode;

        let controller = TuiController::new(DisplayMode::Silent).unwrap();

        // Set wide width (at or above threshold of 60)
        {
            let mut state = controller.state.lock().unwrap();
            state.terminal_width = 80;
        }

        let state = controller.state.lock().unwrap();
        assert!(!is_minimal_mode(state.terminal_width));
    }
}
