extern crate clap;

mod cloudflare;
mod stats;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::{
    download::Download,
    locations::{Location, Locations},
    meta::{Meta, MetaRequest},
    upload::Upload,
};
use crate::stats::{median, quartile};
use chrono::{DateTime, Utc};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use log::{debug, info};
use serde::Serialize;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::join;
use tokio::time::Instant;

pub const LONG_VERSION: &str = concat!(
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

    #[command(flatten)]
    verbose: Verbosity,
}

#[derive(Serialize)]
struct SpeedTestResults<'a> {
    timestamp: DateTime<Utc>,
    meta: &'a Meta,
    location: &'a Location,
    latency: u128,
    jitter: u128,
    download_results: DownloadResults,
    download_speed: f64,
    upload_speed: f64,
}

#[derive(Serialize)]
struct DownloadResults {
    #[serde(rename(serialize = "100kb"))]
    _100kb: f64,
    #[serde(rename(serialize = "1MB"))]
    _1mb: f64,
    #[serde(rename(serialize = "10MB"))]
    _10mb: f64,
    #[serde(rename(serialize = "25MB"))]
    _25mb: f64,
    #[serde(rename(serialize = "100MB"))]
    _100mb: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = Arc::new(Mutex::new(io::stdout().lock()));
    let mut _stderr = io::stderr().lock();

    let cli: Cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let client = Client::new();

    let meta = client.send(MetaRequest {}).await?;

    let location = client.send(Locations {}).await?.get(&meta.colo);

    if !cli.json {
        writeln!(
            stdout.lock().unwrap(),
            "{} {} {}",
            "Server Location:".bold().white(),
            location.city.bright_blue(),
            format!("({})", meta.colo).bright_blue()
        )?;

        writeln!(
            stdout.lock().unwrap(),
            "{} {} {}",
            "Your network:\t".bold().white(),
            meta.as_organization.bright_blue(),
            format!("(AS{})", meta.asn).bright_blue()
        )?;

        writeln!(
            stdout.lock().unwrap(),
            "{} {} {}",
            "Your IP:\t".bold().white(),
            meta.client_ip.bright_blue(),
            format!("({})", meta.country).bright_blue()
        )?;
    }

    let measurements = download_measurements(&client).await;

    let (latency, jitter) =
        join!(measure_latency(&measurements), measure_jitter(&measurements));

    if !cli.json {
        writeln!(
            stdout.lock().unwrap(),
            "{} {}",
            "Latency:\t".bold().white(),
            format!("{} ms", latency.as_millis()).bright_red()
        )?;
        writeln!(
            stdout.lock().unwrap(),
            "{} {}",
            "Jitter:\t\t".bold().white(),
            format!("{} ms", jitter).bright_red()
        )?;
    }

    let (
        download_measurements_100kb,
        download_measurements_1mb,
        download_measurements_10mb,
        download_measurements_25mb,
        download_measurements_100mb,
    ) = join!(
        measure_download(&client, 1e+5 as usize, 10),
        measure_download(&client, 1e+6 as usize, 8),
        measure_download(&client, 1e+7 as usize, 6),
        measure_download(&client, 2.5e+7 as usize, 4),
        measure_download(&client, 1e+8 as usize, 3)
    );

    let download_measurements: Vec<Duration> = vec![
        download_measurements_100kb.as_slice(),
        download_measurements_1mb.as_slice(),
        download_measurements_10mb.as_slice(),
        download_measurements_25mb.as_slice(),
        download_measurements_100mb.as_slice(),
    ]
    .concat();

    let (
        upload_measurements_100kb,
        upload_measurements_1mb,
        upload_measurements_10mb,
        upload_measurements_25mb,
        upload_measurements_50mb,
    ) = join!(
        measure_upload(&client, 1e+5 as usize, 8),
        measure_upload(&client, 1e+6 as usize, 6),
        measure_upload(&client, 1e+7 as usize, 4),
        measure_upload(&client, 2.5e+7 as usize, 4),
        measure_upload(&client, 5e+7 as usize, 3)
    );

