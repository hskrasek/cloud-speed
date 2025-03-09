extern crate clap;

mod cloudflare;
mod measurements;
mod stats;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::{
    locations::{Location, Locations},
    meta::{Meta, MetaRequest},
    upload::Upload,
};
use crate::cloudflare::tests::download::Download as DownloadTest;
use crate::cloudflare::tests::{Test, TestResults};
use crate::measurements::{jitter, jitter_f64, latency, latency_f64};
use crate::stats::{median, median_f64, quartile};
use chrono::{DateTime, Utc};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use futures::future::join_all;
use log::{debug, info};
use serde::Serialize;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::join;
use tokio::time::Instant;

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

    let measurements = latency_measurements().await;

    let (latency, jitter) =
        join!(latency_f64(&measurements), jitter_f64(&measurements));

    if !cli.json {
        writeln!(
            stdout.lock().unwrap(),
            "{} {}",
            "Latency:\t".bold().white(),
            format!("{:.2} ms", latency).bright_red()
        )?;

        writeln!(
            stdout.lock().unwrap(),
            "{} {}",
            "Jitter:\t\t".bold().white(),
            format!("{:.2} ms", jitter).bright_red()
        )?;
    }

    let (
        mut download_measurements_100kb,
        mut download_measurements_1mb,
        // download_measurements_10mb,
        // download_measurements_25mb,
        // download_measurements_100mb,
    ) = join!(
        measure_download(100_000, 10),
        measure_download(1_000_000, 8),
        // measure_download(10_000_000, 6),
        // measure_download(25_000_000, 4),
        // measure_download(100_000_000, 3)
    );

    // let download_measurements: Vec<Duration> = vec![
    //     download_measurements_100kb.as_slice(),
    //     download_measurements_1mb.as_slice(),
    //     download_measurements_10mb.as_slice(),
    //     download_measurements_25mb.as_slice(),
    //     download_measurements_100mb.as_slice(),
    // ]
    // .concat();

    // let (
    //     upload_measurements_100kb,
    //     upload_measurements_1mb,
    //     upload_measurements_10mb,
    //     upload_measurements_25mb,
    //     upload_measurements_50mb,
    // ) = join!(
    //     measure_upload(&client, 1e+5 as usize, 8),
    //     measure_upload(&client, 1e+6 as usize, 6),
    //     measure_upload(&client, 1e+7 as usize, 4),
    //     measure_upload(&client, 2.5e+7 as usize, 4),
    //     measure_upload(&client, 5e+7 as usize, 3)
    // );
    //
    // let upload_measurements: Vec<Duration> = vec![
    //     upload_measurements_100kb.as_slice(),
    //     upload_measurements_1mb.as_slice(),
    //     upload_measurements_10mb.as_slice(),
    //     upload_measurements_25mb.as_slice(),
    //     upload_measurements_50mb.as_slice(),
    // ]
    // .concat();

    // let results = SpeedTestResults {
    //     timestamp: Utc::now(),
    //     meta: &meta,
    //     location: &location,
    //     latency: latency.as_millis(),
    //     jitter,
    //     download_results: DownloadResults {
    //         _100kb: median(&download_measurements_100kb),
    //         _1mb: median(&download_measurements_1mb),
    //         _10mb: median(&download_measurements_10mb),
    //         _25mb: median(&download_measurements_25mb),
    //         _100mb: median(&download_measurements_100mb),
    //     },
    //     download_speed: quartile(&download_measurements, 0.9),
    //     upload_speed: quartile(&upload_measurements, 0.9),
    // };
    //
    // if cli.json {
    //     writeln!(
    //         stdout.lock().unwrap(),
    //         "{}",
    //         if !cli.pretty {
    //             serde_json::to_string(&results)?
    //         } else {
    //             serde_json::to_string_pretty(&results)?
    //         }
    //     )?;
    //
    //     return Ok(());
    // }
    dbg!(&download_measurements_100kb);
    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "100kB speed:\t".bold().white(),
        format!(
            "{:.2} Mbps",
            measure_speed(
                100_000.0,
                median_f64(&mut download_measurements_100kb).unwrap()
            )
        )
        .yellow()
    )?;
    dbg!(&download_measurements_1mb);
    writeln!(
        stdout.lock().unwrap(),
        "{} {}",
        "1MB speed:\t".bold().white(),
        format!(
            "{:.2} Mbps",
            measure_speed(
                1_000_000.0,
                median_f64(&mut download_measurements_1mb).unwrap()
            )
        )
        .yellow()
    )?;

    // writeln!(
    //     stdout.lock().unwrap(),
    //     "{} {}",
    //     "10MB speed:\t".bold().white(),
    //     format!("{:.2} Mbps", median(&download_measurements_10mb)).yellow()
    // )?;
    //
    // writeln!(
    //     stdout.lock().unwrap(),
    //     "{} {}",
    //     "25MB speed:\t".bold().white(),
    //     format!("{:.2} Mbps", median(&download_measurements_25mb)).yellow()
    // )?;
    //
    // writeln!(
    //     stdout.lock().unwrap(),
    //     "{} {}",
    //     "100MB speed:\t".bold().white(),
    //     format!("{:.2} Mbps", median(&download_measurements_100mb)).yellow()
    // )?;
    //
    // writeln!(
    //     stdout.lock().unwrap(),
    //     "{} {}",
    //     "Download speed:\t".bold().white(),
    //     format!("{:.2} Mbps", quartile(&download_measurements, 0.9))
    //         .bright_cyan()
    // )?;
    //
    // writeln!(
    //     stdout.lock().unwrap(),
    //     "{} {}",
    //     "Upload speed:\t".bold().white(),
    //     format!("{:.2} Mbps", quartile(&upload_measurements, 0.9))
    //         .bright_cyan()
    // )?;

    Ok(())
}

