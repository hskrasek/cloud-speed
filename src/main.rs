extern crate clap;

mod cloudflare;
mod stats;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::{
    download::Download, locations::Locations, trace::TraceRequest, upload::Upload,
};
use crate::stats::{median, quartile};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use log::{debug, info};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::join;
use tokio::time::Instant;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity,

    #[arg(short, long, default_value_t = false)]
    json: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = Arc::new(Mutex::new(io::stdout().lock()));
    let mut stderr = io::stderr().lock();

    let _: Cli = Cli::parse();

    let client = Client::new();

    let trace = client.send(TraceRequest {}).await?;

    let location = client.send(Locations {}).await?.get(&trace.colo);

    let measurements = download_measurements(&client).await;

    let (latency, jitter) = join!(
        measure_latency(&measurements),
        measure_jitter(&measurements)
    );

    writeln!(
        stdout.lock().unwrap(),
        "{} {} {}",
        "Server Location:".bold().white(),
        location.city.bright_blue(),
        format!("({})", trace.colo).bright_blue()
    )?;
    writeln!(
        stdout.lock().unwrap(),
        "{} {} {}",
        "Your IP:\t".bold().white(),
        trace.ip.bright_blue(),
        format!("({})", trace.loc).bright_blue()
    )?;
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
        measure_download(&client, 1e+8 as usize, 1)
    );

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
        format!("{:.2} MB", median(&download_measurements_1mb)).yellow()
    )?;
    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "10MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_10mb)).yellow()
    )?;
    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "25MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_25mb)).yellow()
    )?;
    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "100MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_100mb)).yellow()
    )?;

    let download_measurements: Vec<Duration> = vec![
        download_measurements_100kb.as_slice(),
        download_measurements_1mb.as_slice(),
        download_measurements_10mb.as_slice(),
        download_measurements_25mb.as_slice(),
        download_measurements_100mb.as_slice(),
    ]
    .concat();

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "Download speed:\t".bold().white(),
        format!("{:.2} Mbps", quartile(&download_measurements, 0.9)).bright_cyan()
    )?;

    let (upload_measurements_10kb, upload_measurements_100kb, upload_measurements_1mb) = join!(
        measure_upload(&client, 1e+4 as usize, 10),
        measure_upload(&client, 1e+5 as usize, 10),
        measure_upload(&client, 1e+6 as usize, 8)
    );

    let upload_measurements: Vec<Duration> = vec![
        upload_measurements_10kb.as_slice(),
        upload_measurements_100kb.as_slice(),
        upload_measurements_1mb.as_slice(),
    ]
    .concat();

    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "Upload speed:\t".bold().white(),
        format!("{:.2} Mbps", quartile(&upload_measurements, 0.9)).bright_cyan()
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
        .map(|pair| pair[0].abs_diff(pair[1]).as_millis())
        .collect();

    jitters.iter().sum::<u128>() / jitters.len() as u128
}

async fn download_measurements(client: &Client) -> Vec<Duration> {
    let mut measurements = vec![];

    for _ in 0..20 {
        let start = Instant::now();
        let _ = client.send(Download { bytes: 0 }).await;
        let measurement = Instant::now().duration_since(start);

        measurements.push(measurement);
    }

    measurements
}

async fn measure_download(client: &Client, bytes: usize, iterations: usize) -> Vec<Duration> {
    let mut downloads = vec![];
    let download_request = Download { bytes };

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = client.send(download_request).await;
        let measurement = Instant::now().duration_since(start);
        downloads.push(measurement);
    }

    downloads
}

async fn measure_upload(client: &Client, bytes: usize, iterations: usize) -> Vec<Duration> {
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
