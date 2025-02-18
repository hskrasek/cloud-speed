extern crate clap;

mod cloudflare;
mod stats;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::{
    download::Download, locations::Locations, trace::TraceRequest, upload::Upload,
};
use crate::stats::{median, quartile};
use clap::Parser;
use colored::Colorize;
use std::time::Duration;
use tokio::join;
use tokio::time::Instant;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _: Cli = Cli::parse();

    let client = Client::new();

    let trace = client.send(TraceRequest {}).await?;

    let location = client.send(Locations {}).await?.get(&trace.colo);

    let measurements = download_measurements(&client).await;

    let latency = measure_latency(&measurements).await;
    let jitter = measure_jitter(&measurements).await;

    println!(
        "{} {} {}",
        "Server Location:".bold().white(),
        location.city.bright_blue(),
        format!("({})", trace.colo).bright_blue()
    );
    println!(
        "{} {} {}",
        "Your IP:\t".bold().white(),
        trace.ip.bright_blue(),
        format!("({})", trace.loc).bright_blue()
    );
    println!("{} {}", "Latency:\t".bold().white(), format!("{} ms", latency.as_millis()).bright_red());
    println!("{} {}", "Jitter:\t\t".bold().white(), format!("{} ms", jitter).bright_red());

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

    println!(
        "{} {}",
        "100kB speed:\t".bold().white(),
        format!("{:.2} Mbps", median(&download_measurements_100kb)).yellow()
    );
    println!(
        "{} {}",
        "1MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_1mb)).yellow()
    );
    println!(
        "{} {}",
        "10MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_10mb)).yellow()
    );
    println!(
        "{} {}",
        "25MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_25mb)).yellow()
    );
    println!(
        "{} {}",
        "100MB speed:\t".bold().white(),
        format!("{:.2} MB", median(&download_measurements_100mb)).yellow()
    );

    let download_measurements: Vec<Duration> = vec![
        download_measurements_100kb.as_slice(),
        download_measurements_1mb.as_slice(),
        download_measurements_10mb.as_slice(),
        download_measurements_25mb.as_slice(),
        download_measurements_100mb.as_slice(),
    ]
    .concat();

    println!(
        "{} {}",
        "Download speed:\t".bold().white(),
        format!("{:.2} Mbps", quartile(&download_measurements, 0.9)).bright_cyan()
    );

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

    println!(
        "{} {}",
        "Upload speed:\t".bold().white(),
        format!("{:.2} Mbps", quartile(&upload_measurements, 0.9)).bright_cyan()
    );

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
