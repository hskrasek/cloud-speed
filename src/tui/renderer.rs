//! TUI rendering logic using ratatui.
//!
//! Handles the actual rendering of the TUI using ratatui widgets,
//! including layout, formatting, and color coding.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use super::progress::TestPhase;
use super::state::TuiState;

/// Get color for speed value based on thresholds.
///
/// - Green: >= 100 Mbps (fast)
/// - Yellow: 25-100 Mbps (moderate)
/// - Red: < 25 Mbps (slow)
pub fn speed_color(speed_mbps: f64) -> Color {
    if speed_mbps >= 100.0 {
        Color::Green
    } else if speed_mbps >= 25.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Format speed value with 2 decimal places.
pub fn format_speed(speed_mbps: f64) -> String {
    format!("{:.2} Mbps", speed_mbps)
}

/// Format latency value with 2 decimal places.
pub fn format_latency(latency_ms: f64) -> String {
    format!("{:.2} ms", latency_ms)
}

/// Format file size for display.
pub fn format_size_label(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Minimal mode threshold in columns.
const MINIMAL_MODE_THRESHOLD: u16 = 60;

/// Check if minimal mode should be used based on terminal width.
pub fn is_minimal_mode(width: u16) -> bool {
    width < MINIMAL_MODE_THRESHOLD
}

/// Render the TUI to the terminal.
///
/// This is the main entry point for rendering. It determines whether
/// to use normal or minimal mode based on terminal width.
pub fn render_frame(frame: &mut Frame, state: &TuiState) {
    if is_minimal_mode(frame.area().width) {
        render_minimal_frame(frame, state);
    } else {
        render_normal_frame(frame, state);
    }
}

/// Render the normal (full-width) TUI layout.
fn render_normal_frame(frame: &mut Frame, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Metadata
            Constraint::Length(3), // Current phase
            Constraint::Min(8),    // Progress/results
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    render_metadata(frame, chunks[0], state);
    render_phase_indicator(frame, chunks[1], state);
    render_progress_or_results(frame, chunks[2], state);
    render_status_bar(frame, chunks[3], state);
}

/// Render the minimal mode layout for narrow terminals.
pub fn render_minimal_frame(frame: &mut Frame, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Compact metadata
            Constraint::Length(2), // Phase + speed
            Constraint::Min(2),    // Latency/results
        ])
        .split(frame.area());

    render_minimal_metadata(frame, chunks[0], state);
    render_minimal_phase(frame, chunks[1], state);
    render_minimal_results(frame, chunks[2], state);
}

