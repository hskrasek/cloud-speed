use crate::cloudflare::requests::UA;
use crate::cloudflare::tests::{IoReadAndWrite, Test, TestResults, BASE_URL};
use crate::measurements::parse_server_timing;
use hickory_resolver::Resolver;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use log::{debug, info};
use reqwest::Method;
use rustls_connector::RustlsConnector;
use std::borrow::Cow;
use std::convert::Into;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
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

        let (ip_address, _dns_duration) = resolve_dns(&url)?;
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
    const METHOD: Method = Method::POST;

    fn endpoint(&'_ self) -> Cow<'_, str> {
        "__up".into()
    }

    async fn run(&self, _bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        // Note: bytes parameter is ignored; we use self.data.len() instead
        let bytes = self.bytes();
        info!("Beginning Upload Test: {}", bytes);

        let url =
            Url::parse(format!("{}/{}", BASE_URL, self.endpoint()).as_str())?;

        let (_ip_address, _dns_duration) = resolve_dns(&url)?;
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

fn resolve_dns(url: &Url) -> Result<(IpAddr, Duration), Box<dyn Error>> {
    let resolver =
        Resolver::from_system_conf().or_else(|_| Resolver::default())?;

    let begin = Instant::now();

    let response = {
        let url = url.clone();
        thread::spawn(move || resolver.lookup_ip(url.host_str().unwrap()))
            .join()
            .unwrap()?
    };

    let duration = begin.elapsed();

    let ipv4_addresses =
        response.iter().filter(|addr| addr.is_ipv4()).collect::<Vec<_>>();

    let ipv6_addresses =
        response.iter().filter(|addr| addr.is_ipv6()).collect::<Vec<_>>();

    if !ipv4_addresses.is_empty() {
        return Ok((ipv4_addresses[0], duration));
    }

    Ok((ipv6_addresses[0], duration))
}

fn tcp_connect(
    address: IpAddr,
    port: u16,
) -> Result<(TcpStream, Duration), Box<dyn Error>> {
    let now = Instant::now();
    let mut stream = TcpStream::connect((address, port))?;
    stream.flush()?;
    let tcp_connect_duration = now.elapsed();

    Ok((stream, tcp_connect_duration))
}

fn tls_handshake_duration(
    tcp: TcpStream,
    url: &Url,
) -> Result<(Box<dyn IoReadAndWrite>, Duration), Box<dyn Error>> {
    let connector: RustlsConnector = RustlsConnector::new_with_native_certs()
        .unwrap_or_else(|_| RustlsConnector::new_with_webpki_roots_certs());
    let now = Instant::now();

    let certificate_host = url.host_str().unwrap_or("");
    let mut stream = connector.connect(certificate_host, tcp)?;
    stream.flush().expect("Stream error");
    let tls_handshake_duration = now.elapsed();
    Ok((Box::new(stream), tls_handshake_duration))
}

fn execute_http_post(
    tcp: &mut Box<dyn IoReadAndWrite>,
    url: &Url,
    data: &[u8],
) -> Result<(Duration, Duration, Duration, Duration), Box<dyn Error>> {
    let header = build_http_post_header(url, data.len());
    debug!("\r\n{}", header);
    let now = Instant::now();

    // Write headers
    tcp.write_all(header.as_bytes())?;
    // Write body
    tcp.write_all(data)?;
    tcp.flush()?;

    let connect_duration = now.elapsed();

    // Read first byte (TTFB)
    let mut one_byte_buffer = [0_u8];
    let now = Instant::now();
    tcp.read_exact(&mut one_byte_buffer)?;
    let ttfb_duration = now.elapsed();

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

    let headers = extract_http_headers(String::from_utf8(headers).unwrap());

    // Extract server processing time from server-timing header
    let server_time = headers
        .get(HeaderName::from_static("server-timing"))
        .and_then(|h| h.to_str().ok())
        .and_then(parse_server_timing)
        .unwrap_or(Duration::ZERO);

    // Read any remaining response body
    let mut buff = Vec::new();
    tcp.read_to_end(&mut buff)?;

    let end_duration = now.elapsed();

    Ok((connect_duration, ttfb_duration, server_time, end_duration))
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

fn extract_http_headers(raw_headers: String) -> HeaderMap {
    let mut headers = HeaderMap::new();

    for line in raw_headers.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if !line.contains(":") {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ':').collect();
        let name = HeaderName::from_str(parts[0].trim()).unwrap();
        let value = HeaderValue::from_str(parts[1].trim()).unwrap();

        headers.append(name, value);
    }

    headers
}

/// Execute HTTP POST with concurrent latency measurements.
///
/// This function performs the HTTP POST request while spawning a background
/// task that measures latency at regular intervals. Latency measurements
/// are only included if the request duration exceeds the minimum threshold.
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
    let request_start = Instant::now();

    // Write headers
    tcp.write_all(header.as_bytes())?;
    // Write body
    tcp.write_all(data)?;
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

    let headers = extract_http_headers(String::from_utf8(headers).unwrap());

    // Extract server processing time from server-timing header
    let server_time = headers
        .get(HeaderName::from_static("server-timing"))
        .and_then(|h| h.to_str().ok())
        .and_then(parse_server_timing)
        .unwrap_or(Duration::ZERO);

    // Read any remaining response body
    let mut buff = Vec::new();
    tcp.read_to_end(&mut buff)?;

    let end_duration = ttfb_start.elapsed();

    // Signal latency task to stop
    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);

    // Wait for latency task to finish (with timeout)
    let _ =
        tokio::time::timeout(Duration::from_millis(100), latency_handle).await;

    Ok((connect_duration, ttfb_duration, server_time, end_duration))
}

/// Measure TCP latency by performing a TCP handshake.
///
/// This is used for loaded latency measurements during uploads.
/// Returns the round-trip time in milliseconds.
fn measure_tcp_latency(
    ip_address: IpAddr,
    port: u16,
) -> Result<f64, Box<dyn Error + Send + Sync>> {
    let start = Instant::now();
    let stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::new(ip_address, port),
        Duration::from_secs(5),
    )?;
    let latency = start.elapsed();

    // Close the connection
    drop(stream);

    Ok(latency.as_secs_f64() * 1000.0)
}
