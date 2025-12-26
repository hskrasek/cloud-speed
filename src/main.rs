extern crate clap;

mod cloudflare;
pub mod errors;
mod measurements;
pub mod results;
pub mod retry;
mod scoring;
mod stats;
mod tui;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::{locations::Locations, meta::MetaRequest};
use crate::cloudflare::tests::engine::{TestConfig, TestEngine};
use crate::cloudflare::tests::packet_loss::{
    run_packet_loss_test_safe, PacketLossConfig,
};
use crate::errors::{
    classify_error, exit_codes, format_error_for_display, ErrorKind,
    SpeedTestError,
};
use crate::results::{
    AimScoresOutput, BandwidthResults, ConnectionMeta, LatencyResults,
    PacketLossResults, ServerLocation, SizeMeasurement, SpeedTestResults,
};
use crate::scoring::{calculate_aim_scores, ConnectionMetrics, QualityScore};
use crate::tui::state::{ConnectionInfo, ServerInfo};
use crate::tui::{DisplayMode, TuiController};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use std::io::{self, IsTerminal, Write};
use std::process;

const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CLOUDSPEED_BUILD_GIT_HASH"),
    ")"
);

#[derive(Parser)]
#[command(author, version, about, long_about = None, long_version = LONG_VERSION)]
struct Cli {
    /// Print results in json format
    #[arg(short, long, default_value_t = false)]
    json: bool,

    /// Only applies when json is active.
    /// Pretty prints JSON on output
    #[arg(short, long, default_value_t = false)]
    pretty: bool,

    /// TURN server URI for packet loss measurement (e.g., turn:example.com:3478)
    #[arg(long)]
    turn_server: Option<String>,

    #[command(flatten)]
    verbose: Verbosity,
}

impl Cli {
    /// Get the packet loss configuration if TURN server is provided.
    fn packet_loss_config(&self) -> Option<PacketLossConfig> {
        self.turn_server
            .as_ref()
            .map(|uri| PacketLossConfig::new(uri.clone()))
    }
}

#[tokio::main]
async fn main() {
    let cli: Cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    // Detect display mode based on CLI flags and terminal capabilities
    let is_tty = io::stdout().is_terminal();
    let display_mode = DisplayMode::detect(cli.json, is_tty);

    // Create TUI controller
    let mut tui = match TuiController::new(display_mode) {
        Ok(tui) => tui,
        Err(e) => {
            // Fall back to silent mode if TUI initialization fails
            eprintln!("Warning: TUI initialization failed: {}", e);
            TuiController::new(DisplayMode::Silent)
                .expect("Silent mode should always succeed")
        }
    };

    // Initialize TUI (enters alternate screen in TUI mode)
    if let Err(e) = tui.init() {
        eprintln!("Warning: TUI init failed: {}", e);
    }

    let exit_code = match run_speed_test_with_tui(&cli, &mut tui).await {
        Ok(()) => exit_codes::SUCCESS,
        Err(e) => {
            let error = create_user_error(e.as_ref());

            // In TUI mode, display error in the TUI before cleanup
            if tui.mode() == DisplayMode::Tui {
                // Set error state in TUI to display with red styling
                tui.set_error(
                    error.message.clone(),
                    error.suggestion.clone(),
                );
                // Render the error in TUI
                let _ = tui.render();
                // Wait a moment for user to see the error
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }

            // Clean up TUI before printing error to terminal
            let _ = tui.cleanup();
            print_error(&error, cli.json);
            error.exit_code()
        }
    };

    // Clean up TUI (restores terminal state)
    let _ = tui.cleanup();

    process::exit(exit_code);
}

