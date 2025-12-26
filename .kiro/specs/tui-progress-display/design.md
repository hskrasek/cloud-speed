# Design Document: TUI Progress Display

## Overview

This design introduces a Terminal User Interface (TUI) for cloud-speed that provides real-time visual feedback during speed tests. The TUI displays connection metadata, progress indicators, live speed measurements, and a final results summary—emulating the experience of speed.cloudflare.com in the terminal.

The implementation uses `ratatui` with the `crossterm` backend for cross-platform terminal rendering. The architecture separates display logic from test execution through a progress callback system, ensuring measurement accuracy is not affected by UI updates.

## Architecture

The TUI system consists of three main layers:

```
┌─────────────────────────────────────────────────────────────┐
│                      main.rs (Orchestration)                │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │  TuiController  │◄───│  TestEngine (with callbacks)    │ │
│  │  - mode detect  │    │  - emits ProgressEvent          │ │
│  │  - state mgmt   │    │  - non-blocking updates         │ │
│  └────────┬────────┘    └─────────────────────────────────┘ │
│           │                                                  │
│  ┌────────▼────────┐                                        │
│  │   TuiRenderer   │                                        │
│  │  - ratatui      │                                        │
│  │  - widgets      │                                        │
│  └─────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

### Display Mode Detection

The system determines display mode at startup based on:
1. `--json` flag presence → JSON mode (no TUI)
2. stdout TTY detection → TUI mode if TTY, silent mode if not

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  JSON flag?  │─Yes─►  JSON Mode   │     │  Silent Mode │
└──────┬───────┘     └──────────────┘     └──────────────┘
       │No                                       ▲
       ▼                                         │
┌──────────────┐                                 │
│  stdout TTY? │─No──────────────────────────────┘
└──────┬───────┘
       │Yes
       ▼
┌──────────────┐
│   TUI Mode   │
└──────────────┘
```

## Components and Interfaces

### DisplayMode Enum

```rust
/// The display mode for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Full TUI with progress indicators and live updates
    Tui,
    /// Silent mode - no output until final results
    Silent,
    /// JSON mode - structured output only
    Json,
}

impl DisplayMode {
    /// Determine display mode from CLI flags and environment.
    pub fn detect(json_flag: bool, is_tty: bool) -> Self {
        if json_flag {
            DisplayMode::Json
        } else if is_tty {
            DisplayMode::Tui
        } else {
            DisplayMode::Silent
        }
    }
}
```

### ProgressEvent Enum

Events emitted by the test engine to update the TUI:

```rust
/// Progress events emitted during test execution.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Test phase has changed
    PhaseChange(TestPhase),
    /// Latency measurement completed
    LatencyMeasurement {
        value_ms: f64,
        current: usize,
        total: usize,
    },
    /// Bandwidth measurement completed
    BandwidthMeasurement {
        direction: BandwidthDirection,
        speed_mbps: f64,
        bytes: u64,
        current: usize,
        total: usize,
    },
    /// Phase completed with results
    PhaseComplete(TestPhase),
    /// Error occurred
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestPhase {
    Initializing,
    Latency,
    Download,
    Upload,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthDirection {
    Download,
    Upload,
}
```

### TuiState Struct

Holds all state needed for rendering:

```rust
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

#[derive(Debug, Clone, Default)]
pub struct LatencyState {
    pub measurements: Vec<f64>,
    pub current: usize,
    pub total: usize,
    pub median_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct BandwidthState {
    pub current_speed_mbps: Option<f64>,
    pub current_bytes: u64,
    pub current_measurement: usize,
    pub total_measurements: usize,
    pub final_speed_mbps: Option<f64>,
    pub completed: bool,
}
```

### ProgressCallback Trait

```rust
/// Callback interface for progress updates.
/// 
/// Implementations must be non-blocking to avoid affecting
/// measurement accuracy.
pub trait ProgressCallback: Send + Sync {
    /// Called when a progress event occurs.
    fn on_progress(&self, event: ProgressEvent);
}
```

### TuiController

Main controller that manages the TUI lifecycle:

