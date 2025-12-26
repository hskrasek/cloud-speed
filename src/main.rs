extern crate clap;

mod cloudflare;
pub mod errors;
mod measurements;
pub mod results;
pub mod retry;
mod scoring;
mod stats;

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
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use std::io::{self, Write};
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

    let exit_code = match run_speed_test(&cli).await {
        Ok(()) => exit_codes::SUCCESS,
        Err(e) => {
            let error = create_user_error(e.as_ref());
            print_error(&error, cli.json);
            error.exit_code()
        }
    };

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
    let engine = TestEngine::new(TestConfig::default());
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
