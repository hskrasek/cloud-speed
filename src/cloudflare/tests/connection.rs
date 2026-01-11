//! Shared connection utilities for speed tests.
//!
//! This module provides common connection establishment functions used by
//! both download and upload tests.

use super::IoReadAndWrite;
use hickory_resolver::TokioResolver;
use rustls_connector::RustlsConnector;
use std::error::Error;
use std::io::Write;
use std::net::{IpAddr, TcpStream};
use std::time::Duration;
use tokio::time::Instant;
use url::Url;

/// Resolve DNS for a URL, preferring IPv4 addresses.
///
/// Returns the resolved IP address and the time taken for DNS resolution.
pub async fn resolve_dns(url: &Url) -> Result<(IpAddr, Duration), Box<dyn Error>> {
    let resolver = TokioResolver::builder_tokio()?.build();

    let begin = Instant::now();

    let response = resolver.lookup_ip(url.host_str().unwrap()).await?;

    let duration = begin.elapsed();

    let ipv4_addresses: Vec<_> =
        response.iter().filter(|addr| addr.is_ipv4()).collect();

    let ipv6_addresses: Vec<_> =
        response.iter().filter(|addr| addr.is_ipv6()).collect();

    if !ipv4_addresses.is_empty() {
        return Ok((ipv4_addresses[0], duration));
    }

    Ok((ipv6_addresses[0], duration))
}

/// Establish a TCP connection to the given address and port.
///
/// Returns the connected stream and the time taken to establish the connection.
pub fn tcp_connect(
    address: IpAddr,
    port: u16,
) -> Result<(TcpStream, Duration), Box<dyn Error>> {
    let now = Instant::now();
    let mut stream = TcpStream::connect((address, port))?;
    stream.flush()?;
    let tcp_connect_duration = now.elapsed();

    Ok((stream, tcp_connect_duration))
}

/// Perform TLS handshake on an established TCP connection.
///
/// Returns a TLS-wrapped stream and the time taken for the handshake.
pub fn tls_handshake_duration(
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

/// Measure TCP latency by performing a TCP handshake.
///
/// This is used for loaded latency measurements during bandwidth tests.
/// Returns the round-trip time in milliseconds.
pub fn measure_tcp_latency(
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
