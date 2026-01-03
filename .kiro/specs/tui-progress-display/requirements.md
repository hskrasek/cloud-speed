# Requirements Document

## Introduction

This feature adds a Terminal User Interface (TUI) to cloud-speed that provides real-time visual feedback during speed tests, similar to the experience on speed.cloudflare.com. The TUI displays progress indicators, live measurements, and animated visualizations while tests run. When JSON output mode is enabled, the TUI is disabled to preserve machine-readable output for scripting and monitoring.

## Glossary

- **TUI**: Terminal User Interface - an interactive text-based interface rendered in the terminal
- **Progress_Indicator**: A visual element showing the current state and completion percentage of a test phase
- **Speed_Gauge**: A visual representation of the current measured speed, similar to a speedometer
- **Test_Phase**: A distinct stage of the speed test (latency, download, upload)
- **Live_Update**: Real-time display updates as measurements are collected
- **JSON_Mode**: Output mode where results are formatted as JSON for machine consumption

## Requirements

### Requirement 1: TUI Mode Detection

**User Story:** As a user, I want the TUI to automatically activate in interactive terminals, so that I get visual feedback without extra configuration.

#### Acceptance Criteria

1. WHEN the application starts without the `--json` flag AND stdout is a TTY, THE TUI_Controller SHALL activate the TUI display mode
2. WHEN the application starts with the `--json` flag, THE TUI_Controller SHALL disable the TUI and use silent mode during tests
3. WHEN stdout is not a TTY (piped output), THE TUI_Controller SHALL disable the TUI regardless of other flags
4. THE TUI_Controller SHALL provide a method to query the current display mode

### Requirement 2: Connection Metadata Display

**User Story:** As a user, I want to see my connection information displayed prominently, so that I know which server I'm testing against.

#### Acceptance Criteria

1. WHEN the TUI initializes, THE TUI_Controller SHALL display server location (city and IATA code)
2. WHEN the TUI initializes, THE TUI_Controller SHALL display network information (ISP name and ASN)
3. WHEN the TUI initializes, THE TUI_Controller SHALL display client IP and country
4. THE TUI_Controller SHALL format metadata with consistent alignment and styling

### Requirement 3: Test Phase Progress Indication

**User Story:** As a user, I want to see which test phase is currently running and its progress, so that I know how long the test will take.

#### Acceptance Criteria

1. WHEN a test phase begins, THE Progress_Indicator SHALL display the phase name (Latency, Download, Upload)
2. WHILE a test phase is running, THE Progress_Indicator SHALL show a progress bar or spinner
3. WHEN measurements complete within a phase, THE Progress_Indicator SHALL update the completion percentage
4. WHEN a test phase completes, THE Progress_Indicator SHALL mark it as complete with a checkmark or similar indicator
5. THE Progress_Indicator SHALL display the current file size being tested during bandwidth tests

### Requirement 4: Live Speed Display

**User Story:** As a user, I want to see the current speed measurement updating in real-time, so that I can observe my connection performance as it's measured.

#### Acceptance Criteria

1. WHILE download tests are running, THE Speed_Gauge SHALL display the current download speed in Mbps
2. WHILE upload tests are running, THE Speed_Gauge SHALL display the current upload speed in Mbps
3. WHEN a new measurement completes, THE Speed_Gauge SHALL update within 100ms
4. THE Speed_Gauge SHALL display speeds with 2 decimal places precision
5. THE Speed_Gauge SHALL use color coding to indicate speed quality (green for fast, yellow for moderate, red for slow)

### Requirement 5: Latency Display

**User Story:** As a user, I want to see latency measurements as they're collected, so that I understand my connection quality.

#### Acceptance Criteria

1. WHILE latency tests are running, THE TUI_Controller SHALL display the current latency measurement
2. WHEN latency measurements complete, THE TUI_Controller SHALL display the median latency
3. WHEN jitter is calculated, THE TUI_Controller SHALL display the jitter value
4. THE TUI_Controller SHALL display latency values in milliseconds with 2 decimal places

### Requirement 6: Final Results Summary

**User Story:** As a user, I want to see a clear summary of all results when the test completes, so that I can understand my overall connection performance.

#### Acceptance Criteria

1. WHEN all tests complete, THE TUI_Controller SHALL display a summary section
2. THE Summary SHALL include final download speed, upload speed, latency, and jitter
3. THE Summary SHALL include quality scores (Streaming, Gaming, Video Calls)
4. IF packet loss was measured, THE Summary SHALL include packet loss percentage
5. THE Summary SHALL use consistent formatting with the rest of the TUI

### Requirement 7: Error Display in TUI Mode

**User Story:** As a user, I want errors to be displayed clearly within the TUI, so that I understand what went wrong.

#### Acceptance Criteria

1. IF an error occurs during testing, THE TUI_Controller SHALL display the error message prominently
2. THE TUI_Controller SHALL use red coloring for error messages
3. THE TUI_Controller SHALL preserve any partial results collected before the error
4. WHEN displaying errors, THE TUI_Controller SHALL suggest possible solutions when available

### Requirement 8: Graceful Terminal Handling

**User Story:** As a user, I want the TUI to handle terminal resize and interrupts gracefully, so that the display doesn't break.

#### Acceptance Criteria

1. WHEN the terminal is resized, THE TUI_Controller SHALL adapt the display layout
2. WHEN the user sends SIGINT (Ctrl+C), THE TUI_Controller SHALL clean up the display before exiting
3. THE TUI_Controller SHALL restore the terminal to its original state on exit
4. IF the terminal width is too narrow, THE TUI_Controller SHALL fall back to a minimal display mode

### Requirement 9: Progress Callback Integration

**User Story:** As a developer, I want the test engine to emit progress events, so that the TUI can display real-time updates.

#### Acceptance Criteria

1. THE Test_Engine SHALL accept an optional progress callback
2. WHEN a latency measurement completes, THE Test_Engine SHALL emit a latency progress event
3. WHEN a bandwidth measurement completes, THE Test_Engine SHALL emit a bandwidth progress event with speed and size
4. WHEN a test phase changes, THE Test_Engine SHALL emit a phase change event
5. THE progress callback interface SHALL be non-blocking to avoid affecting measurement accuracy

### Requirement 10: JSON Mode Behavior

**User Story:** As a user running automated tests, I want JSON mode to produce clean output without TUI artifacts, so that I can parse the results programmatically.

#### Acceptance Criteria

1. WHEN `--json` flag is provided, THE Application SHALL suppress all TUI output
2. WHEN `--json` flag is provided, THE Application SHALL only output the final JSON result to stdout
3. WHEN `--json` flag is provided, THE Application SHALL output errors as JSON to stderr
4. THE JSON output format SHALL remain unchanged from the current implementation
