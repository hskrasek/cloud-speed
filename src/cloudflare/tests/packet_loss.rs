//! Packet loss measurement using UDP via TURN server.
//!
//! This module implements packet loss measurement by sending UDP packets
//! through a TURN (Traversal Using Relays around NAT) server and measuring
//! how many packets are lost in transit.
//!
//! # Requirements
//! - Requires TURN server credentials to be configured
//! - Sends UDP packets and waits for responses
//! - Calculates packet loss ratio as lost/sent

use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

/// Configuration for packet loss measurement via TURN server.
///
/// This struct contains all the parameters needed to connect to a TURN
/// server and perform packet loss measurements.
///
/// # Example
/// ```
/// use cloud_speed::cloudflare::tests::packet_loss::PacketLossConfig;
///
/// let config = PacketLossConfig::new(
///     "turn:turn.example.com:3478".to_string(),
///     "username".to_string(),
///     "password".to_string(),
/// );
/// ```
#[derive(Debug, Clone)]
pub struct PacketLossConfig {
    /// TURN server URI (e.g., "turn:turn.example.com:3478")
    pub turn_server_uri: String,
    /// Number of UDP packets to send for measurement
    /// Default: 1000
    pub num_packets: usize,
    /// Number of packets to send in each batch
    /// Default: 100
    pub batch_size: usize,
    /// Wait time between batches (in ms)
    /// Default: 10ms
    pub batch_wait_time_ms: u64,
    /// Timeout for individual packet responses (in ms)
    /// Default: 1000ms
    pub packet_timeout_ms: u64,
}

impl PacketLossConfig {
    /// Default number of packets to send.
    pub const DEFAULT_NUM_PACKETS: usize = 1000;

    /// Default batch size.
    pub const DEFAULT_BATCH_SIZE: usize = 100;

    /// Default batch wait time in milliseconds.
    pub const DEFAULT_BATCH_WAIT_TIME_MS: u64 = 10;

    /// Default packet timeout in milliseconds.
    pub const DEFAULT_PACKET_TIMEOUT_MS: u64 = 1000;

    /// Create a new PacketLossConfig with required parameters and defaults.
    ///
    /// # Arguments
    /// * `turn_server_uri` - TURN server URI
    pub fn new(turn_server_uri: String) -> Self {
        Self {
            turn_server_uri,
            num_packets: Self::DEFAULT_NUM_PACKETS,
            batch_size: Self::DEFAULT_BATCH_SIZE,
            batch_wait_time_ms: Self::DEFAULT_BATCH_WAIT_TIME_MS,
            packet_timeout_ms: Self::DEFAULT_PACKET_TIMEOUT_MS,
        }
    }
}

/// Result of a packet loss measurement.
///
/// Contains the calculated packet loss ratio and detailed statistics
/// about the measurement.
#[derive(Debug, Clone)]
pub struct PacketLossResult {
    /// Packet loss ratio (0.0 to 1.0)
    /// 0.0 = no packets lost, 1.0 = all packets lost
    pub packet_loss_ratio: f64,
    /// Total number of packets that were supposed to be sent
    pub total_packets: usize,
    /// Number of packets actually sent
    pub packets_sent: usize,
    /// Number of packets that were lost (sent but no response received)
    pub packets_lost: usize,
    /// Number of packets that received responses
    pub packets_received: usize,
    /// Average round-trip time for received packets (in ms)
    pub avg_rtt_ms: Option<f64>,
}

impl PacketLossResult {
    /// Create a new PacketLossResult from measurement data.
    ///
    /// # Arguments
    /// * `packets_sent` - Number of packets sent
    /// * `packets_received` - Number of packets that received responses
    /// * `avg_rtt_ms` - Optional average round-trip time
    ///
    /// # Panics
    /// Panics if packets_received > packets_sent
    pub fn new(
        packets_sent: usize,
        packets_received: usize,
        avg_rtt_ms: Option<f64>,
    ) -> Self {
        assert!(
            packets_received <= packets_sent,
            "packets_received ({}) cannot exceed packets_sent ({})",
            packets_received,
            packets_sent
        );

        let packets_lost = packets_sent.saturating_sub(packets_received);
        let packet_loss_ratio = if packets_sent > 0 {
            packets_lost as f64 / packets_sent as f64
        } else {
            0.0
        };

        Self {
            packet_loss_ratio,
            total_packets: packets_sent,
            packets_sent,
            packets_lost,
            packets_received,
            avg_rtt_ms,
        }
    }

