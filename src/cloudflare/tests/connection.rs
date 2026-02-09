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
/// Runs on a blocking thread pool via `spawn_blocking` to avoid
/// starving the tokio async runtime.
///
/// Returns the connected stream and the time taken to establish the connection.
pub async fn tcp_connect(
    address: IpAddr,
    port: u16,
) -> Result<(TcpStream, Duration), Box<dyn Error>> {
    tokio::task::spawn_blocking(move || {
        let now = Instant::now();
        let mut stream = TcpStream::connect((address, port))?;
        stream.flush()?;
        let tcp_connect_duration = now.elapsed();
        Ok::<_, std::io::Error>((stream, tcp_connect_duration))
    })
    .await?
    .map_err(|e| e.into())
}

/// Perform TLS handshake on an established TCP connection.
///
/// Runs on a blocking thread pool via `spawn_blocking` to avoid
/// starving the tokio async runtime.
///
/// Returns a TLS-wrapped stream and the time taken for the handshake.
pub async fn tls_handshake_duration(
    tcp: TcpStream,
    host: String,
) -> Result<(Box<dyn IoReadAndWrite>, Duration), Box<dyn Error>> {
    let result: Result<_, Box<dyn Error + Send + Sync>> =
        tokio::task::spawn_blocking(move || {
            let connector: RustlsConnector =
                RustlsConnector::new_with_native_certs()
                    .unwrap_or_else(|_| {
                        RustlsConnector::new_with_webpki_roots_certs()
                    });
            let now = Instant::now();

            let mut stream = connector.connect(&host, tcp)?;
            stream.flush()?;
            let tls_handshake_duration = now.elapsed();
            Ok((
                Box::new(stream) as Box<dyn IoReadAndWrite>,
                tls_handshake_duration,
            ))
        })
        .await?;

    result.map_err(|e| e as Box<dyn Error>)
}

/// Measure TCP latency by performing a TCP handshake.
///
/// Runs on a blocking thread pool via `spawn_blocking` to avoid
/// starving the tokio async runtime.
///
/// This is used for loaded latency measurements during bandwidth tests.
/// Returns the round-trip time in milliseconds.
pub async fn measure_tcp_latency(
    ip_address: IpAddr,
    port: u16,
) -> Result<f64, Box<dyn Error + Send + Sync>> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let stream = TcpStream::connect_timeout(
            &std::net::SocketAddr::new(ip_address, port),
            Duration::from_secs(5),
        )?;
        let latency = start.elapsed();

        // Close the connection
        drop(stream);

        Ok(latency.as_secs_f64() * 1000.0)
    })
    .await?
}