/// Render connection metadata (server, network, IP).
pub fn render_metadata(frame: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Server location
    if let Some(ref server) = state.server {
        lines.push(Line::from(vec![
            Span::styled(
                "Server: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ({})", server.city, server.iata),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }

    // Network info
    if let Some(ref conn) = state.connection {
        lines.push(Line::from(vec![
            Span::styled(
                "Network: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} (AS{})", conn.isp, conn.asn),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        // Client IP
        lines.push(Line::from(vec![
            Span::styled(
                "IP: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ({})", conn.ip, conn.country),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the current test phase indicator.
pub fn render_phase_indicator(frame: &mut Frame, area: Rect, state: &TuiState) {
    let phase_text = match state.phase {
        TestPhase::Initializing => "◐ Initializing...",
        TestPhase::Latency => "▶ Latency Test",
        TestPhase::Download => "▶ Download Test",
        TestPhase::Upload => "▶ Upload Test",
        TestPhase::Complete => "✓ Complete",
    };

    let style = match state.phase {
        TestPhase::Complete => {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        }
        _ => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    };

    let block = Block::default().borders(Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let paragraph = Paragraph::new(phase_text).style(style);
    frame.render_widget(paragraph, inner);
}

/// Render progress bars or final results depending on phase.
pub fn render_progress_or_results(
    frame: &mut Frame,
    area: Rect,
    state: &TuiState,
) {
    // Check for error state first
    if let Some(ref error) = state.error {
        render_error(frame, area, error);
        return;
    }

    match state.phase {
        TestPhase::Initializing => {
            render_initializing(frame, area);
        }
        TestPhase::Latency => {
            render_latency_progress(frame, area, state);
        }
        TestPhase::Download => {
            render_bandwidth_progress(frame, area, state, true);
        }
        TestPhase::Upload => {
            render_bandwidth_progress(frame, area, state, false);
        }
        TestPhase::Complete => {
            render_final_results(frame, area, state);
        }
    }
}

/// Render the status bar at the bottom.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &TuiState) {
    let status_text = match state.phase {
        TestPhase::Initializing => "Connecting to Cloudflare...".to_string(),
        TestPhase::Latency => {
            format!(
                "Measuring latency ({}/{})...",
                state.latency.current, state.latency.total
            )
        }
        TestPhase::Download => {
            let size_label = format_size_label(state.download.current_bytes);
            format!(
                "Testing {} download ({}/{})...",
                size_label,
                state.download.current_measurement,
                state.download.total_measurements
            )
        }
        TestPhase::Upload => {
            let size_label = format_size_label(state.upload.current_bytes);
            format!(
                "Testing {} upload ({}/{})...",
                size_label,
                state.upload.current_measurement,
                state.upload.total_measurements
            )
        }
        TestPhase::Complete => "Speed test complete.".to_string(),
    };

    let style = Style::default().fg(Color::DarkGray);
    let paragraph = Paragraph::new(status_text).style(style);
    frame.render_widget(paragraph, area);
}

// --- Helper rendering functions ---

/// Render the initializing state.
fn render_initializing(frame: &mut Frame, area: Rect) {
    let text = "Connecting to speed.cloudflare.com...";
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(paragraph, area);
}

/// Render latency test progress.
fn render_latency_progress(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Progress bar
            Constraint::Min(1),    // Current measurement
        ])
        .split(area);

    // Progress bar
    let progress = if state.latency.total > 0 {
        (state.latency.current as f64 / state.latency.total as f64).min(1.0)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent((progress * 100.0) as u16)
        .label(format!("{}%", (progress * 100.0) as u16));
    frame.render_widget(gauge, chunks[0]);

    // Current measurement
    let current_text = if let Some(&last) = state.latency.measurements.last() {
        format!("Current: {}", format_latency(last))
    } else {
        "Measuring...".to_string()
    };

    let paragraph = Paragraph::new(current_text)
        .style(Style::default().fg(Color::White));
    frame.render_widget(paragraph, chunks[1]);
}

/// Render bandwidth test progress (download or upload).
fn render_bandwidth_progress(
    frame: &mut Frame,
    area: Rect,
    state: &TuiState,
    is_download: bool,
) {
    let bandwidth_state = if is_download {
        &state.download
    } else {
        &state.upload
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Progress bar
            Constraint::Length(2), // Current speed
            Constraint::Min(1),    // Previous results
        ])
        .split(area);

    // Progress bar
    let progress = if bandwidth_state.total_measurements > 0 {
        (bandwidth_state.current_measurement as f64
            / bandwidth_state.total_measurements as f64)
            .min(1.0)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent((progress * 100.0) as u16)
        .label(format!("{}%", (progress * 100.0) as u16));
    frame.render_widget(gauge, chunks[0]);

    // Current speed
    let speed_text = if let Some(speed) = bandwidth_state.current_speed_mbps {
        let size_label = format_size_label(bandwidth_state.current_bytes);
        format!("Current: {} ({})", format_speed(speed), size_label)
    } else {
        "Measuring...".to_string()
    };

    let speed_color = bandwidth_state
        .current_speed_mbps
        .map(speed_color)
        .unwrap_or(Color::White);

    let paragraph =
        Paragraph::new(speed_text).style(Style::default().fg(speed_color));
    frame.render_widget(paragraph, chunks[1]);

    // Show completed latency results if available
    let mut lines = Vec::new();
    if state.latency.median_ms.is_some() {
        let latency_text = format!(
            "✓ Latency: {}",
            format_latency(state.latency.median_ms.unwrap())
        );
        let jitter_text = state
            .latency
            .jitter_ms
            .map(|j| format!("  Jitter: {}", format_latency(j)))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(latency_text, Style::default().fg(Color::Green)),
            Span::styled(jitter_text, Style::default().fg(Color::Green)),
        ]));
    }

    // Show download results if we're in upload phase
    if !is_download && state.download.final_speed_mbps.is_some() {
        let download_text = format!(
            "✓ Download: {}",
            format_speed(state.download.final_speed_mbps.unwrap())
        );
        lines.push(Line::from(Span::styled(
            download_text,
            Style::default().fg(Color::Green),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, chunks[2]);
}

/// Render final results summary.
fn render_final_results(frame: &mut Frame, area: Rect, state: &TuiState) {
    let mut lines = Vec::new();

    // Latency
    if let Some(median) = state.latency.median_ms {
        let mut spans = vec![
            Span::styled(
                "Latency: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format_latency(median), Style::default().fg(Color::Red)),
        ];

        if let Some(jitter) = state.latency.jitter_ms {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                "Jitter: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format_latency(jitter),
                Style::default().fg(Color::Red),
            ));
        }

        lines.push(Line::from(spans));
    }

    // Download
    if let Some(speed) = state.download.final_speed_mbps {
        lines.push(Line::from(vec![
            Span::styled(
                "Download: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format_speed(speed),
                Style::default().fg(speed_color(speed)),
            ),
        ]));
    }

    // Upload
    if let Some(speed) = state.upload.final_speed_mbps {
        lines.push(Line::from(vec![
            Span::styled(
                "Upload: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format_speed(speed),
                Style::default().fg(speed_color(speed)),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render error message.
fn render_error(
    frame: &mut Frame,
    area: Rect,
    error: &super::state::ErrorInfo,
) {
    let mut lines = vec![Line::from(Span::styled(
        format!("Error: {}", error.message),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    ))];

    if let Some(ref suggestion) = error.suggestion {
        lines.push(Line::from(Span::styled(
            format!("Suggestion: {}", suggestion),
            Style::default().fg(Color::Yellow),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

// --- Minimal mode rendering functions ---

/// Render compact metadata for minimal mode.
fn render_minimal_metadata(frame: &mut Frame, area: Rect, state: &TuiState) {
    let text = match (&state.server, &state.connection) {
        (Some(server), Some(conn)) => {
            format!("{} | {}", server.iata, conn.isp)
        }
        (Some(server), None) => server.iata.clone(),
        (None, Some(conn)) => conn.isp.clone(),
        (None, None) => "Connecting...".to_string(),
    };

    let paragraph =
        Paragraph::new(text).style(Style::default().fg(Color::Cyan));
    frame.render_widget(paragraph, area);
}

/// Render compact phase indicator for minimal mode.
fn render_minimal_phase(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Phase with progress
    let (phase_text, progress) = match state.phase {
        TestPhase::Initializing => ("◐ Init".to_string(), 0),
        TestPhase::Latency => {
            let pct = if state.latency.total > 0 {
                (state.latency.current * 100) / state.latency.total
            } else {
                0
            };
            (format!("▶ Latency {}%", pct), pct)
        }
        TestPhase::Download => {
            let pct = if state.download.total_measurements > 0 {
                (state.download.current_measurement * 100)
                    / state.download.total_measurements
            } else {
                0
            };
            (format!("▶ Download {}%", pct), pct)
        }
        TestPhase::Upload => {
            let pct = if state.upload.total_measurements > 0 {
                (state.upload.current_measurement * 100)
                    / state.upload.total_measurements
            } else {
                0
            };
            (format!("▶ Upload {}%", pct), pct)
        }
        TestPhase::Complete => ("✓ Done".to_string(), 100),
    };

    let style = if progress == 100 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let paragraph = Paragraph::new(phase_text).style(style);
    frame.render_widget(paragraph, chunks[0]);

    // Current speed
    let speed_text = match state.phase {
        TestPhase::Download => state
            .download
            .current_speed_mbps
            .map(format_speed)
            .unwrap_or_default(),
        TestPhase::Upload => state
            .upload
            .current_speed_mbps
            .map(format_speed)
            .unwrap_or_default(),
        _ => String::new(),
    };

    let speed_color = match state.phase {
        TestPhase::Download => state
            .download
            .current_speed_mbps
            .map(speed_color)
            .unwrap_or(Color::White),
        TestPhase::Upload => state
            .upload
            .current_speed_mbps
            .map(speed_color)
            .unwrap_or(Color::White),
        _ => Color::White,
    };

    let paragraph =
        Paragraph::new(speed_text).style(Style::default().fg(speed_color));
    frame.render_widget(paragraph, chunks[1]);
}

/// Render compact results for minimal mode.
fn render_minimal_results(frame: &mut Frame, area: Rect, state: &TuiState) {
    // Check for error state first
    if let Some(ref error) = state.error {
        let paragraph = Paragraph::new(format!("Error: {}", error.message))
            .style(Style::default().fg(Color::Red));
        frame.render_widget(paragraph, area);
        return;
    }

    let text = if let Some(median) = state.latency.median_ms {
        format!("Latency: {}", format_latency(median))
    } else {
        String::new()
    };

    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::Red));
    frame.render_widget(paragraph, area);
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::Config as ProptestConfig;

    // **Feature: tui-progress-display, Property 6: Speed Formatting Precision**
    // **Validates: Requirements 4.4, 5.4**
    proptest! {
        #[test]
        fn prop_speed_formatting_precision(speed in proptest::num::f64::NORMAL) {
            let formatted = format_speed(speed);
            // Should end with " Mbps"
            prop_assert!(formatted.ends_with(" Mbps"));
            // Extract the numeric part
            let numeric_part = formatted.trim_end_matches(" Mbps");
            // Should have exactly 2 decimal places
            if let Some(dot_pos) = numeric_part.find('.') {
                let decimal_places = numeric_part.len() - dot_pos - 1;
                prop_assert_eq!(decimal_places, 2);
            } else {
                prop_assert!(false, "No decimal point found in formatted speed");
            }
        }

        #[test]
        fn prop_latency_formatting_precision(latency in proptest::num::f64::NORMAL) {
            let formatted = format_latency(latency);
            // Should end with " ms"
            prop_assert!(formatted.ends_with(" ms"));
            // Extract the numeric part
            let numeric_part = formatted.trim_end_matches(" ms");
            // Should have exactly 2 decimal places
            if let Some(dot_pos) = numeric_part.find('.') {
                let decimal_places = numeric_part.len() - dot_pos - 1;
                prop_assert_eq!(decimal_places, 2);
            } else {
                prop_assert!(false, "No decimal point found in formatted latency");
            }
        }
    }

    // **Feature: tui-progress-display, Property 7: Speed Color Coding Consistency**
    // **Validates: Requirements 4.5**
    proptest! {
        #[test]
        fn prop_speed_color_coding_fast(speed in 100.0f64..=f64::MAX) {
            // speed >= 100.0 → Green
            if speed.is_finite() {
                prop_assert_eq!(speed_color(speed), Color::Green);
            }
        }

        #[test]
        fn prop_speed_color_coding_moderate(speed in 25.0f64..100.0f64) {
            // 25.0 <= speed < 100.0 → Yellow
            prop_assert_eq!(speed_color(speed), Color::Yellow);
        }

        #[test]
        fn prop_speed_color_coding_slow(speed in f64::MIN..25.0f64) {
            // speed < 25.0 → Red
            if speed.is_finite() {
                prop_assert_eq!(speed_color(speed), Color::Red);
            }
        }
    }

    #[test]
    fn test_format_size_label() {
        assert_eq!(format_size_label(0), "0B");
        assert_eq!(format_size_label(512), "512B");
        assert_eq!(format_size_label(1024), "1.0KB");
        assert_eq!(format_size_label(1536), "1.5KB");
        assert_eq!(format_size_label(1024 * 1024), "1.0MB");
        assert_eq!(format_size_label(10 * 1024 * 1024), "10.0MB");
        assert_eq!(format_size_label(1024 * 1024 * 1024), "1.0GB");
    }

    // **Feature: tui-progress-display, Property 11: Minimal Mode Threshold**
    // **Validates: Requirements 8.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any terminal_width < 60, the layout mode SHALL be
        /// Minimal.
        #[test]
        fn prop_minimal_mode_below_threshold(width in 0u16..60) {
            prop_assert!(
                is_minimal_mode(width),
                "Width {} should trigger minimal mode (< 60)",
                width
            );
        }

        /// Property: For any terminal_width >= 60, the layout mode SHALL be
        /// Normal (not minimal).
        #[test]
        fn prop_normal_mode_at_or_above_threshold(width in 60u16..=u16::MAX) {
            prop_assert!(
                !is_minimal_mode(width),
                "Width {} should NOT trigger minimal mode (>= 60)",
                width
            );
        }
    }

    #[test]
    fn test_minimal_mode_boundary() {
        // Exactly at threshold
        assert!(!is_minimal_mode(60));
        // Just below threshold
        assert!(is_minimal_mode(59));
        // Well below threshold
        assert!(is_minimal_mode(40));
        // Well above threshold
        assert!(!is_minimal_mode(80));
    }

    // **Feature: tui-progress-display, Property 2: Metadata Rendering Completeness**
    // **Validates: Requirements 2.1, 2.2, 2.3**
    //
    // Helper function to render metadata to a test buffer and extract text
    fn render_metadata_to_string(state: &TuiState) -> String {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_metadata(frame, area, state);
            })
            .unwrap();

        // Extract text from the buffer
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = buffer.cell((x, y)).unwrap();
                text.push_str(cell.symbol());
            }
            text.push('\n');
        }
        text
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any valid ServerInfo and ConnectionInfo, when rendered
        /// to a string representation, the output SHALL contain:
        /// - The server city name
        /// - The server IATA code
        /// - The ISP name
        /// - The ASN number
        /// - The client IP address
        /// - The country code
        #[test]
        fn prop_metadata_rendering_completeness(
            city in "[A-Za-z ]{3,20}",
            iata in "[A-Z]{3}",
            isp in "[A-Za-z0-9 ]{3,20}",
            asn in 1i64..100000,
            ip in "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
            country in "[A-Z]{2}"
        ) {
            use super::super::state::{ConnectionInfo, ServerInfo};

            let mut state = TuiState::default();
            state.server = Some(ServerInfo {
                city: city.clone(),
                iata: iata.clone(),
            });
            state.connection = Some(ConnectionInfo {
                ip: ip.clone(),
                country: country.clone(),
                isp: isp.clone(),
                asn,
            });

            let rendered = render_metadata_to_string(&state);

            // Verify all required fields are present
            prop_assert!(
                rendered.contains(&city),
                "Rendered metadata should contain city '{}': {}",
                city,
                rendered
            );
            prop_assert!(
                rendered.contains(&iata),
                "Rendered metadata should contain IATA code '{}': {}",
                iata,
                rendered
            );
            prop_assert!(
                rendered.contains(&isp),
                "Rendered metadata should contain ISP '{}': {}",
                isp,
                rendered
            );
            prop_assert!(
                rendered.contains(&asn.to_string()),
                "Rendered metadata should contain ASN '{}': {}",
                asn,
                rendered
            );
            prop_assert!(
                rendered.contains(&ip),
                "Rendered metadata should contain IP '{}': {}",
                ip,
                rendered
            );
            prop_assert!(
                rendered.contains(&country),
                "Rendered metadata should contain country '{}': {}",
                country,
                rendered
            );
        }
    }
}