    /// Create a result indicating packet loss measurement is unavailable.
    ///
    /// Used when TURN server is not configured or connection fails.
    pub fn unavailable() -> Self {
        Self {
            packet_loss_ratio: 0.0,
            total_packets: 0,
            packets_sent: 0,
            packets_lost: 0,
            packets_received: 0,
            avg_rtt_ms: None,
        }
    }

    /// Check if the measurement was successful (packets were sent).
    pub fn is_available(&self) -> bool {
        self.packets_sent > 0
    }

    /// Get the packet loss as a percentage (0-100).
    pub fn packet_loss_percent(&self) -> f64 {
        self.packet_loss_ratio * 100.0
    }
}

/// Error type for packet loss measurement failures.
#[derive(Debug)]
pub enum PacketLossError {
    /// Failed to connect to TURN server
    ConnectionFailed(String),
    /// Invalid TURN server URI
    InvalidUri(String),
}

impl fmt::Display for PacketLossError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketLossError::ConnectionFailed(msg) => {
                write!(f, "Failed to connect to TURN server: {}", msg)
            }
            PacketLossError::InvalidUri(uri) => {
                write!(f, "Invalid TURN server URI: {}", uri)
            }
        }
    }
}

impl Error for PacketLossError {}

/// Packet loss test implementation.
///
/// This struct handles the execution of packet loss measurements
/// using UDP packets via a TURN server.
///
/// # Note
/// This is a simplified implementation. A full TURN client implementation
/// would require the STUN/TURN protocol stack. For now, this provides
/// the interface and basic structure, with the actual UDP measurement
/// being a placeholder that can be extended.
pub struct PacketLossTest {
    config: PacketLossConfig,
}

impl PacketLossTest {
    /// Create a new packet loss test with the given configuration.
    pub fn new(config: PacketLossConfig) -> Self {
        Self { config }
    }