    let upload_measurements: Vec<Duration> = vec![
        upload_measurements_100kb.as_slice(),
        upload_measurements_1mb.as_slice(),
        upload_measurements_10mb.as_slice(),
        upload_measurements_25mb.as_slice(),
        upload_measurements_50mb.as_slice(),
    ]
    .concat();

    let results = SpeedTestResults {
        timestamp: Utc::now(),
        meta: &meta,
        location: &location,
        latency: latency.as_millis(),
        jitter,
        download_results: DownloadResults {
            _100kb: median(&download_measurements_100kb),
            _1mb: median(&download_measurements_1mb),
            _10mb: median(&download_measurements_10mb),
            _25mb: median(&download_measurements_25mb),
            _100mb: median(&download_measurements_100mb),
        },
        download_speed: quartile(&download_measurements, 0.9),
        upload_speed: quartile(&upload_measurements, 0.9),
    };

    if cli.json {
        writeln!(
            stdout.lock().unwrap(),
            "{}",
            if !cli.pretty {
                serde_json::to_string(&results)?
            } else {
                serde_json::to_string_pretty(&results)?
            }
        )?;

        return Ok(());
    }

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "100kB speed:\t".bold().white(),
        format!("{:.2} Mbps", median(&download_measurements_100kb)).yellow()
    )?;

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "1MB speed:\t".bold().white(),
        format!("{:.2} Mbps", median(&download_measurements_1mb)).yellow()
    )?;

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "10MB speed:\t".bold().white(),
        format!("{:.2} Mbps", median(&download_measurements_10mb)).yellow()
    )?;

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "25MB speed:\t".bold().white(),
        format!("{:.2} Mbps", median(&download_measurements_25mb)).yellow()
    )?;

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "100MB speed:\t".bold().white(),
        format!("{:.2} Mbps", median(&download_measurements_100mb)).yellow()
    )?;

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "Download speed:\t".bold().white(),
        format!("{:.2} Mbps", quartile(&download_measurements, 0.9))
            .bright_cyan()
    )?;

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "Upload speed:\t".bold().white(),
        format!("{:.2} Mbps", quartile(&upload_measurements, 0.9))
            .bright_cyan()
    )?;

    Ok(())
}

async fn measure_latency(measurements: &Vec<Duration>) -> Duration {
    let latency = measurements
        .iter()
        .fold(Duration::new(0, 0), |latency, &measurement| {
            latency + measurement
        })
        / ((measurements.len() as u32) * 2);

    latency
}

async fn measure_jitter(measurements: &Vec<Duration>) -> u128 {
    let jitters: Vec<u128> = measurements
        .windows(2)
        .map(|pair| pair[0].abs_diff(pair[1]))
        .map(|duration| duration.as_millis())
        .collect();

    jitters.iter().sum::<u128>() / jitters.len() as u128
}

async fn download_measurements(client: &Client) -> Vec<Duration> {
    let mut measurements = vec![];

    // Execute 20 requests async
    for _ in 0..20 {
        let start = Instant::now();
        let result = client.send(Download { bytes: 0 }).await;
        let _ = result.unwrap().bytes().last();
        let measurement = Instant::now().duration_since(start);

        info!("Trip calculated to {} ms", measurement.as_millis());

        measurements.push(measurement);
    }

    measurements
}

async fn measure_download(
    client: &Client,
    bytes: usize,
    iterations: usize,
) -> Vec<Duration> {
    let mut downloads = vec![];
    let download_request = Download { bytes };

    for _ in 0..iterations {
        let start = Instant::now();
        debug!("Starting download {:#?}", start);
        let _ = client.send(download_request).await;
        let finish = Instant::now();
        debug!("Finished download {:#?}", finish);
        let measurement = finish.duration_since(start);

        info!("Trip calculated to {} ms", measurement.as_millis());

        downloads.push(measurement);
    }

    downloads
}

async fn measure_upload(
    client: &Client,
    bytes: usize,
    iterations: usize,
) -> Vec<Duration> {
    let mut uploads = vec![];
    let upload_request = Upload::new(bytes);

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = client.send(&upload_request).await;
        let measurement = Instant::now().duration_since(start);
        uploads.push(measurement);
    }

    uploads
}
