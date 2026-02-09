use std::borrow::Cow;
use std::error::Error;
use std::io::{Read, Write};
use std::time::Duration;

pub(crate) mod connection;
pub(crate) mod download;
pub mod engine;
pub mod packet_loss;
pub(crate) mod upload;

pub(crate) static BASE_URL: &str = "https://speed.cloudflare.com";

pub trait IoReadAndWrite: Read + Write + Send {}

impl<T: Read + Write + Send> IoReadAndWrite for T {}

pub(crate) trait Test {
    fn endpoint(&'_ self) -> Cow<'_, str>;

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>>;
}

impl<T: Test> Test for &T {
    fn endpoint(&'_ self) -> Cow<'_, str> {
        (**self).endpoint()
    }

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        (**self).run(bytes).await
    }
}

impl<T: Test> Test for &mut T {
    fn endpoint(&'_ self) -> Cow<'_, str> {
        (**self).endpoint()
    }

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        (**self).run(bytes).await
    }
}

/// Complete timing breakdown for a network test.
///
/// This struct captures all timing information needed for accurate
/// bandwidth and latency calculations according to the Cloudflare
/// speed test methodology.
#[derive(Debug, Clone)]
pub(crate) struct TestResults {
    /// Time to establish TCP connection (handshake)
    pub tcp_duration: Duration,
    /// Time to first byte - from request sent to first response byte
    pub ttfb_duration: Duration,
    /// Server processing time extracted from server-timing header
    pub server_time: Duration,
    /// Total time from first response byte to last byte received
    pub end_duration: Duration,
    /// Number of bytes transferred
    pub bytes: u64,
}

impl TestResults {
    pub(crate) const fn new(
        tcp_duration: Duration,
        ttfb_duration: Duration,
        server_time: Duration,
        end_duration: Duration,
        bytes: u64,
    ) -> Self {
        TestResults {
            tcp_duration,
            ttfb_duration,
            server_time,
            end_duration,
            bytes,
        }
    }

    /// Calculate the transfer duration (time to download/upload data).
    ///
    /// This is the time from first byte to last byte, which represents
    /// the actual data transfer time.
    pub fn transfer_duration(&self) -> Duration {
        self.end_duration.saturating_sub(self.ttfb_duration)
    }

    /// Calculate bandwidth in bits per second.
    ///
    /// Uses the transfer duration (end - ttfb) minus server processing time
    /// to calculate the actual data transfer rate.
    ///
    /// # Returns
    /// Bandwidth in bits per second, or 0.0 if the effective transfer time is <= 0
    pub fn bandwidth_bps(&self) -> f64 {
        crate::measurements::calculate_bandwidth_bps(
            self.bytes,
            self.transfer_duration(),
            self.server_time,
        )
    }

    /// Convert the test results to a BandwidthMeasurement for aggregation.
    pub fn to_bandwidth_measurement(
        &self,
    ) -> crate::measurements::BandwidthMeasurement {
        crate::measurements::BandwidthMeasurement {
            bytes: self.bytes,
            bandwidth_bps: self.bandwidth_bps(),
            duration_ms: self.end_duration.as_secs_f64() * 1000.0,
            server_time_ms: self.server_time.as_secs_f64() * 1000.0,
            ttfb_ms: self.ttfb_duration.as_secs_f64() * 1000.0,
        }
    }
}
