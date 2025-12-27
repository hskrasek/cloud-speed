//! TUI rendering logic using ratatui.
//!
//! Handles the actual rendering of the TUI using ratatui widgets,
//! including layout, formatting, and color coding. Designed to match
//! the Cloudflare speed test dashboard style.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
    Frame,
};

use super::progress::TestPhase;
use super::state::{QualityRating, TuiState};

/// Get color for speed value based on thresholds.
pub fn speed_color(speed_mbps: f64) -> Color {
    if speed_mbps >= 100.0 {
        Color::Green
    } else if speed_mbps >= 25.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Get color for quality rating.
pub fn quality_color(rating: &QualityRating) -> Color {
    match rating {
        QualityRating::Great => Color::Green,
        QualityRating::Good => Color::LightGreen,
        QualityRating::Average => Color::Yellow,
        QualityRating::Poor => Color::Red,
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
#[allow(dead_code)]
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
pub fn render_frame(frame: &mut Frame, state: &TuiState) {
    if is_minimal_mode(frame.area().width) {
        render_minimal_frame(frame, state);
    } else {
        render_dashboard_frame(frame, state);
    }
}

/// Render the dashboard-style TUI layout (like Cloudflare's speed test).
fn render_dashboard_frame(frame: &mut Frame, state: &TuiState) {
    let area = frame.area();

    // Main layout: header, content, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with title
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_header(frame, main_chunks[0], state);
    render_main_content(frame, main_chunks[1], state);
    render_status_bar(frame, main_chunks[2], state);
}

/// Render the header with title and server info.
fn render_header(frame: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let title_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(40)])
        .split(inner);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("☁ ", Style::default().fg(Color::Cyan)),
        Span::styled(
            "Speed Test",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(title, title_chunks[0]);

    // Server info on the right
    if let Some(ref server) = state.server {
        let server_info = Paragraph::new(Line::from(vec![
            Span::styled("Server: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} ({})", server.city, server.iata),
                Style::default().fg(Color::Cyan),
            ),
        ]))
        .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(server_info, title_chunks[1]);
    }
}

/// Render the main content area with speed displays and graphs.
fn render_main_content(frame: &mut Frame, area: Rect, state: &TuiState) {
    // Check for error state first
    if let Some(ref error) = state.error {
        render_error(frame, area, error);
        return;
    }

    // Layout: connection info, speeds, graphs, quality/latency
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Connection info
            Constraint::Length(5), // Speed displays
            Constraint::Min(6),    // Graphs
            Constraint::Length(6), // Quality scores and latency
        ])
        .split(area);

    render_connection_info(frame, content_chunks[0], state);
    render_speed_displays(frame, content_chunks[1], state);
    render_speed_graphs(frame, content_chunks[2], state);
    render_bottom_section(frame, content_chunks[3], state);
}

