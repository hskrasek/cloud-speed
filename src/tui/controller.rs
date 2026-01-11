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

/// Result of waiting for user input after test completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// User wants to exit
    Exit,
    /// User wants to retest
    Retest,
}

/// Controller for the TUI display.
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
    pub fn new(mode: DisplayMode) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            mode,
            state: Arc::new(Mutex::new(TuiState::new())),
            terminal: None,
            initialized: false,
        })
    }

    /// Get current display mode.
    pub fn mode(&self) -> DisplayMode {
        self.mode
    }

    /// Initialize the TUI.
    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        self.terminal = Some(terminal);
        self.initialized = true;

        if let Some(ref terminal) = self.terminal {
            let size = terminal.size()?;
            if let Ok(mut state) = self.state.lock() {
                state.terminal_width = size.width;
                state.terminal_height = size.height;
            }
        }

        Ok(())
    }

    /// Clean up and restore terminal state.
    pub fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.initialized {
            return Ok(());
        }

        if let Some(ref mut terminal) = self.terminal {
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                cursor::Show
            )?;
        }

        disable_raw_mode()?;

        self.initialized = false;
        self.terminal = None;

        Ok(())
    }

    /// Set connection metadata for display.
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
    pub fn set_error(&mut self, message: String, suggestion: Option<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.set_error(message, suggestion);
        }
    }

    /// Set quality scores for display.
    pub fn set_quality_scores(
        &mut self,
        streaming: &str,
        gaming: &str,
        video_conferencing: &str,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.set_quality_scores(streaming, gaming, video_conferencing);
        }
    }

    /// Set loaded latency values.
    pub fn set_loaded_latency(
        &mut self,
        down_ms: Option<f64>,
        down_jitter_ms: Option<f64>,
        up_ms: Option<f64>,
        up_jitter_ms: Option<f64>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.latency.loaded_down_ms = down_ms;
            state.latency.loaded_down_jitter_ms = down_jitter_ms;
            state.latency.loaded_up_ms = up_ms;
            state.latency.loaded_up_jitter_ms = up_jitter_ms;
        }
    }

    /// Render the current state to the terminal.
    pub fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        self.handle_pending_events()?;

        if let Some(ref mut terminal) = self.terminal {
            let size = terminal.size()?;
            if let Ok(mut state) = self.state.lock() {
                state.terminal_width = size.width;
                state.terminal_height = size.height;
            }

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
    pub fn handle_pending_events(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(());
        }

        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Resize(width, height) => {
                    if let Ok(mut state) = self.state.lock() {
                        state.terminal_width = width;
                        state.terminal_height = height;
                    }
                }
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                // Handled by wait_for_exit
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Wait for user to press 'q' or Esc to exit, or 'r' to retest.
    /// Returns Ok(WaitResult::Exit) if user wants to exit,
    /// Ok(WaitResult::Retest) if user wants to retest,
    /// or Err if interrupted.
    pub fn wait_for_exit(
        &mut self,
        shutdown_flag: &std::sync::atomic::AtomicBool,
    ) -> Result<WaitResult, Box<dyn std::error::Error>> {
        if self.mode != DisplayMode::Tui {
            return Ok(WaitResult::Exit);
        }

        // Set waiting state
        if let Ok(mut state) = self.state.lock() {
            state.waiting_for_exit = true;
        }

        // Render with exit prompt
        self.render()?;

        loop {
            // Check for shutdown signal
            if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("Interrupted".into());
            }

            // Poll for events with timeout
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                return Ok(WaitResult::Exit);
                            }
                            KeyCode::Char('r') => {
                                // Reset state for retest
                                if let Ok(mut state) = self.state.lock() {
                                    state.reset_for_retest();
                                }
                                return Ok(WaitResult::Retest);
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Re-render periodically to handle resize
            self.render()?;
        }
    }

    /// Display final results.
    pub fn show_results(
        &mut self,
        results: &SpeedTestResults,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut state) = self.state.lock() {
            state.latency.median_ms = Some(results.latency.idle_ms);
            state.latency.jitter_ms = results.latency.idle_jitter_ms;
            state.latency.loaded_down_ms = results.latency.loaded_down_ms;
            state.latency.loaded_down_jitter_ms =
                results.latency.loaded_down_jitter_ms;
            state.latency.loaded_up_ms = results.latency.loaded_up_ms;
            state.latency.loaded_up_jitter_ms =
                results.latency.loaded_up_jitter_ms;

            state.download.final_speed_mbps =
                Some(results.download.speed_mbps);
            state.download.completed = true;

            state.upload.final_speed_mbps = Some(results.upload.speed_mbps);
            state.upload.completed = true;

            state.phase = super::progress::TestPhase::Complete;
        }

        self.render()?;

        Ok(())
    }

    /// Get a progress callback for the test engine.
    pub fn progress_callback(&self) -> Arc<dyn ProgressCallback> {
        Arc::new(TuiProgressCallback { state: Arc::clone(&self.state) })
    }

    /// Get partial results collected so far.
    pub fn get_partial_results(&self) -> Option<PartialResults> {
        let state = self.state.lock().ok()?;

        if state.latency.measurements.is_empty()
            && state.download.current_speed_mbps.is_none()
            && state.upload.current_speed_mbps.is_none()
        {
            return None;
        }

        Some(PartialResults {
            latency_median_ms: state.latency.median_ms,
            latency_jitter_ms: state.latency.jitter_ms,
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
    /// Median latency in ms (if calculated)
    pub latency_median_ms: Option<f64>,
    /// Jitter in ms (if calculated)
    pub latency_jitter_ms: Option<f64>,
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
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Progress callback implementation for the TUI.
struct TuiProgressCallback {
    state: Arc<Mutex<TuiState>>,
}

impl ProgressCallback for TuiProgressCallback {
    fn on_progress(&self, event: ProgressEvent) {
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
    fn test_terminal_width_default() {
        let controller = TuiController::new(DisplayMode::Silent).unwrap();
        let state = controller.state.lock().unwrap();
        assert_eq!(state.terminal_width, 80);
    }

    #[test]
    fn test_set_quality_scores() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        controller.set_quality_scores("great", "good", "average");

        let state = controller.state.lock().unwrap();
        assert!(state.quality_scores.streaming.is_some());
        assert!(state.quality_scores.gaming.is_some());
        assert!(state.quality_scores.video_conferencing.is_some());
    }

    #[test]
    fn test_set_loaded_latency() {
        let mut controller = TuiController::new(DisplayMode::Silent).unwrap();
        controller.set_loaded_latency(
            Some(25.0),
            Some(5.0),
            Some(30.0),
            Some(6.0),
        );

        let state = controller.state.lock().unwrap();
        assert_eq!(state.latency.loaded_down_ms, Some(25.0));
        assert_eq!(state.latency.loaded_down_jitter_ms, Some(5.0));
        assert_eq!(state.latency.loaded_up_ms, Some(30.0));
        assert_eq!(state.latency.loaded_up_jitter_ms, Some(6.0));
    }
}