async fn latency_measurements() -> Vec<f64> {
    let mut tests: Vec<_> = Vec::new();

    for _ in 0..10 {
        tests.push((DownloadTest {}).run(1000));
    }

    let futures = tests.into_iter().map(|test| async move { test.await });

    let results: Result<Vec<_>, _> =
        join_all(futures).await.into_iter().collect();

    let results = match results {
        Ok(results) => results,
        Err(error) => panic!("{:?}", error),
    };

    results
        .into_iter()
        .map(|result| {
            let measurement = result.tcp_duration;

            info!("Latency Measurement: {} ms", measurement.as_millis());

            return measurement.as_secs_f64() * 1000.0;
        })
        .collect::<Vec<f64>>()
}

//          0           1       2               3           4       5       6
// resolve([started, dnsLookup, tcpHandshake, sslHandshake, ttfb, ended, parseFloat(res.headers["server-timing"].slice(22))]);
async fn measure_download(bytes: u64, iterations: usize) -> Vec<f64> {
    let mut tests: Vec<_> = Vec::with_capacity(iterations);

    for _ in 0..tests.capacity() {
        tests.push((DownloadTest {}).run(bytes));
    }

    let futures = tests.into_iter().map(|test| async move { test.await });

    let results: Result<Vec<_>, _> =
        join_all(futures).await.into_iter().collect();

    let results = match results {
        Ok(results) => results,
        Err(error) => panic!("{:?}", error),
    };

    results
        .into_iter()
        .map(|result| {
            info!("{:#?}", result);
            let measurement = result.end_duration - result.ttfb_duration;

            info!("Download duration: {} ms", measurement.as_millis());

            measurement.as_secs_f64() * 1000.0
        })
        .collect::<Vec<f64>>()
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

fn measure_speed(bytes: f64, duration: f64) -> f64 {
    (bytes * 8.0) / (duration / 1000.0) / 1e6
}
