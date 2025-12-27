# Implementation Plan: TUI Progress Display

## Overview

This implementation plan breaks down the TUI progress display feature into incremental tasks. Each task builds on previous work, starting with core types and progressing to full integration. Property-based tests are included as optional sub-tasks to validate correctness properties.

## Tasks

- [x] 1. Add dependencies and create module structure
  - Add `ratatui = "0.29"` and `crossterm = "0.28"` to Cargo.toml
  - Create `src/tui/mod.rs` with module declarations
  - Create placeholder files for each submodule
  - _Requirements: N/A (setup)_

- [x] 2. Implement DisplayMode and detection logic
  - [x] 2.1 Create `src/tui/display_mode.rs` with DisplayMode enum
    - Implement `DisplayMode::Tui`, `DisplayMode::Silent`, `DisplayMode::Json` variants
    - Implement `DisplayMode::detect(json_flag: bool, is_tty: bool) -> Self`
    - _Requirements: 1.1, 1.2, 1.3, 1.4_
  - [x] 2.2 Write property test for display mode selection
    - **Property 1: Display Mode Selection**
    - **Validates: Requirements 1.1, 1.2, 1.3**

- [x] 3. Implement progress event types
  - [x] 3.1 Create `src/tui/progress.rs` with ProgressEvent enum
    - Implement TestPhase enum (Initializing, Latency, Download, Upload, Complete)
    - Implement BandwidthDirection enum (Download, Upload)
    - Implement ProgressEvent variants (PhaseChange, LatencyMeasurement, BandwidthMeasurement, PhaseComplete, Error)
    - _Requirements: 9.1, 9.2, 9.3, 9.4_
  - [x] 3.2 Implement ProgressCallback trait
    - Define `on_progress(&self, event: ProgressEvent)` method
    - Ensure trait is Send + Sync for thread safety
    - _Requirements: 9.5_

- [x] 4. Implement TUI state management
  - [x] 4.1 Create `src/tui/state.rs` with TuiState struct
    - Implement ServerInfo and ConnectionInfo structs
    - Implement LatencyState with measurements, median, jitter
    - Implement BandwidthState with current speed, progress, final speed
    - Implement ErrorInfo struct
    - _Requirements: 2.1, 2.2, 2.3, 5.1, 5.2, 5.3_
  - [x] 4.2 Implement state update methods
    - `update_from_event(&mut self, event: &ProgressEvent)`
    - `set_metadata(&mut self, server: ServerInfo, connection: ConnectionInfo)`
    - `set_error(&mut self, message: String, suggestion: Option<String>)`
    - _Requirements: 3.3, 4.1, 4.2, 7.1, 7.3_
  - [x] 4.3 Write property test for progress monotonicity
    - **Property 3: Progress Percentage Monotonicity**
    - **Validates: Requirements 3.3**
  - [x] 4.4 Write property test for error state preservation
    - **Property 10: Error State Preservation**
    - **Validates: Requirements 7.1, 7.3, 7.4**

- [x] 5. Implement formatting and color utilities
  - [x] 5.1 Create formatting functions in `src/tui/renderer.rs`
    - `format_speed(speed_mbps: f64) -> String` with 2 decimal places
    - `format_latency(latency_ms: f64) -> String` with 2 decimal places
    - `format_size_label(bytes: u64) -> String` for file sizes
    - _Requirements: 4.4, 5.4_
  - [x] 5.2 Implement speed color coding
    - `speed_color(speed_mbps: f64) -> Color`
    - Green >= 100 Mbps, Yellow 25-100 Mbps, Red < 25 Mbps
    - _Requirements: 4.5_
  - [x] 5.3 Write property test for speed formatting precision
    - **Property 6: Speed Formatting Precision**
    - **Validates: Requirements 4.4, 5.4**
  - [x] 5.4 Write property test for speed color coding
    - **Property 7: Speed Color Coding Consistency**
    - **Validates: Requirements 4.5**