/// Create a user-friendly error from a generic error.
fn create_user_error(
    error: &(dyn std::error::Error + 'static),
) -> SpeedTestError {
    let kind = classify_error(error);
    let message = error.to_string();

    match kind {
        ErrorKind::Network => SpeedTestError::network(format!(
            "Failed to connect to speed.cloudflare.com: {}",
            message
        )),
        ErrorKind::Dns => SpeedTestError::dns(format!(
            "Failed to resolve speed.cloudflare.com: {}",
            message
        )),
        ErrorKind::Timeout => SpeedTestError::timeout(format!(
            "Connection timed out: {}",
            message
        )),
        ErrorKind::Tls => SpeedTestError::tls(format!(
            "TLS/SSL connection failed: {}",
            message
        )),
        ErrorKind::Api => {
            SpeedTestError::api(format!("Cloudflare API error: {}", message))
        }
        _ => SpeedTestError::new(kind, message),
    }
}

/// Print an error message to stderr.
fn print_error(error: &SpeedTestError, json_mode: bool) {
    if json_mode {
        // Output error as JSON
        let error_json = serde_json::json!({
            "error": {
                "kind": format!("{:?}", error.kind),
                "message": error.message,
                "suggestion": error.suggestion,
            }
        });
        eprintln!(
            "{}",
            serde_json::to_string(&error_json).unwrap_or_default()
        );
    } else {
        // Output human-readable error
        eprintln!("{}", format_error_for_display(error).red());
    }
}

/// Run the speed test and return a result.
async fn run_speed_test(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // Fetch connection metadata
    let meta = client
        .send(MetaRequest {})
        .await
        .map_err(|e| format!("Failed to fetch connection metadata: {}", e))?;

    let location = client
        .send(Locations {})
        .await
        .map_err(|e| format!("Failed to fetch server locations: {}", e))?
        .get(&meta.colo);

    // Display metadata (human-readable mode only)
    if !cli.json {
        print_metadata(&meta, &location)?;
    }

    // Run the test engine
    let engine = TestEngine::new(TestConfig::default(), None);
    let output =
        engine.run().await.map_err(|e| format!("Speed test failed: {}", e))?;

    // Run packet loss test if configured
    let packet_loss_config = cli.packet_loss_config();
    let packet_loss_result =
        run_packet_loss_test_safe(packet_loss_config).await;

    // Build result structures
    let server =
        ServerLocation::new(location.city.clone(), location.iata.clone());
    let connection = ConnectionMeta::new(
        meta.client_ip.clone(),
        meta.country.clone(),
        meta.as_organization.clone(),
        meta.asn,
    );

    let latency = LatencyResults::new(
        output.latency.idle_ms,
        output.latency.idle_jitter_ms,
        output.latency.loaded_down_ms,
        output.latency.loaded_down_jitter_ms,
        output.latency.loaded_up_ms,
        output.latency.loaded_up_jitter_ms,
    );

    let download = BandwidthResults::new(
        output.download.speed_mbps,
        output
            .download
            .measurements
            .iter()
            .map(|m| SizeMeasurement::new(m.bytes, m.speed_mbps, m.count))
            .collect(),
        output.download.early_terminated,
    );

    let upload = BandwidthResults::new(
        output.upload.speed_mbps,
        output
            .upload
            .measurements
            .iter()
            .map(|m| SizeMeasurement::new(m.bytes, m.speed_mbps, m.count))
            .collect(),
        output.upload.early_terminated,
    );

    let packet_loss = if packet_loss_result.is_available() {
        Some(PacketLossResults::new(
            packet_loss_result.packet_loss_ratio,
            packet_loss_result.packets_sent,
            packet_loss_result.packets_lost,
            packet_loss_result.packets_received,
            packet_loss_result.avg_rtt_ms,
        ))
    } else {
        None
    };

    // Calculate AIM scores
    let metrics = ConnectionMetrics::new(
        download.speed_mbps,
        upload.speed_mbps,
        latency.idle_ms,
        latency.idle_jitter_ms.unwrap_or(0.0),
    )
    .with_loaded_latency(latency.loaded_down_ms, latency.loaded_up_ms);

    let metrics = if let Some(ref pl) = packet_loss {
        metrics.with_packet_loss(pl.ratio)
    } else {
        metrics
    };

    let aim_scores = calculate_aim_scores(&metrics);
    let scores = AimScoresOutput::from_aim_scores(&aim_scores);

    let results = SpeedTestResults::new(
        server,
        connection,
        latency.clone(),
        download.clone(),
        upload.clone(),
        packet_loss.clone(),
        scores,
    );

    // Output results
    if cli.json {
        print_json_output(&results, cli.pretty)?;
    } else {
        print_human_output(
            &latency,
            &download,
            &upload,
            &packet_loss,
            &aim_scores,
        )?;
    }

    Ok(())
}

/// Run the speed test with TUI integration.
///
/// This function integrates the TuiController for real-time progress display.
/// In TUI mode, it shows live updates during the test. In JSON mode, it
/// suppresses all output until the final JSON result.
///
/// # Requirements
/// _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3_
async fn run_speed_test_with_tui(
    cli: &Cli,
    tui: &mut TuiController,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // Fetch connection metadata
    let meta = client
        .send(MetaRequest {})
        .await
        .map_err(|e| format!("Failed to fetch connection metadata: {}", e))?;

    let location = client
        .send(Locations {})
        .await
        .map_err(|e| format!("Failed to fetch server locations: {}", e))?
        .get(&meta.colo);

    // Set metadata in TUI
    let server_info = ServerInfo {
        city: location.city.clone(),
        iata: location.iata.clone(),
    };
    let connection_info = ConnectionInfo {
        ip: meta.client_ip.clone(),
        country: meta.country.clone(),
        isp: meta.as_organization.clone(),
        asn: meta.asn,
    };
    tui.set_metadata(server_info, connection_info);

    // Initial render to show metadata
    tui.render()?;

    // Get progress callback for the test engine
    let progress_callback = tui.progress_callback();

    // Run the test engine with progress callback
    let engine = TestEngine::new(TestConfig::default(), Some(progress_callback));

    // Create a render loop that updates the TUI during test execution
    let output = run_test_with_render_loop(&engine, tui).await?;

    // Run packet loss test if configured
    let packet_loss_config = cli.packet_loss_config();
    let packet_loss_result =
        run_packet_loss_test_safe(packet_loss_config).await;

    // Build result structures
    let server =
        ServerLocation::new(location.city.clone(), location.iata.clone());
    let connection = ConnectionMeta::new(
        meta.client_ip.clone(),
        meta.country.clone(),
        meta.as_organization.clone(),
        meta.asn,
    );

    let latency = LatencyResults::new(
        output.latency.idle_ms,
        output.latency.idle_jitter_ms,
        output.latency.loaded_down_ms,
        output.latency.loaded_down_jitter_ms,
        output.latency.loaded_up_ms,
        output.latency.loaded_up_jitter_ms,
    );

    let download = BandwidthResults::new(
        output.download.speed_mbps,
        output
            .download
            .measurements
            .iter()
            .map(|m| SizeMeasurement::new(m.bytes, m.speed_mbps, m.count))
            .collect(),
        output.download.early_terminated,
    );

    let upload = BandwidthResults::new(
        output.upload.speed_mbps,
        output
            .upload
            .measurements
            .iter()
            .map(|m| SizeMeasurement::new(m.bytes, m.speed_mbps, m.count))
            .collect(),
        output.upload.early_terminated,
    );

    let packet_loss = if packet_loss_result.is_available() {
        Some(PacketLossResults::new(
            packet_loss_result.packet_loss_ratio,
            packet_loss_result.packets_sent,
            packet_loss_result.packets_lost,
            packet_loss_result.packets_received,
            packet_loss_result.avg_rtt_ms,
        ))
    } else {
        None
    };

    // Calculate AIM scores
    let metrics = ConnectionMetrics::new(
        download.speed_mbps,
        upload.speed_mbps,
        latency.idle_ms,
        latency.idle_jitter_ms.unwrap_or(0.0),
    )
    .with_loaded_latency(latency.loaded_down_ms, latency.loaded_up_ms);

    let metrics = if let Some(ref pl) = packet_loss {
        metrics.with_packet_loss(pl.ratio)
    } else {
        metrics
    };

    let aim_scores = calculate_aim_scores(&metrics);
    let scores = AimScoresOutput::from_aim_scores(&aim_scores);

    let results = SpeedTestResults::new(
        server,
        connection,
        latency.clone(),
        download.clone(),
        upload.clone(),
        packet_loss.clone(),
        scores,
    );

    // Output results based on display mode
    match tui.mode() {
        DisplayMode::Json => {
            // Clean up TUI before JSON output
            tui.cleanup()?;
            print_json_output(&results, cli.pretty)?;
        }
        DisplayMode::Tui => {
            // Show final results in TUI
            tui.show_results(&results)?;
            // Wait a moment for user to see results, then clean up
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            tui.cleanup()?;
            // Print human-readable summary after TUI cleanup
            print_human_output(
                &latency,
                &download,
                &upload,
                &packet_loss,
                &aim_scores,
            )?;
        }
        DisplayMode::Silent => {
            // Silent mode: just print human-readable output
            print_human_output(
                &latency,
                &download,
                &upload,
                &packet_loss,
                &aim_scores,
            )?;
        }
    }

    Ok(())
}

/// Run the test engine with a render loop for TUI updates.
///
/// This function runs the test engine while periodically rendering
/// the TUI to show progress updates.
async fn run_test_with_render_loop(
    engine: &TestEngine,
    tui: &mut TuiController,
) -> Result<crate::cloudflare::tests::engine::SpeedTestOutput, Box<dyn std::error::Error>>
{
    use tokio::select;
    use tokio::time::{interval, Duration};

    // Only run render loop in TUI mode
    if tui.mode() != DisplayMode::Tui {
        return engine.run().await;
    }

    // Create a render interval (60fps = ~16ms, but 100ms is fine for progress)
    let mut render_interval = interval(Duration::from_millis(100));

    // Spawn the test engine as a task
    let engine_future = engine.run();
    tokio::pin!(engine_future);

    loop {
        select! {
            // Test engine completed
            result = &mut engine_future => {
                // Final render
                let _ = tui.render();
                return result;
            }
            // Render tick
            _ = render_interval.tick() => {
                let _ = tui.render();
            }
        }
    }
}

/// Print connection metadata in human-readable format.
fn print_metadata(
    meta: &crate::cloudflare::requests::meta::Meta,
    location: &crate::cloudflare::requests::locations::Location,
) -> io::Result<()> {
    let mut stdout = io::stdout().lock();

    writeln!(
        stdout,
        "{} {} {}",
        "Server Location:".bold().white(),
        location.city.bright_blue(),
        format!("({})", location.iata).bright_blue()
    )?;

    writeln!(
        stdout,
        "{} {} {}",
        "Your network:\t".bold().white(),
        meta.as_organization.bright_blue(),
        format!("(AS{})", meta.asn).bright_blue()
    )?;

    writeln!(
        stdout,
        "{} {} {}",
        "Your IP:\t".bold().white(),
        meta.client_ip.bright_blue(),
        format!("({})", meta.country).bright_blue()
    )?;

    Ok(())
}

/// Print results in JSON format.
fn print_json_output(
    results: &SpeedTestResults,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();

    let json = if pretty {
        serde_json::to_string_pretty(results)?
    } else {
        serde_json::to_string(results)?
    };

    writeln!(stdout, "{}", json)?;

    Ok(())
}

/// Print results in human-readable format.
fn print_human_output(
    latency: &LatencyResults,
    download: &BandwidthResults,
    upload: &BandwidthResults,
    packet_loss: &Option<PacketLossResults>,
    aim_scores: &crate::scoring::AimScores,
) -> io::Result<()> {
    let mut stdout = io::stdout().lock();

    // Latency section
    writeln!(
        stdout,
        "{} {}",
        "Latency:\t".bold().white(),
        format!("{:.2} ms", latency.idle_ms).bright_red()
    )?;

    writeln!(
        stdout,
        "{} {}",
        "Jitter:\t\t".bold().white(),
        match latency.idle_jitter_ms {
            Some(j) => format!("{:.2} ms", j).bright_red(),
            None => "N/A".bright_red(),
        }
    )?;

    // Loaded latency (if available)
    if let Some(loaded_down) = latency.loaded_down_ms {
        writeln!(
            stdout,
            "{} {}",
            "Loaded (down):\t".bold().white(),
            format!("{:.2} ms", loaded_down).bright_red()
        )?;
    }

    if let Some(loaded_up) = latency.loaded_up_ms {
        writeln!(
            stdout,
            "{} {}",
            "Loaded (up):\t".bold().white(),
            format!("{:.2} ms", loaded_up).bright_red()
        )?;
    }

    writeln!(stdout)?;

    // Download speeds by size
    for measurement in &download.measurements {
        let size_label = format_size_label(measurement.bytes);
        writeln!(
            stdout,
            "{} {}",
            format!("{} speed:\t", size_label).bold().white(),
            format!("{:.2} Mbps", measurement.speed_mbps).yellow()
        )?;
    }

    // Final download speed
    writeln!(
        stdout,
        "{} {}",
        "Download speed:\t".bold().white(),
        format!("{:.2} Mbps", download.speed_mbps).bright_cyan()
    )?;

    writeln!(stdout)?;

    // Upload speeds by size
    for measurement in &upload.measurements {
        let size_label = format_size_label(measurement.bytes);
        writeln!(
            stdout,
            "{} {}",
            format!("{} up:\t", size_label).bold().white(),
            format!("{:.2} Mbps", measurement.speed_mbps).yellow()
        )?;
    }

    // Final upload speed
    writeln!(
        stdout,
        "{} {}",
        "Upload speed:\t".bold().white(),
        format!("{:.2} Mbps", upload.speed_mbps).bright_cyan()
    )?;

    writeln!(stdout)?;

    // Packet loss (if available)
    if let Some(pl) = packet_loss {
        writeln!(
            stdout,
            "{} {}",
            "Packet loss:\t".bold().white(),
            format!("{:.2}%", pl.percent).bright_magenta()
        )?;
        writeln!(stdout)?;
    }

    // AIM Scores
    writeln!(stdout, "{}", "Quality Scores:".bold().white())?;
    writeln!(
        stdout,
        "  {} {}",
        "Streaming:\t".white(),
        format_quality_score(&aim_scores.streaming)
    )?;
    writeln!(
        stdout,
        "  {} {}",
        "Gaming:\t\t".white(),
        format_quality_score(&aim_scores.gaming)
    )?;
    writeln!(
        stdout,
        "  {} {}",
        "Video Calls:\t".white(),
        format_quality_score(&aim_scores.video_conferencing)
    )?;

    Ok(())
}

/// Format a byte size into a human-readable label.
fn format_size_label(bytes: u64) -> String {
    match bytes {
        b if b >= 1_000_000_000 => format!("{}GB", b / 1_000_000_000),
        b if b >= 1_000_000 => format!("{}MB", b / 1_000_000),
        b if b >= 1_000 => format!("{}kB", b / 1_000),
        b => format!("{}B", b),
    }
}

/// Format a quality score with appropriate color.
fn format_quality_score(score: &QualityScore) -> colored::ColoredString {
    match score {
        QualityScore::Great => "Great".bright_green(),
        QualityScore::Good => "Good".green(),
        QualityScore::Average => "Average".yellow(),
        QualityScore::Poor => "Poor".red(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Helper function to create test SpeedTestResults
    fn create_test_results(
        download_speed: f64,
        upload_speed: f64,
        latency_ms: f64,
        jitter_ms: Option<f64>,
    ) -> SpeedTestResults {
        let server = ServerLocation::new(
            "Test City".to_string(),
            "TST".to_string(),
        );
        let connection = ConnectionMeta::new(
            "192.168.1.1".to_string(),
            "US".to_string(),
            "Test ISP".to_string(),
            12345,
        );
        let latency = LatencyResults::new(
            latency_ms,
            jitter_ms,
            None,
            None,
            None,
            None,
        );
        let download = BandwidthResults::new(download_speed, vec![], false);
        let upload = BandwidthResults::new(upload_speed, vec![], false);
        let scores = AimScoresOutput {
            streaming: "good".to_string(),
            gaming: "good".to_string(),
            video_conferencing: "good".to_string(),
            overall: "good".to_string(),
        };

        SpeedTestResults::new(
            server,
            connection,
            latency,
            download,
            upload,
            None,
            scores,
        )
    }

    // Helper to check for TUI escape sequences
    fn contains_escape_sequences(s: &str) -> bool {
        // Common ANSI escape sequences used by TUI libraries
        s.contains("\x1b[") || // CSI sequences
        s.contains("\x1b]") || // OSC sequences
        s.contains("\x1bP") || // DCS sequences
        s.contains("\x1b\\") || // ST sequences
        s.contains("\x1b(") || // Character set selection
        s.contains("\x1b)") || // Character set selection
        s.contains("\x1b*") || // Character set selection
        s.contains("\x1b+")    // Character set selection
    }

    // **Feature: tui-progress-display, Property 13: JSON Mode Output Correctness**
    // **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any valid SpeedTestResults, when serialized to JSON:
        /// - The output SHALL be valid JSON
        /// - The JSON SHALL contain all required fields
        /// - No TUI escape sequences SHALL appear in the output
        #[test]
        fn json_output_is_valid_and_complete(
            download_speed in 0.0f64..1000.0,
            upload_speed in 0.0f64..1000.0,
            latency_ms in 0.1f64..500.0,
            jitter_ms in proptest::option::of(0.1f64..100.0)
        ) {
            let results = create_test_results(
                download_speed,
                upload_speed,
                latency_ms,
                jitter_ms,
            );

            // Serialize to JSON (non-pretty)
            let json = serde_json::to_string(&results);
            prop_assert!(
                json.is_ok(),
                "Serialization should succeed"
            );
            let json_str = json.unwrap();

            // Verify it's valid JSON by parsing it
            let parsed: Result<serde_json::Value, _> =
                serde_json::from_str(&json_str);
            prop_assert!(
                parsed.is_ok(),
                "Output should be valid JSON: {}",
                json_str
            );

            // Verify required fields are present
            let value = parsed.unwrap();
            prop_assert!(
                value.get("timestamp").is_some(),
                "JSON should contain timestamp field"
            );
            prop_assert!(
                value.get("server").is_some(),
                "JSON should contain server field"
            );
            prop_assert!(
                value.get("connection").is_some(),
                "JSON should contain connection field"
            );
            prop_assert!(
                value.get("latency").is_some(),
                "JSON should contain latency field"
            );
            prop_assert!(
                value.get("download").is_some(),
                "JSON should contain download field"
            );
            prop_assert!(
                value.get("upload").is_some(),
                "JSON should contain upload field"
            );
            prop_assert!(
                value.get("scores").is_some(),
                "JSON should contain scores field"
            );

            // Verify no TUI escape sequences
            prop_assert!(
                !contains_escape_sequences(&json_str),
                "JSON output should not contain TUI escape sequences"
            );
        }

        /// Property: Pretty-printed JSON is also valid and deserializable
        #[test]
        fn pretty_json_output_is_valid(
            download_speed in 0.0f64..1000.0,
            upload_speed in 0.0f64..1000.0,
            latency_ms in 0.1f64..500.0
        ) {
            let results = create_test_results(
                download_speed,
                upload_speed,
                latency_ms,
                Some(latency_ms * 0.1),
            );

            // Serialize to pretty JSON
            let json = serde_json::to_string_pretty(&results);
            prop_assert!(
                json.is_ok(),
                "Pretty serialization should succeed"
            );
            let json_str = json.unwrap();

            // Verify it's valid JSON
            let parsed: Result<serde_json::Value, _> =
                serde_json::from_str(&json_str);
            prop_assert!(
                parsed.is_ok(),
                "Pretty output should be valid JSON"
            );

            // Verify no TUI escape sequences
            prop_assert!(
                !contains_escape_sequences(&json_str),
                "Pretty JSON should not contain TUI escape sequences"
            );
        }

        /// Property: JSON error output is valid JSON
        #[test]
        fn json_error_output_is_valid(
            error_message in "[a-zA-Z0-9 ]{1,100}",
            suggestion in proptest::option::of("[a-zA-Z0-9 ]{1,50}")
        ) {
            let error = SpeedTestError::new(
                ErrorKind::Network,
                error_message.clone(),
            );

            // Create error JSON as print_error does
            let error_json = serde_json::json!({
                "error": {
                    "kind": format!("{:?}", error.kind),
                    "message": error.message,
                    "suggestion": suggestion,
                }
            });

            let json_str = serde_json::to_string(&error_json);
            prop_assert!(
                json_str.is_ok(),
                "Error JSON serialization should succeed"
            );
            let json_str = json_str.unwrap();

            // Verify it's valid JSON
            let parsed: Result<serde_json::Value, _> =
                serde_json::from_str(&json_str);
            prop_assert!(
                parsed.is_ok(),
                "Error output should be valid JSON"
            );

            // Verify no TUI escape sequences
            prop_assert!(
                !contains_escape_sequences(&json_str),
                "Error JSON should not contain TUI escape sequences"
            );
        }
    }

    // Unit tests for JSON output
    #[test]
    fn test_json_output_contains_required_fields() {
        let results = create_test_results(100.0, 50.0, 15.0, Some(2.0));
        let json_str = serde_json::to_string(&results).unwrap();

        // Verify required fields are present
        assert!(json_str.contains("\"timestamp\""));
        assert!(json_str.contains("\"server\""));
        assert!(json_str.contains("\"connection\""));
        assert!(json_str.contains("\"latency\""));
        assert!(json_str.contains("\"download\""));
        assert!(json_str.contains("\"upload\""));
        assert!(json_str.contains("\"scores\""));
    }

    #[test]
    fn test_json_output_no_escape_sequences() {
        let results = create_test_results(100.0, 50.0, 15.0, Some(2.0));
        let json_str = serde_json::to_string(&results).unwrap();

        assert!(
            !contains_escape_sequences(&json_str),
            "JSON should not contain escape sequences"
        );
    }

    #[test]
    fn test_display_mode_json_suppresses_tui() {
        // When json_flag is true, DisplayMode should be Json
        let mode = DisplayMode::detect(true, true);
        assert_eq!(mode, DisplayMode::Json);

        let mode = DisplayMode::detect(true, false);
        assert_eq!(mode, DisplayMode::Json);
    }
}