```rust
/// Controller for the TUI display.
pub struct TuiController {
    mode: DisplayMode,
    state: Arc<Mutex<TuiState>>,
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
}

impl TuiController {
    /// Create a new TUI controller.
    pub fn new(mode: DisplayMode) -> Result<Self, Box<dyn Error>>;
    
    /// Initialize the TUI (enters alternate screen if TUI mode).
    pub fn init(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// Set connection metadata for display.
    pub fn set_metadata(&mut self, server: ServerInfo, connection: ConnectionInfo);
    
    /// Get a progress callback for the test engine.
    pub fn progress_callback(&self) -> Arc<dyn ProgressCallback>;
    
    /// Render the current state.
    pub fn render(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// Display final results.
    pub fn show_results(&mut self, results: &SpeedTestResults) -> Result<(), Box<dyn Error>>;
    
    /// Clean up and restore terminal.
    pub fn cleanup(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// Get current display mode.
    pub fn mode(&self) -> DisplayMode;
}
```

### TuiRenderer Module

Handles the actual rendering using ratatui widgets:

```rust
/// Render the TUI to the terminal.
pub fn render_frame(frame: &mut Frame, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),  // Metadata
            Constraint::Length(3),  // Current phase
            Constraint::Min(8),     // Progress/results
            Constraint::Length(1),  // Status bar
        ])
        .split(frame.area());
    
    render_metadata(frame, chunks[0], state);
    render_phase_indicator(frame, chunks[1], state);
    render_progress_or_results(frame, chunks[2], state);
    render_status_bar(frame, chunks[3], state);
}
```

## Data Models

### ServerInfo

```rust
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub city: String,
    pub iata: String,
}
```

### ConnectionInfo

```rust
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub ip: String,
    pub country: String,
    pub isp: String,
    pub asn: i64,
}
```

### ErrorInfo

```rust
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub message: String,
    pub suggestion: Option<String>,
}
```

## TUI Layout

The TUI uses a vertical layout with four main sections:

```
┌─────────────────────────────────────────────────────────────┐
│  Server: San Francisco (SFO)                                │
│  Network: Comcast (AS7922)                                  │
│  IP: 203.0.113.1 (US)                                       │
├─────────────────────────────────────────────────────────────┤
│  ▶ Download Test                                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│     ████████████████████░░░░░░░░░░  75%                    │
│                                                             │
│     Current: 95.42 Mbps (10MB)                             │
│                                                             │
│  ✓ Latency: 15.23 ms  Jitter: 2.45 ms                      │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  Testing 10MB download (6/8)...                            │
└─────────────────────────────────────────────────────────────┘
```

### Minimal Mode (narrow terminals)

When terminal width < 60 columns:

```
┌────────────────────────────────┐
│ SFO | Comcast                  │
├────────────────────────────────┤
│ ▶ Download 75%                 │
│ 95.42 Mbps                     │
├────────────────────────────────┤
│ Latency: 15.23ms               │
└────────────────────────────────┘
```

## Speed Color Coding

Speed values are color-coded based on thresholds:

| Speed Range | Color | Quality |
|-------------|-------|---------|
| ≥ 100 Mbps | Green | Fast |
| 25-100 Mbps | Yellow | Moderate |
| < 25 Mbps | Red | Slow |