/// Render connection information section.
fn render_connection_info(frame: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Connection ",
            Style::default().fg(Color::White),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Server location
    if let Some(ref server) = state.server {
        lines.push(Line::from(vec![
            Span::styled("⚡ Server: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} ({})", server.city, server.iata),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }

    // Network info
    if let Some(ref conn) = state.connection {
        lines.push(Line::from(vec![
            Span::styled("⊙ Network: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} (AS{})", conn.isp, conn.asn),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled("⊡ Your IP: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} ({})", conn.ip, conn.country),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the large speed displays (Download, Upload, Latency, Jitter).
fn render_speed_displays(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Download speed
    render_metric_box(
        frame,
        chunks[0],
        "Download",
        state.download.final_speed_mbps.or(state.download.current_speed_mbps),
        "Mbps",
        state.phase == TestPhase::Download,
        speed_color,
    );

    // Upload speed
    render_metric_box(
        frame,
        chunks[1],
        "Upload",
        state.upload.final_speed_mbps.or(state.upload.current_speed_mbps),
        "Mbps",
        state.phase == TestPhase::Upload,
        speed_color,
    );

    // Latency
    render_metric_box(
        frame,
        chunks[2],
        "Latency",
        state.latency.median_ms,
        "ms",
        state.phase == TestPhase::Latency,
        |v| {
            if v <= 30.0 {
                Color::Green
            } else if v <= 100.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        },
    );

    // Jitter
    render_metric_box(
        frame,
        chunks[3],
        "Jitter",
        state.latency.jitter_ms,
        "ms",
        false,
        |v| {
            if v <= 10.0 {
                Color::Green
            } else if v <= 30.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        },
    );
}

/// Render a single metric box with large value display.
fn render_metric_box<F>(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: Option<f64>,
    unit: &str,
    is_active: bool,
    color_fn: F,
) where
    F: Fn(f64) -> Color,
{
    let border_color = if is_active { Color::Cyan } else { Color::DarkGray };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            format!(" {} ", label),
            Style::default().fg(Color::White),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = if let Some(v) = value {
        let color = color_fn(v);
        vec![
            Line::from(Span::styled(
                format!("{:.1}", v),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                unit,
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else if is_active {
        vec![Line::from(Span::styled(
            "...",
            Style::default().fg(Color::Yellow),
        ))]
    } else {
        vec![Line::from(Span::styled(
            "—",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let paragraph =
        Paragraph::new(content).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, inner);
}

/// Render speed graphs for download and upload.
fn render_speed_graphs(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_speed_graph(
        frame,
        chunks[0],
        "Download",
        &state.download,
        Color::Rgb(255, 165, 0),
    );
    render_speed_graph(
        frame,
        chunks[1],
        "Upload",
        &state.upload,
        Color::Magenta,
    );
}

/// Render a single speed graph using sparkline.
fn render_speed_graph(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    bandwidth: &super::state::BandwidthState,
    color: Color,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", label),
            Style::default().fg(Color::White),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if bandwidth.speed_history.is_empty() {
        let placeholder = Paragraph::new("Waiting for data...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(placeholder, inner);
        return;
    }

    // Convert speed history to sparkline data
    let max_speed = bandwidth
        .speed_history
        .iter()
        .map(|s| s.speed_mbps)
        .fold(0.0f64, |a, b| a.max(b));

    let data: Vec<u64> = bandwidth
        .speed_history
        .iter()
        .map(|s| {
            if max_speed > 0.0 {
                ((s.speed_mbps / max_speed) * 100.0) as u64
            } else {
                0
            }
        })
        .collect();

    // Split inner area for sparkline and percentile label
    let graph_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(inner);

    let sparkline =
        Sparkline::default().data(&data).style(Style::default().fg(color));
    frame.render_widget(sparkline, graph_chunks[0]);

    // Show 90th percentile label
    let percentile_text = if let Some(p90) = bandwidth.percentile_90 {
        format!("90th percentile: {:.1} Mbps", p90)
    } else if let Some(speed) = bandwidth.current_speed_mbps {
        format!("Current: {:.1} Mbps", speed)
    } else {
        String::new()
    };

    let percentile_label = Paragraph::new(percentile_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Left);
    frame.render_widget(percentile_label, graph_chunks[1]);
}

/// Render the bottom section with quality scores and latency details.
fn render_bottom_section(frame: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_quality_scores(frame, chunks[0], state);
    render_latency_details(frame, chunks[1], state);
}

/// Render the Network Quality Score section.
fn render_quality_scores(frame: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Network Quality Score ",
            Style::default().fg(Color::White),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        // Video Streaming
        render_quality_line(
            "Video Streaming:",
            state.quality_scores.streaming.as_ref(),
        ),
        // Online Gaming
        render_quality_line(
            "Online Gaming:",
            state.quality_scores.gaming.as_ref(),
        ),
        // Video Chatting
        render_quality_line(
            "Video Chatting:",
            state.quality_scores.video_conferencing.as_ref(),
        ),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render a single quality score line.
fn render_quality_line<'a>(
    label: &'a str,
    rating: Option<&QualityRating>,
) -> Line<'a> {
    let rating_span = if let Some(r) = rating {
        Span::styled(
            r.as_str(),
            Style::default().fg(quality_color(r)).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("—", Style::default().fg(Color::DarkGray))
    };

    Line::from(vec![
        Span::styled(label, Style::default().fg(Color::White)),
        Span::raw(" "),
        rating_span,
    ])
}

/// Render latency measurement details.
fn render_latency_details(frame: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Latency Measurements ",
            Style::default().fg(Color::White),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Unloaded latency
    let idle_text = if let Some(ms) = state.latency.median_ms {
        format!("{:.1} ms", ms)
    } else {
        "—".to_string()
    };
    lines.push(Line::from(vec![
        Span::styled("Unloaded latency: ", Style::default().fg(Color::White)),
        Span::styled(idle_text, Style::default().fg(Color::Cyan)),
    ]));

    // Latency during download
    let down_text = if let Some(ms) = state.latency.loaded_down_ms {
        format!("{:.1} ms", ms)
    } else {
        "—".to_string()
    };
    lines.push(Line::from(vec![
        Span::styled("During download: ", Style::default().fg(Color::White)),
        Span::styled(down_text, Style::default().fg(Color::Rgb(255, 165, 0))),
    ]));

    // Latency during upload
    let up_text = if let Some(ms) = state.latency.loaded_up_ms {
        format!("{:.1} ms", ms)
    } else {
        "—".to_string()
    };
    lines.push(Line::from(vec![
        Span::styled("During upload: ", Style::default().fg(Color::White)),
        Span::styled(up_text, Style::default().fg(Color::Magenta)),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the status bar at the bottom.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &TuiState) {
    let status_text = if state.waiting_for_exit {
        "Press 'r' to retest • 'q' or Esc to exit"
    } else {
        match state.phase {
            TestPhase::Initializing => "Connecting to Cloudflare...",
            TestPhase::Latency => "Measuring latency...",
            TestPhase::Download => "Testing download speed...",
            TestPhase::Upload => "Testing upload speed...",
            TestPhase::Complete => "Speed test complete",
        }
    };

    let style = if state.waiting_for_exit {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let paragraph = Paragraph::new(status_text).style(style);
    frame.render_widget(paragraph, area);
}

/// Render error message.
fn render_error(
    frame: &mut Frame,
    area: Rect,
    error: &super::state::ErrorInfo,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " Error ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::from(Span::styled(
        &error.message,
        Style::default().fg(Color::Red),
    ))];

    if let Some(ref suggestion) = error.suggestion {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Suggestion: {}", suggestion),
            Style::default().fg(Color::Yellow),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

// --- Minimal mode rendering functions ---

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

    let paragraph =
        Paragraph::new(text).style(Style::default().fg(Color::Red));
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::Config as ProptestConfig;

    proptest! {
        #[test]
        fn prop_speed_formatting_precision(speed in proptest::num::f64::NORMAL) {
            let formatted = format_speed(speed);
            prop_assert!(formatted.ends_with(" Mbps"));
            let numeric_part = formatted.trim_end_matches(" Mbps");
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
            prop_assert!(formatted.ends_with(" ms"));
            let numeric_part = formatted.trim_end_matches(" ms");
            if let Some(dot_pos) = numeric_part.find('.') {
                let decimal_places = numeric_part.len() - dot_pos - 1;
                prop_assert_eq!(decimal_places, 2);
            } else {
                prop_assert!(false, "No decimal point found in formatted latency");
            }
        }
    }

    proptest! {
        #[test]
        fn prop_speed_color_coding_fast(speed in 100.0f64..=f64::MAX) {
            if speed.is_finite() {
                prop_assert_eq!(speed_color(speed), Color::Green);
            }
        }

        #[test]
        fn prop_speed_color_coding_moderate(speed in 25.0f64..100.0f64) {
            prop_assert_eq!(speed_color(speed), Color::Yellow);
        }

        #[test]
        fn prop_speed_color_coding_slow(speed in f64::MIN..25.0f64) {
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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_minimal_mode_below_threshold(width in 0u16..60) {
            prop_assert!(
                is_minimal_mode(width),
                "Width {} should trigger minimal mode (< 60)",
                width
            );
        }

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
        assert!(!is_minimal_mode(60));
        assert!(is_minimal_mode(59));
        assert!(is_minimal_mode(40));
        assert!(!is_minimal_mode(80));
    }

    #[test]
    fn test_quality_color() {
        assert_eq!(quality_color(&QualityRating::Great), Color::Green);
        assert_eq!(quality_color(&QualityRating::Good), Color::LightGreen);
        assert_eq!(quality_color(&QualityRating::Average), Color::Yellow);
        assert_eq!(quality_color(&QualityRating::Poor), Color::Red);
    }
}
