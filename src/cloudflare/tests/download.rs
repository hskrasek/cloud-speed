use crate::cloudflare::requests::UA;
use crate::cloudflare::tests::connection::{
    measure_tcp_latency, resolve_dns, tcp_connect, tls_handshake_duration,
};
use crate::cloudflare::tests::{IoReadAndWrite, Test, TestResults, BASE_URL};
use crate::measurements::parse_server_timing;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use log::{debug, info};
use std::borrow::Cow;
use std::error::Error;
use std::io::{Read, Write};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use url::Url;

pub(crate) struct Download {}

impl Download {
    /// Run the download test with concurrent loaded latency measurements.
    ///
    /// This method performs a download test while simultaneously measuring
    /// latency at regular intervals. Latency measurements are sent through
    /// the provided channel.
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes to download
    /// * `latency_tx` - Channel sender for latency measurements (in milliseconds)
    /// * `throttle_ms` - Minimum interval between latency measurements (typically 400ms)
    /// * `min_request_duration_ms` - Minimum request duration to include latency (typically 250ms)
    ///
    /// # Returns
    /// The test results including timing breakdown
    pub async fn run_with_loaded_latency(
        &self,
        bytes: u64,
        latency_tx: mpsc::Sender<f64>,
        throttle_ms: u64,
        min_request_duration_ms: u64,
    ) -> Result<TestResults, Box<dyn Error>> {
        info!("Beginning Download Test with loaded latency: {}", bytes);
        let mut url =
            Url::parse(format!("{}/{}", BASE_URL, self.endpoint()).as_str())?;
        url.set_query(Some(format!("bytes={}", bytes).as_str()));

        let (ip_address, _dns_duration) = resolve_dns(&url).await?;
        let port = url.port_or_known_default().unwrap();
        let (stream, tcp_connect_duration) = tcp_connect(ip_address, port)?;
        let (mut stream, _tls_handshake_duration) =
            tls_handshake_duration(stream, &url)?;

        // Execute HTTP GET with concurrent latency measurements
        let (_connect_duration, ttfb_duration, server_time, end_duration) =
            execute_http_get_with_latency(
                &mut stream,
                &url,
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

impl Test for Download {
    fn endpoint(&'_ self) -> Cow<'_, str> {
        "__down".into()
    }

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        info!("Beginning Download Test: {}", bytes);
        let mut url =
            Url::parse(format!("{}/{}", BASE_URL, self.endpoint()).as_str())?;
        // Add query param or body based on test method
        url.set_query(Some(format!("bytes={}", bytes).as_str()));

        let (_ip_address, _dns_duration) = resolve_dns(&url).await?;
        let port = url.port_or_known_default().unwrap();
        let (stream, tcp_connect_duration) = tcp_connect(_ip_address, port)?;
        let (mut stream, _tls_handshake_duration) =
            tls_handshake_duration(stream, &url)?;
        let (_connect_duration, ttfb_duration, server_time, end_duration) =
            execute_http_get(&mut stream, &url)?;

        Ok(TestResults::new(
            tcp_connect_duration,
            ttfb_duration,
            server_time,
            end_duration,
            bytes,
        ))
    }
}

fn execute_http_get(
    tcp: &mut Box<dyn IoReadAndWrite>,
    url: &Url,
) -> Result<(Duration, Duration, Duration, Duration), Box<dyn Error>> {
    let header = build_http_header(url);
    debug!("\r\n{}", header);
    let now = Instant::now();

    tcp.write_all(header.as_bytes())?;
    tcp.flush()?;

    let connect_duration = now.elapsed();

    let mut one_byte_buffer = [0_u8];
    let now = Instant::now();
    tcp.read_exact(&mut one_byte_buffer)?;
    let ttfb_duration = now.elapsed();

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

    let headers_str = String::from_utf8(headers)
        .map_err(|e| format!("Invalid UTF-8 in HTTP headers: {}", e))?;
    let headers = extract_http_headers(&headers_str);

    // Extract server processing time from server-timing header
    let server_time = headers
        .get(HeaderName::from_static("server-timing"))
        .and_then(|h| h.to_str().ok())
        .and_then(parse_server_timing)
        .unwrap_or(Duration::ZERO);

    let mut buff = Vec::new();

    tcp.read_to_end(&mut buff)?;

    let end_duration = now.elapsed();

    Ok((connect_duration, ttfb_duration, server_time, end_duration))
}

fn build_http_header(url: &Url) -> String {
    format!(
        "GET {}?{} HTTP/1.1\r\n\
        Host: {}\r\n\
        User-Agent: {}\r\n\
        Accept: */*\r\n\
        Accept-Encoding: gzip, deflate, br, zstd\r\n\
        Connection: close\r\n\
        \r\n",
        url.path(),
        url.query().unwrap(),
        url.host_str().unwrap(),
        UA
    )
}

fn extract_http_headers(raw_headers: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();

    for line in raw_headers.lines() {
        let line = line.trim();

        if line.is_empty() || !line.contains(':') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }

        // Skip malformed header names/values instead of panicking
        let name = match HeaderName::from_str(parts[0].trim()) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let value = match HeaderValue::from_str(parts[1].trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        headers.append(name, value);
    }

    headers
}

/// Execute HTTP GET with concurrent latency measurements.
///
/// This function performs the HTTP GET request while spawning a background
/// task that measures latency at regular intervals. Latency measurements
/// are only included if the request duration exceeds the minimum threshold.
async fn execute_http_get_with_latency(
    tcp: &mut Box<dyn IoReadAndWrite>,
    url: &Url,
    ip_address: IpAddr,
    port: u16,
    latency_tx: mpsc::Sender<f64>,
    throttle_ms: u64,
    min_request_duration_ms: u64,
) -> Result<(Duration, Duration, Duration, Duration), Box<dyn Error>> {
    let header = build_http_header(url);
    debug!("\r\n{}", header);
    let request_start = Instant::now();

    tcp.write_all(header.as_bytes())?;
    tcp.flush()?;

    let connect_duration = request_start.elapsed();

    // Start latency measurement task
    let latency_tx_clone = latency_tx.clone();
    let throttle_duration = Duration::from_millis(throttle_ms);
    let min_duration = Duration::from_millis(min_request_duration_ms);
    let request_start_clone = request_start;

    // Use Arc to share the stop flag between tasks
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Spawn latency measurement task
    let latency_handle = tokio::spawn(async move {
        let mut last_measurement = Instant::now();

        loop {
            // Check if we should stop (Acquire pairs with Release in main thread)
            if stop_flag_clone.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }

            // Wait for throttle interval
            let elapsed_since_last = last_measurement.elapsed();
            if elapsed_since_last < throttle_duration {
                tokio::time::sleep(throttle_duration - elapsed_since_last)
                    .await;
            }

            // Check again after sleep (Acquire pairs with Release in main thread)
            if stop_flag_clone.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }

            // Only measure if request has been running long enough
            let request_duration = request_start_clone.elapsed();
            if request_duration >= min_duration {
                // Measure latency using TCP handshake time
                if let Ok(latency_ms) = measure_tcp_latency(ip_address, port) {
                    let _ = latency_tx_clone.send(latency_ms).await;
                }
            }

            last_measurement = Instant::now();
        }
    });

    // Read first byte (TTFB)
    let mut one_byte_buffer = [0_u8];
    let ttfb_start = Instant::now();
    tcp.read_exact(&mut one_byte_buffer)?;
    let ttfb_duration = ttfb_start.elapsed();

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

    let headers_str = String::from_utf8(headers)
        .map_err(|e| format!("Invalid UTF-8 in HTTP headers: {}", e))?;
    let headers = extract_http_headers(&headers_str);

    // Extract server processing time from server-timing header
    let server_time = headers
        .get(HeaderName::from_static("server-timing"))
        .and_then(|h| h.to_str().ok())
        .and_then(parse_server_timing)
        .unwrap_or(Duration::ZERO);

    // Read body
    let mut buff = Vec::new();
    tcp.read_to_end(&mut buff)?;

    let end_duration = ttfb_start.elapsed();

    // Signal latency task to stop (Release ensures visibility to other thread)
    stop_flag.store(true, std::sync::atomic::Ordering::Release);

    // Wait for latency task to finish (with timeout)
    let _ =
        tokio::time::timeout(Duration::from_millis(100), latency_handle).await;

    Ok((connect_duration, ttfb_duration, server_time, end_duration))
}