```rust
/// Get color for speed value.
pub fn speed_color(speed_mbps: f64) -> Color {
    if speed_mbps >= 100.0 {
        Color::Green
    } else if speed_mbps >= 25.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Display Mode Selection

*For any* combination of (json_flag: bool, is_tty: bool), the DisplayMode::detect function SHALL return:
- Json when json_flag is true (regardless of is_tty)
- Tui when json_flag is false AND is_tty is true
- Silent when json_flag is false AND is_tty is false

**Validates: Requirements 1.1, 1.2, 1.3**

### Property 2: Metadata Rendering Completeness

*For any* valid ServerInfo and ConnectionInfo, when rendered to a string representation, the output SHALL contain:
- The server city name
- The server IATA code
- The ISP name
- The ASN number
- The client IP address
- The country code

**Validates: Requirements 2.1, 2.2, 2.3**

### Property 3: Progress Percentage Monotonicity

*For any* sequence of ProgressEvents for a single test phase, the completion percentage (current/total) SHALL be monotonically non-decreasing.

**Validates: Requirements 3.3**

### Property 4: Phase State Transitions

*For any* TuiState, when a PhaseComplete event is processed:
- The phase's completed flag SHALL be set to true
- The phase's final results SHALL be populated

**Validates: Requirements 3.4, 3.5**

### Property 5: Speed Display Updates

*For any* BandwidthMeasurement event, the corresponding BandwidthState (download or upload based on direction) SHALL have its current_speed_mbps updated to the event's speed_mbps value.

**Validates: Requirements 4.1, 4.2**

### Property 6: Speed Formatting Precision

*For any* speed value (f64), when formatted for display, the resulting string SHALL contain exactly 2 decimal places.

**Validates: Requirements 4.4, 5.4**

### Property 7: Speed Color Coding Consistency

*For any* speed value:
- speed >= 100.0 → Green
- 25.0 <= speed < 100.0 → Yellow  
- speed < 25.0 → Red

**Validates: Requirements 4.5**

### Property 8: Latency State Completeness

*For any* TuiState after latency phase completion:
- median_ms SHALL be Some(value) where value equals the statistical median of all measurements
- If measurements.len() >= 2, jitter_ms SHALL be Some(value)

**Validates: Requirements 5.1, 5.2, 5.3**

### Property 9: Summary Completeness

*For any* TuiState in Complete phase, the state SHALL contain:
- download.final_speed_mbps as Some(value)
- upload.final_speed_mbps as Some(value)
- latency.median_ms as Some(value)

**Validates: Requirements 6.1, 6.2, 6.3, 6.4**

### Property 10: Error State Preservation

*For any* TuiState that receives an Error event after collecting N measurements, the state SHALL:
- Have error set to Some(ErrorInfo)
- Preserve all N previously collected measurements in their respective states

**Validates: Requirements 7.1, 7.3, 7.4**

### Property 11: Minimal Mode Threshold

*For any* terminal_width < 60, the layout mode SHALL be Minimal.

**Validates: Requirements 8.4**

### Property 12: Progress Event Emission

*For any* test execution with the test engine:
- Each latency measurement SHALL emit exactly one LatencyMeasurement event
- Each bandwidth measurement SHALL emit exactly one BandwidthMeasurement event
- Each phase transition SHALL emit exactly one PhaseChange event

**Validates: Requirements 9.2, 9.3, 9.4**

### Property 13: JSON Mode Output Correctness

*For any* execution with json_flag=true:
- stdout SHALL contain only valid JSON
- The JSON SHALL deserialize to SpeedTestResults
- No TUI escape sequences SHALL appear in stdout

**Validates: Requirements 10.1, 10.2, 10.3, 10.4**

## Error Handling

### Terminal Initialization Errors

If terminal initialization fails (e.g., not a real terminal), fall back to Silent mode gracefully.

### Render Errors

Render errors are logged but don't stop test execution. The test continues and results are still collected.

### Signal Handling

SIGINT (Ctrl+C) triggers cleanup:
1. Restore terminal to original state
2. Show cursor
3. Exit alternate screen
4. Print partial results if available

```rust
impl Drop for TuiController {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
```

## Testing Strategy

### Unit Tests

Unit tests verify individual components:
- DisplayMode::detect() with all input combinations
- Speed color coding thresholds
- Formatting functions (speed, latency)
- State update logic

### Property-Based Tests

Property tests use `proptest` to verify:
- Display mode selection (Property 1)
- Speed formatting precision (Property 6)
- Speed color coding (Property 7)
- Progress monotonicity (Property 3)

Configuration:
- Minimum 100 iterations per property test
- Each test tagged with: **Feature: tui-progress-display, Property N: {description}**

### Integration Tests

Integration tests verify:
- Full TUI lifecycle (init → render → cleanup)
- Progress callback integration with test engine
- JSON mode produces valid output
- Terminal state restoration

### Test Approach

- **Unit tests**: Verify formatting, color coding, state transitions
- **Property tests**: Verify invariants across random inputs
- **Integration tests**: Verify end-to-end behavior

Property-based testing library: `proptest` (already a dev dependency)

## Dependencies

New dependencies to add to `Cargo.toml`:

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
```

These provide:
- `ratatui`: TUI framework with widgets (Gauge, Block, Paragraph, etc.)
- `crossterm`: Cross-platform terminal manipulation (already used by ratatui)

## File Structure

```
src/
├── tui/
│   ├── mod.rs           # Module exports
│   ├── controller.rs    # TuiController implementation
│   ├── renderer.rs      # Rendering logic with ratatui
│   ├── state.rs         # TuiState and related types
│   ├── progress.rs      # ProgressEvent and ProgressCallback
│   └── display_mode.rs  # DisplayMode enum and detection
├── main.rs              # Updated to use TuiController
└── cloudflare/
    └── tests/
        └── engine.rs    # Updated with progress callbacks
```
