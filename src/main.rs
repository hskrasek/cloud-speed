#[allow(dead_code)]
extern crate clap;

mod cloudflare;

use crate::cloudflare::client::Client;
use crate::cloudflare::requests::trace::TraceRequest;
use clap::Parser;
use tokio::time::Duration;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _: Cli = Cli::parse();

    let client = Client::new();

    //TODO: Add custom UA header to these request
    let request = TraceRequest {};
    let trace = client.send(request).await?;

    // let location = locations.get(&trace.colo).unwrap();

    // println!("Server Location {} ({})", location.city, trace.colo);
    println!("Your IP {} ({})", trace.ip, trace.loc);

    Ok(())
}

fn measure_speed(bytes: usize, duration: Duration) -> f64 {
    let seconds = duration.as_secs_f64();
    let bits = (bytes * 8) as f64;

    (bits / seconds) * 1_000_000.0
}

fn download(bytes: i64) -> () {
    let client = reqwest::Client::new();
}