- [x] 6. Checkpoint - Verify core types compile
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Implement TUI renderer
  - [x] 7.1 Create layout functions in `src/tui/renderer.rs`
    - `render_frame(frame: &mut Frame, state: &TuiState)`
    - `render_metadata(frame: &mut Frame, area: Rect, state: &TuiState)`
    - `render_phase_indicator(frame: &mut Frame, area: Rect, state: &TuiState)`
    - `render_progress_or_results(frame: &mut Frame, area: Rect, state: &TuiState)`
    - `render_status_bar(frame: &mut Frame, area: Rect, state: &TuiState)`
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.4, 3.5_
  - [x] 7.2 Implement minimal mode layout
    - `is_minimal_mode(width: u16) -> bool` (threshold: 60)
    - `render_minimal_frame(frame: &mut Frame, state: &TuiState)`
    - _Requirements: 8.4_
  - [x] 7.3 Write property test for minimal mode threshold
    - **Property 11: Minimal Mode Threshold**
    - **Validates: Requirements 8.4**
  - [x] 7.4 Write property test for metadata rendering completeness
    - **Property 2: Metadata Rendering Completeness**
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [x] 8. Implement TuiController
  - [x] 8.1 Create `src/tui/controller.rs` with TuiController struct
    - Store DisplayMode, Arc<Mutex<TuiState>>, optional Terminal
    - Implement `new(mode: DisplayMode) -> Result<Self>`
    - Implement `mode(&self) -> DisplayMode`
    - _Requirements: 1.4_
  - [x] 8.2 Implement terminal lifecycle methods
    - `init(&mut self) -> Result<()>` - enter alternate screen, hide cursor
    - `cleanup(&mut self) -> Result<()>` - restore terminal state
    - Implement Drop trait for automatic cleanup
    - _Requirements: 8.2, 8.3_
  - [x] 8.3 Implement rendering and update methods
    - `set_metadata(&mut self, server: ServerInfo, connection: ConnectionInfo)`
    - `render(&mut self) -> Result<()>`
    - `show_results(&mut self, results: &SpeedTestResults) -> Result<()>`
    - _Requirements: 4.3, 6.1, 6.2, 6.3, 6.4, 6.5_
  - [x] 8.4 Implement progress callback
    - Create struct implementing ProgressCallback trait
    - `progress_callback(&self) -> Arc<dyn ProgressCallback>`
    - Non-blocking updates via Arc<Mutex<TuiState>>
    - _Requirements: 9.1, 9.5_

- [x] 9. Checkpoint - Verify TUI renders correctly
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Integrate progress callbacks into TestEngine
  - [x] 10.1 Add optional progress callback to TestEngine
    - Add `progress_callback: Option<Arc<dyn ProgressCallback>>` field
    - Update `TestEngine::new()` to accept optional callback
    - _Requirements: 9.1_
  - [x] 10.2 Emit progress events from test engine
    - Emit PhaseChange events at phase transitions
    - Emit LatencyMeasurement events after each latency measurement
    - Emit BandwidthMeasurement events after each bandwidth measurement
    - Emit PhaseComplete events when phases finish
    - _Requirements: 9.2, 9.3, 9.4_
  - [x] 10.3 Write property test for progress event emission
    - **Property 12: Progress Event Emission**
    - **Validates: Requirements 9.2, 9.3, 9.4**

- [x] 11. Update main.rs to use TUI
  - [x] 11.1 Integrate TuiController into main flow
    - Detect display mode using DisplayMode::detect()
    - Create TuiController with detected mode
    - Initialize TUI before tests start
    - Set metadata after fetching connection info
    - Pass progress callback to TestEngine
    - Render updates during test execution
    - Show final results
    - Clean up on exit
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3_
  - [x] 11.2 Implement error display in TUI mode
    - Display errors with red styling
    - Show suggestions when available
    - Preserve partial results
    - _Requirements: 7.1, 7.2, 7.3, 7.4_
  - [x] 11.3 Ensure JSON mode remains unchanged
    - Suppress all TUI output when --json flag is set
    - Output only final JSON to stdout
    - Output errors as JSON to stderr
    - _Requirements: 10.1, 10.2, 10.3, 10.4_
  - [x] 11.4 Write property test for JSON mode output
    - **Property 13: JSON Mode Output Correctness**
    - **Validates: Requirements 10.1, 10.2, 10.3, 10.4**

- [x] 12. Implement signal handling
  - [x] 12.1 Add SIGINT handler for graceful cleanup
    - Install signal handler using ctrlc crate or crossterm events
    - Trigger TuiController cleanup on SIGINT
    - Print partial results if available
    - _Requirements: 8.2, 8.3_

- [x] 13. Implement terminal resize handling
  - [x] 13.1 Handle terminal resize events
    - Update terminal_width in TuiState on resize
    - Re-render with new dimensions
    - Switch between normal and minimal mode as needed
    - _Requirements: 8.1, 8.4_

- [x] 14. Final checkpoint - Full integration testing
  - Ensure all tests pass, ask the user if questions arise.
  - Verify TUI displays correctly in interactive terminal
  - Verify JSON mode produces clean output
  - Verify piped output works correctly

## Notes

- All tasks including property tests are required
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The test engine modifications (Task 10) are designed to be backward compatible
