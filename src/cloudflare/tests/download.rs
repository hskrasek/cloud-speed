use crate::cloudflare::requests::UA;
use crate::cloudflare::tests::{IoReadAndWrite, Test, TestResults, BASE_URL};
use hickory_resolver::Resolver;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use log::{debug, info};
use rustls_connector::RustlsConnector;
use std::borrow::Cow;
use std::convert::Into;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use tokio::time::Instant;
use url::Url;

pub(crate) struct Download {}

impl Test for Download {
    fn endpoint(&self) -> Cow<str> {
        "__down".into()
    }

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        info!("Beginning Download Test: {}", bytes);
        let mut url =
            Url::parse(format!("{}/{}", BASE_URL, self.endpoint()).as_str())?;
        // Add query param or body based on test method
        url.set_query(Some(format!("bytes={}", bytes).as_str()));

        let (ip_address, dns_duration) = resolve_dns(&url)?;
        let port = url.port_or_known_default().unwrap();
        let (stream, tcp_connect_duration) = tcp_connect(ip_address, port)?;
        let (mut stream, tls_handshake_duration) =
            tls_handshake_duration(stream, &url)?;
        let (connect_duration, ttfb_duration, request_duration, end_duration) =
            execute_http_get(&mut stream, &url)?;

        Ok(TestResults::new(
            dns_duration,
            tcp_connect_duration,
            tls_handshake_duration,
            connect_duration,
            ttfb_duration,
            request_duration.unwrap(),
            end_duration,
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

fn execute_http_get(
    tcp: &mut Box<dyn IoReadAndWrite>,
    url: &Url,
) -> Result<(Duration, Duration, Option<Duration>, Duration), Box<dyn Error>> {
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

    let headers = extract_http_headers(String::from_utf8(headers).unwrap());
    let request_duration = extract_request_duration(
        headers.get(HeaderName::from_static("server-timing")).unwrap(),
    );

    let mut buff = Vec::new();

    tcp.read_to_end(&mut buff)?;

    let end_duration = now.elapsed();

    Ok((connect_duration, ttfb_duration, request_duration, end_duration))
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

fn extract_request_duration(header: &HeaderValue) -> Option<Duration> {
    let dur = header.to_str().unwrap().split(";").collect::<Vec<_>>()[1];
    let ms = dur.split("=").collect::<Vec<_>>()[1].parse::<f64>().unwrap();

    Some(Duration::from_secs_f64(ms / 1000.0))
}
