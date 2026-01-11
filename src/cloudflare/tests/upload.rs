use crate::cloudflare::requests::UA;
use crate::cloudflare::tests::connection::{
    measure_tcp_latency, resolve_dns, tcp_connect, tls_handshake_duration,
};
use crate::cloudflare::tests::{IoReadAndWrite, Test, TestResults, BASE_URL};
use log::{debug, info};
use std::borrow::Cow;
use std::error::Error;
use std::io::{Read, Write};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use url::Url;

/// Upload test implementation for measuring upload bandwidth.
///
/// This struct performs upload tests by POSTing data to Cloudflare's
/// `/__up` endpoint and measuring the timing breakdown.
pub(crate) struct Upload {
    /// Pre-generated payload data to upload
    data: Vec<u8>,
}

impl Upload {
    /// Create a new upload test with the specified payload size.
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes to upload
    ///
    /// # Returns
    /// A new Upload instance with pre-generated payload data
    pub fn new(bytes: u64) -> Self {
        // Generate payload data (zeros are efficient and compress well)
        let data = vec![b'0'; bytes as usize];
        Self { data }
    }

    /// Get the size of the upload payload in bytes.
    pub fn bytes(&self) -> u64 {
        self.data.len() as u64
    }

    /// Run the upload test with concurrent loaded latency measurements.
    ///
    /// This method performs an upload test while simultaneously measuring
    /// latency at regular intervals. Latency measurements are sent through
    /// the provided channel.
    ///
    /// # Arguments
    /// * `latency_tx` - Channel sender for latency measurements (in ms)
    /// * `throttle_ms` - Minimum interval between latency measurements
    /// * `min_request_duration_ms` - Minimum request duration to include
    ///   latency (typically 250ms)
    ///
    /// # Returns
    /// The test results including timing breakdown
    pub async fn run_with_loaded_latency(
        &self,
        latency_tx: mpsc::Sender<f64>,
        throttle_ms: u64,
        min_request_duration_ms: u64,
    ) -> Result<TestResults, Box<dyn Error>> {
        let bytes = self.bytes();
        info!("Beginning Upload Test with loaded latency: {}", bytes);

        let url =
            Url::parse(format!("{}/{}", BASE_URL, self.endpoint()).as_str())?;

        let (ip_address, _dns_duration) = resolve_dns(&url).await?;
        let port = url.port_or_known_default().unwrap();
        let (stream, tcp_connect_duration) = tcp_connect(ip_address, port)?;
        let (mut stream, _tls_handshake_duration) =
            tls_handshake_duration(stream, &url)?;

        // Execute HTTP POST with concurrent latency measurements
        let (_connect_duration, ttfb_duration, server_time, end_duration) =
            execute_http_post_with_latency(
                &mut stream,
                &url,
                &self.data,
                ip_address,
                port,
                latency_tx,
                throttle_ms,
                min_request_duration_ms,
            )
            .await?;

        Ok(TestResults::new(
            tcp_connect_duration,
            ttfb_duration,
            server_time,
            end_duration,
            bytes,
        ))
    }
}

