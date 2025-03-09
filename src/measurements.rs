use crate::stats::median_f64;
use std::time::Duration;

pub async fn latency(measurements: &Vec<Duration>) -> Duration {
    let latency = measurements
        .iter()
        .fold(Duration::new(0, 0), |latency, &measurement| {
            latency + measurement
        })
        / ((measurements.len() as u32) * 2);

    latency
}

pub async fn latency_f64(measurements: &Vec<f64>) -> f64 {
    let mut measurements = measurements.clone();

    median_f64(&mut measurements).unwrap()
}

pub async fn jitter(measurements: &Vec<Duration>) -> u128 {
    let jitters: Vec<u128> = measurements
        .windows(2)
        .map(|pair| pair[0].abs_diff(pair[1]))
        .map(|duration| duration.as_millis())
        .collect();

    jitters.iter().sum::<u128>() / jitters.len() as u128
}

pub async fn jitter_f64(measurements: &Vec<f64>) -> f64 {
    let jitters: Vec<_> = measurements
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .collect();

    jitters.iter().sum::<f64>() / jitters.len() as f64
}
