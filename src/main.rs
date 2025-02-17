extern crate clap;

mod cloudflare;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::{download::Download, locations::Locations, trace::TraceRequest};
use clap::Parser;
use std::time::Duration;
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

    println!("Server Location {} ({})", location.city, trace.colo);
    println!("Your IP {} ({})", trace.ip, trace.loc);
    println!("Latency: {} ms", latency.as_millis());
    println!("Jitter: {} ms", jitter);

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