impl Test for Upload {
    fn endpoint(&'_ self) -> Cow<'_, str> {
        "__up".into()
    }

    async fn run(&self, _bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        // Note: bytes parameter is ignored; we use self.data.len() instead
        let bytes = self.bytes();
        info!("Beginning Upload Test: {}", bytes);

        let url =
            Url::parse(format!("{}/{}", BASE_URL, self.endpoint()).as_str())?;

        let (_ip_address, _dns_duration) = resolve_dns(&url).await?;
        let port = url.port_or_known_default().unwrap();
        let (stream, tcp_connect_duration) = tcp_connect(_ip_address, port)?;
        let (mut stream, _tls_handshake_duration) =
            tls_handshake_duration(stream, &url)?;
        let (_connect_duration, ttfb_duration, server_time, end_duration) =
            execute_http_post(&mut stream, &url, &self.data)?;

        Ok(TestResults::new(
            tcp_connect_duration,
            ttfb_duration,
            server_time,
            end_duration,
            bytes,
        ))
    }
}

fn execute_http_post(
    tcp: &mut Box<dyn IoReadAndWrite>,
    url: &Url,
    data: &[u8],
) -> Result<(Duration, Duration, Duration, Duration), Box<dyn Error>> {
    let header = build_http_post_header(url, data.len());
    debug!("\r\n{}", header);
    let upload_start = Instant::now();

    // Write headers
    tcp.write_all(header.as_bytes())?;
    // Write body - this is the actual upload
    tcp.write_all(data)?;
    tcp.flush()?;

    // Read first byte (TTFB) - this marks when server received all data
    // and started responding
    let mut one_byte_buffer = [0_u8];
    tcp.read_exact(&mut one_byte_buffer)?;

    // For uploads, the transfer time is from start of write to TTFB
    // This captures the actual network transfer time
    let upload_duration = upload_start.elapsed();

    // Read headers
    let mut headers: Vec<u8> = Vec::new();
    headers.push(one_byte_buffer[0]);

    while tcp.read(&mut one_byte_buffer)? > 0 {
        headers.push(one_byte_buffer[0]);
        if headers.len() >= 4
            && headers[headers.len() - 4..] == [b'\r', b'\n', b'\r', b'\n']
        {
            break;
        }
    }

    // Read any remaining response body (we don't need server-timing for uploads)
    let mut buff = Vec::new();
    tcp.read_to_end(&mut buff)?;

    // For uploads: return upload_duration as end_duration and Duration::ZERO
    // for both ttfb and server_time. This way:
    // - transfer_duration() = end_duration - ttfb = upload_duration
    // - bandwidth calculation uses upload_duration directly without subtracting
    //   server_time (which for uploads includes the receive time)
    Ok((upload_duration, Duration::ZERO, Duration::ZERO, upload_duration))
}

fn build_http_post_header(url: &Url, content_length: usize) -> String {
    format!(
        "POST {} HTTP/1.1\r\n\
        Host: {}\r\n\
        User-Agent: {}\r\n\
        Accept: */*\r\n\
        Content-Type: text/plain;charset=UTF-8\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\
        \r\n",
        url.path(),
        url.host_str().unwrap(),
        UA,
        content_length
    )
}

/// Execute HTTP POST with concurrent latency measurements.
///
/// This function performs the HTTP POST request while spawning a background
/// task that measures latency at regular intervals. Latency measurements
/// are only included if the request duration exceeds the minimum threshold.
#[allow(clippy::too_many_arguments)]
async fn execute_http_post_with_latency(
    tcp: &mut Box<dyn IoReadAndWrite>,
    url: &Url,
    data: &[u8],
    ip_address: IpAddr,
    port: u16,
    latency_tx: mpsc::Sender<f64>,
    throttle_ms: u64,
    min_request_duration_ms: u64,
) -> Result<(Duration, Duration, Duration, Duration), Box<dyn Error>> {
    let header = build_http_post_header(url, data.len());
    debug!("\r\n{}", header);
    let upload_start = Instant::now();

    // Start latency measurement task before upload begins
    let latency_tx_clone = latency_tx.clone();
    let throttle_duration = Duration::from_millis(throttle_ms);
    let min_duration = Duration::from_millis(min_request_duration_ms);
    let upload_start_clone = upload_start;

    // Use Arc to share the stop flag between tasks
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Spawn latency measurement task
    let latency_handle = tokio::spawn(async move {
        let mut last_measurement = Instant::now();

        loop {
            // Check if we should stop
            if stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            // Wait for throttle interval
            let elapsed_since_last = last_measurement.elapsed();
            if elapsed_since_last < throttle_duration {
                tokio::time::sleep(throttle_duration - elapsed_since_last)
                    .await;
            }

            // Check again after sleep
            if stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            // Only measure if request has been running long enough
            let request_duration = upload_start_clone.elapsed();
            if request_duration >= min_duration {
                // Measure latency using TCP handshake time
                if let Ok(latency_ms) = measure_tcp_latency(ip_address, port) {
                    let _ = latency_tx_clone.send(latency_ms).await;
                }
            }

            last_measurement = Instant::now();
        }
    });

    // Write headers
    tcp.write_all(header.as_bytes())?;
    // Write body - this is the actual upload
    tcp.write_all(data)?;
    tcp.flush()?;

    // Read first byte (TTFB) - this marks when server received all data
    // and started responding
    let mut one_byte_buffer = [0_u8];
    tcp.read_exact(&mut one_byte_buffer)?;

    // For uploads, the transfer time is from start of write to TTFB
    // This captures the actual network transfer time
    let upload_duration = upload_start.elapsed();

    // Read headers
    let mut headers: Vec<u8> = Vec::new();
    headers.push(one_byte_buffer[0]);

    while tcp.read(&mut one_byte_buffer)? > 0 {
        headers.push(one_byte_buffer[0]);
        if headers.len() >= 4
            && headers[headers.len() - 4..] == [b'\r', b'\n', b'\r', b'\n']
        {
            break;
        }
    }

    // Read any remaining response body (we don't need server-timing for uploads)
    let mut buff = Vec::new();
    tcp.read_to_end(&mut buff)?;

    // Signal latency task to stop
    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);

    // Wait for latency task to finish (with timeout)
    let _ =
        tokio::time::timeout(Duration::from_millis(100), latency_handle).await;

    // For uploads: return upload_duration as end_duration and Duration::ZERO
    // for both ttfb and server_time. This way:
    // - transfer_duration() = end_duration - ttfb = upload_duration
    // - bandwidth calculation uses upload_duration directly without subtracting
    //   server_time (which for uploads includes the receive time)
    Ok((upload_duration, Duration::ZERO, Duration::ZERO, upload_duration))
}