    /// Run the packet loss measurement.
    ///
    /// This method sends UDP packets through the configured TURN server
    /// and measures how many packets are lost.
    ///
    /// # Returns
    /// * `Ok(PacketLossResult)` - Measurement results
    /// * `Err(PacketLossError)` - If measurement fails
    ///
    /// # Note
    /// This is a simplified implementation. A production implementation
    /// would need a full STUN/TURN client library.
    pub async fn run(&self) -> Result<PacketLossResult, PacketLossError> {
        use log::{debug, info, warn};
        use std::time::Instant;

        info!(
            "Starting packet loss measurement: {} packets to {}",
            self.config.num_packets, self.config.turn_server_uri
        );

        // Parse the TURN URI to extract host and port
        let (host, port) = self.parse_turn_uri()?;
        debug!("Parsed TURN server: {}:{}", host, port);

        // Resolve the TURN server address
        let addr = self.resolve_address(&host, port).await?;
        debug!("Resolved TURN server address: {}", addr);

        // Create UDP socket
        let socket = self.create_socket().await?;
        debug!("Created UDP socket");

        // Send packets and track responses
        let start_time = Instant::now();
        let mut packets_sent = 0usize;
        let mut packets_received = 0usize;
        let mut total_rtt_ms = 0.0f64;

        // Send packets in batches
        let num_batches =
            self.config.num_packets.div_ceil(self.config.batch_size);

        for batch in 0..num_batches {
            let batch_start = batch * self.config.batch_size;
            let batch_end = (batch_start + self.config.batch_size)
                .min(self.config.num_packets);

            debug!(
                "Sending batch {}/{}: packets {}-{}",
                batch + 1,
                num_batches,
                batch_start,
                batch_end - 1
            );

            for seq in batch_start..batch_end {
                // Create a simple packet with sequence number
                let packet = self.create_packet(seq as u32);

                // Send the packet
                let send_time = Instant::now();
                match socket.send_to(&packet, addr).await {
                    Ok(_) => {
                        packets_sent += 1;

                        // Try to receive response with timeout
                        let timeout = Duration::from_millis(
                            self.config.packet_timeout_ms,
                        );
                        let mut buf = [0u8; 1024];

                        match tokio::time::timeout(
                            timeout,
                            socket.recv_from(&mut buf),
                        )
                        .await
                        {
                            Ok(Ok((len, _from))) => {
                                if self
                                    .validate_response(&buf[..len], seq as u32)
                                {
                                    packets_received += 1;
                                    let rtt = send_time.elapsed();
                                    total_rtt_ms += rtt.as_secs_f64() * 1000.0;
                                }
                            }
                            Ok(Err(e)) => {
                                debug!(
                                    "Receive error for packet {}: {}",
                                    seq, e
                                );
                            }
                            Err(_) => {
                                // Timeout - packet lost
                                debug!("Timeout for packet {}", seq);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to send packet {}: {}", seq, e);
                    }
                }
            }

            // Wait between batches (except for the last batch)
            if batch < num_batches - 1 && self.config.batch_wait_time_ms > 0 {
                tokio::time::sleep(Duration::from_millis(
                    self.config.batch_wait_time_ms,
                ))
                .await;
            }
        }

        let elapsed = start_time.elapsed();
        info!(
            "Packet loss measurement complete in {:.2}s: sent={}, received={}, lost={}",
            elapsed.as_secs_f64(),
            packets_sent,
            packets_received,
            packets_sent.saturating_sub(packets_received)
        );

        let avg_rtt_ms = if packets_received > 0 {
            Some(total_rtt_ms / packets_received as f64)
        } else {
            None
        };

        Ok(PacketLossResult::new(packets_sent, packets_received, avg_rtt_ms))
    }

    /// Parse the TURN URI to extract host and port.
    fn parse_turn_uri(&self) -> Result<(String, u16), PacketLossError> {
        let uri = &self.config.turn_server_uri;

        // Remove the "turn:" or "turns:" prefix
        let without_scheme = uri
            .strip_prefix("turn:")
            .or_else(|| uri.strip_prefix("turns:"))
            .or_else(|| uri.strip_prefix("//"))
            .unwrap_or(uri);

        // Split host and port
        let parts: Vec<&str> = without_scheme.split(':').collect();

        match parts.len() {
            1 => {
                // No port specified, use default TURN port
                Ok((parts[0].to_string(), 3478))
            }
            2 => {
                let host = parts[0].to_string();
                let port = parts[1].parse::<u16>().map_err(|_| {
                    PacketLossError::InvalidUri(format!(
                        "Invalid port in URI: {}",
                        uri
                    ))
                })?;
                Ok((host, port))
            }
            _ => Err(PacketLossError::InvalidUri(format!(
                "Cannot parse TURN URI: {}",
                uri
            ))),
        }
    }

    /// Resolve the TURN server hostname to a socket address.
    async fn resolve_address(
        &self,
        host: &str,
        port: u16,
    ) -> Result<SocketAddr, PacketLossError> {
        use tokio::net::lookup_host;

        let addr_str = format!("{}:{}", host, port);

        let mut addrs = lookup_host(&addr_str).await.map_err(|e| {
            PacketLossError::ConnectionFailed(format!(
                "Failed to resolve {}: {}",
                addr_str, e
            ))
        })?;

        addrs.next().ok_or_else(|| {
            PacketLossError::ConnectionFailed(format!(
                "No addresses found for {}",
                addr_str
            ))
        })
    }

    /// Create a UDP socket for packet loss measurement.
    async fn create_socket(
        &self,
    ) -> Result<tokio::net::UdpSocket, PacketLossError> {
        // Bind to any available port
        tokio::net::UdpSocket::bind("0.0.0.0:0").await.map_err(|e| {
            PacketLossError::ConnectionFailed(format!(
                "Failed to create UDP socket: {}",
                e
            ))
        })
    }

    /// Create a packet with the given sequence number.
    ///
    /// The packet format is simple:
    /// - 4 bytes: sequence number (big-endian)
    /// - 8 bytes: timestamp (big-endian, microseconds since epoch)
    /// - 4 bytes: padding
    fn create_packet(&self, seq: u32) -> Vec<u8> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut packet = Vec::with_capacity(16);

        // Sequence number (4 bytes, big-endian)
        packet.extend_from_slice(&seq.to_be_bytes());

        // Timestamp (8 bytes, big-endian)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        packet.extend_from_slice(&timestamp.to_be_bytes());

        // Padding (4 bytes)
        packet.extend_from_slice(&[0u8; 4]);

        packet
    }

    /// Validate a response packet.
    ///
    /// For a simple echo server, we just check if the sequence number matches.
    fn validate_response(&self, data: &[u8], expected_seq: u32) -> bool {
        if data.len() < 4 {
            return false;
        }

        let seq_bytes: [u8; 4] = data[0..4].try_into().unwrap_or([0; 4]);
        let seq = u32::from_be_bytes(seq_bytes);

        seq == expected_seq
    }
}

/// Run packet loss measurement with optional configuration.
///
/// This function handles the case where TURN server configuration may not
/// be available. If no configuration is provided, it returns an unavailable
/// result instead of failing.
///
/// # Arguments
/// * `config` - Optional TURN server configuration
///
/// # Returns
/// * `Ok(PacketLossResult)` - Measurement results (may be unavailable)
/// * `Err(PacketLossError)` - If measurement fails for reasons other than
///   missing configuration
///
/// # Example
/// ```
/// // With configuration
/// let config = Some(PacketLossConfig::new(...));
/// let result = run_packet_loss_test(config).await?;
///
/// // Without configuration - returns unavailable result
/// let result = run_packet_loss_test(None).await?;
/// assert!(!result.is_available());
/// ```
pub async fn run_packet_loss_test(
    config: Option<PacketLossConfig>,
) -> Result<PacketLossResult, PacketLossError> {
    match config {
        Some(cfg) => {
            let test = PacketLossTest::new(cfg);
            test.run().await
        }
        None => {
            log::info!(
                "Packet loss measurement skipped: TURN server not configured"
            );
            Ok(PacketLossResult::unavailable())
        }
    }
}

/// Run packet loss measurement, returning unavailable on any error.
///
/// This function is a convenience wrapper that catches all errors and
/// returns an unavailable result instead of propagating the error.
/// This is useful when packet loss measurement is optional and should
/// not cause the overall speed test to fail.
///
/// # Arguments
/// * `config` - Optional TURN server configuration
///
/// # Returns
/// `PacketLossResult` - Always returns a result, never fails
pub async fn run_packet_loss_test_safe(
    config: Option<PacketLossConfig>,
) -> PacketLossResult {
    match run_packet_loss_test(config).await {
        Ok(result) => result,
        Err(e) => {
            log::warn!("Packet loss measurement failed: {}. Reporting as unavailable.", e);
            PacketLossResult::unavailable()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Helper function for tests - calculates packet loss ratio
    fn calculate_packet_loss_ratio(
        packets_sent: usize,
        packets_received: usize,
    ) -> f64 {
        assert!(
            packets_received <= packets_sent,
            "packets_received ({}) cannot exceed packets_sent ({})",
            packets_received,
            packets_sent
        );

        if packets_sent == 0 {
            return 0.0;
        }

        let packets_lost = packets_sent - packets_received;
        packets_lost as f64 / packets_sent as f64
    }

    // Unit tests for PacketLossConfig
    #[test]
    fn test_packet_loss_config_new() {
        let config =
            PacketLossConfig::new("turn:example.com:3478".to_string());

        assert_eq!(config.turn_server_uri, "turn:example.com:3478");
        assert_eq!(config.num_packets, PacketLossConfig::DEFAULT_NUM_PACKETS);
        assert_eq!(config.batch_size, PacketLossConfig::DEFAULT_BATCH_SIZE);
        assert_eq!(
            config.batch_wait_time_ms,
            PacketLossConfig::DEFAULT_BATCH_WAIT_TIME_MS
        );
        assert_eq!(
            config.packet_timeout_ms,
            PacketLossConfig::DEFAULT_PACKET_TIMEOUT_MS
        );
    }

    // Unit tests for PacketLossResult
    #[test]
    fn test_packet_loss_result_no_loss() {
        let result = PacketLossResult::new(100, 100, Some(15.5));

        assert!((result.packet_loss_ratio - 0.0).abs() < 0.001);
        assert_eq!(result.total_packets, 100);
        assert_eq!(result.packets_sent, 100);
        assert_eq!(result.packets_lost, 0);
        assert_eq!(result.packets_received, 100);
        assert!(result.is_available());
        assert!((result.packet_loss_percent() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_packet_loss_result_some_loss() {
        let result = PacketLossResult::new(100, 90, Some(20.0));

        assert!((result.packet_loss_ratio - 0.1).abs() < 0.001);
        assert_eq!(result.packets_lost, 10);
        assert_eq!(result.packets_received, 90);
        assert!((result.packet_loss_percent() - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_packet_loss_result_all_lost() {
        let result = PacketLossResult::new(100, 0, None);

        assert!((result.packet_loss_ratio - 1.0).abs() < 0.001);
        assert_eq!(result.packets_lost, 100);
        assert_eq!(result.packets_received, 0);
        assert!(result.avg_rtt_ms.is_none());
        assert!((result.packet_loss_percent() - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_packet_loss_result_unavailable() {
        let result = PacketLossResult::unavailable();

        assert!(!result.is_available());
        assert_eq!(result.packets_sent, 0);
        assert_eq!(result.packets_received, 0);
        assert_eq!(result.packets_lost, 0);
    }

    #[test]
    fn test_packet_loss_result_zero_packets() {
        let result = PacketLossResult::new(0, 0, None);

        assert!((result.packet_loss_ratio - 0.0).abs() < 0.001);
        assert!(!result.is_available());
    }

    #[test]
    #[should_panic(expected = "packets_received")]
    fn test_packet_loss_result_invalid() {
        // Should panic: received > sent
        let _ = PacketLossResult::new(50, 100, None);
    }

    // Unit tests for calculate_packet_loss_ratio
    #[test]
    fn test_calculate_packet_loss_ratio_no_loss() {
        let ratio = calculate_packet_loss_ratio(100, 100);
        assert!((ratio - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_packet_loss_ratio_some_loss() {
        let ratio = calculate_packet_loss_ratio(100, 90);
        assert!((ratio - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_calculate_packet_loss_ratio_all_lost() {
        let ratio = calculate_packet_loss_ratio(100, 0);
        assert!((ratio - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_packet_loss_ratio_zero_sent() {
        let ratio = calculate_packet_loss_ratio(0, 0);
        assert!((ratio - 0.0).abs() < 0.001);
    }

    #[test]
    #[should_panic(expected = "packets_received")]
    fn test_calculate_packet_loss_ratio_invalid() {
        // Should panic: received > sent
        let _ = calculate_packet_loss_ratio(50, 100);
    }

    // Unit tests for PacketLossTest URI parsing
    #[test]
    fn test_parse_turn_uri_with_port() {
        let config =
            PacketLossConfig::new("turn:example.com:3478".to_string());
        let test = PacketLossTest::new(config);

        let (host, port) = test.parse_turn_uri().unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 3478);
    }

    #[test]
    fn test_parse_turn_uri_without_port() {
        let config = PacketLossConfig::new("turn:example.com".to_string());
        let test = PacketLossTest::new(config);

        let (host, port) = test.parse_turn_uri().unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 3478); // Default port
    }

    #[test]
    fn test_parse_turn_uri_turns_scheme() {
        let config =
            PacketLossConfig::new("turns:secure.example.com:5349".to_string());
        let test = PacketLossTest::new(config);

        let (host, port) = test.parse_turn_uri().unwrap();
        assert_eq!(host, "secure.example.com");
        assert_eq!(port, 5349);
    }

    #[test]
    fn test_parse_turn_uri_no_scheme() {
        let config = PacketLossConfig::new("example.com:3478".to_string());
        let test = PacketLossTest::new(config);

        let (host, port) = test.parse_turn_uri().unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 3478);
    }

    #[test]
    fn test_parse_turn_uri_invalid_port() {
        let config =
            PacketLossConfig::new("turn:example.com:invalid".to_string());
        let test = PacketLossTest::new(config);

        let result = test.parse_turn_uri();
        assert!(result.is_err());
    }

    // Unit tests for packet creation and validation
    #[test]
    fn test_create_packet() {
        let config =
            PacketLossConfig::new("turn:example.com:3478".to_string());
        let test = PacketLossTest::new(config);

        let packet = test.create_packet(42);
        assert_eq!(packet.len(), 16);

        // Check sequence number
        let seq_bytes: [u8; 4] = packet[0..4].try_into().unwrap();
        let seq = u32::from_be_bytes(seq_bytes);
        assert_eq!(seq, 42);
    }

    #[test]
    fn test_validate_response_valid() {
        let config =
            PacketLossConfig::new("turn:example.com:3478".to_string());
        let test = PacketLossTest::new(config);

        let packet = test.create_packet(123);
        assert!(test.validate_response(&packet, 123));
    }

    #[test]
    fn test_validate_response_wrong_seq() {
        let config =
            PacketLossConfig::new("turn:example.com:3478".to_string());
        let test = PacketLossTest::new(config);

        let packet = test.create_packet(123);
        assert!(!test.validate_response(&packet, 456));
    }

    #[test]
    fn test_validate_response_too_short() {
        let config =
            PacketLossConfig::new("turn:example.com:3478".to_string());
        let test = PacketLossTest::new(config);

        let short_packet = vec![0u8; 3];
        assert!(!test.validate_response(&short_packet, 0));
    }

    // Tests for graceful handling of missing TURN configuration
    // Validates: Requirements 7.6
    #[tokio::test]
    async fn test_run_packet_loss_test_no_config() {
        use super::run_packet_loss_test;

        // When no config is provided, should return unavailable result
        let result = run_packet_loss_test(None).await.unwrap();

        assert!(!result.is_available());
        assert_eq!(result.packets_sent, 0);
        assert_eq!(result.packets_received, 0);
        assert_eq!(result.packets_lost, 0);
        assert!((result.packet_loss_ratio - 0.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_run_packet_loss_test_safe_no_config() {
        use super::run_packet_loss_test_safe;

        // When no config is provided, should return unavailable result
        let result = run_packet_loss_test_safe(None).await;

        assert!(!result.is_available());
        assert_eq!(result.packets_sent, 0);
    }

    #[test]
    fn test_packet_loss_result_unavailable_values() {
        // Verify all fields of unavailable result
        let result = PacketLossResult::unavailable();

        assert!(!result.is_available());
        assert_eq!(result.packet_loss_ratio, 0.0);
        assert_eq!(result.total_packets, 0);
        assert_eq!(result.packets_sent, 0);
        assert_eq!(result.packets_lost, 0);
        assert_eq!(result.packets_received, 0);
        assert!(result.avg_rtt_ms.is_none());
        assert_eq!(result.packet_loss_percent(), 0.0);
    }

    // Property-based tests for packet loss ratio calculation
    // Feature: cloudflare-speedtest-parity, Property 9: Packet Loss Ratio Calculation
    // Validates: Requirements 7.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: packet_loss_ratio = packets_lost / packets_sent
        /// where packets_lost = packets_sent - packets_received
        #[test]
        fn packet_loss_ratio_formula_correctness(
            packets_sent in 1usize..10000usize,
            // packets_received must be <= packets_sent
            received_ratio in 0.0f64..=1.0f64,
        ) {
            let packets_received =
                (packets_sent as f64 * received_ratio).floor() as usize;
            let packets_received = packets_received.min(packets_sent);

            let ratio = calculate_packet_loss_ratio(packets_sent, packets_received);

            // Calculate expected ratio manually
            let packets_lost = packets_sent - packets_received;
            let expected_ratio = packets_lost as f64 / packets_sent as f64;

            let tolerance = 1e-10;
            prop_assert!(
                (ratio - expected_ratio).abs() <= tolerance,
                "Ratio mismatch: got {}, expected {} (sent={}, received={}, lost={})",
                ratio,
                expected_ratio,
                packets_sent,
                packets_received,
                packets_lost
            );
        }

        /// Property: packet_loss_ratio SHALL be in the range [0.0, 1.0]
        #[test]
        fn packet_loss_ratio_in_valid_range(
            packets_sent in 1usize..10000usize,
            packets_received in 0usize..10000usize,
        ) {
            // Ensure packets_received <= packets_sent
            let packets_received = packets_received.min(packets_sent);

            let ratio = calculate_packet_loss_ratio(packets_sent, packets_received);

            prop_assert!(
                ratio >= 0.0 && ratio <= 1.0,
                "Ratio {} should be in [0.0, 1.0] (sent={}, received={})",
                ratio,
                packets_sent,
                packets_received
            );
        }

        /// Property: packets_lost = packets_sent - packets_received
        /// and packets_lost SHALL be non-negative
        #[test]
        fn packets_lost_is_non_negative(
            packets_sent in 0usize..10000usize,
            packets_received in 0usize..10000usize,
        ) {
            // Ensure packets_received <= packets_sent
            let packets_received = packets_received.min(packets_sent);

            let result = PacketLossResult::new(packets_sent, packets_received, None);

            // Verify the formula (packets_lost is usize, always non-negative)
            prop_assert_eq!(
                result.packets_lost,
                packets_sent.saturating_sub(packets_received),
                "packets_lost should equal packets_sent - packets_received"
            );
        }

        /// Property: When packets_received == packets_sent, ratio SHALL be 0.0
        #[test]
        fn no_loss_when_all_received(
            packets_sent in 1usize..10000usize,
        ) {
            let ratio = calculate_packet_loss_ratio(packets_sent, packets_sent);

            prop_assert!(
                ratio.abs() < 1e-10,
                "Ratio should be 0.0 when all packets received, got {}",
                ratio
            );
        }

        /// Property: When packets_received == 0, ratio SHALL be 1.0
        #[test]
        fn full_loss_when_none_received(
            packets_sent in 1usize..10000usize,
        ) {
            let ratio = calculate_packet_loss_ratio(packets_sent, 0);

            prop_assert!(
                (ratio - 1.0).abs() < 1e-10,
                "Ratio should be 1.0 when no packets received, got {}",
                ratio
            );
        }

        /// Property: PacketLossResult.packet_loss_ratio matches
        /// calculate_packet_loss_ratio function
        #[test]
        fn result_ratio_matches_function(
            packets_sent in 1usize..10000usize,
            packets_received in 0usize..10000usize,
        ) {
            let packets_received = packets_received.min(packets_sent);

            let result = PacketLossResult::new(packets_sent, packets_received, None);
            let function_ratio =
                calculate_packet_loss_ratio(packets_sent, packets_received);

            let tolerance = 1e-10;
            prop_assert!(
                (result.packet_loss_ratio - function_ratio).abs() <= tolerance,
                "Result ratio {} should match function ratio {}",
                result.packet_loss_ratio,
                function_ratio
            );
        }

        /// Property: packet_loss_percent = packet_loss_ratio * 100
        #[test]
        fn packet_loss_percent_is_ratio_times_100(
            packets_sent in 1usize..10000usize,
            packets_received in 0usize..10000usize,
        ) {
            let packets_received = packets_received.min(packets_sent);

            let result = PacketLossResult::new(packets_sent, packets_received, None);

            let expected_percent = result.packet_loss_ratio * 100.0;
            let tolerance = 1e-10;

            prop_assert!(
                (result.packet_loss_percent() - expected_percent).abs() <= tolerance,
                "packet_loss_percent {} should equal ratio * 100 = {}",
                result.packet_loss_percent(),
                expected_percent
            );
        }
    }
}
