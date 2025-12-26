# Requirements Document

## Introduction

This specification defines the requirements for completing the `cloud-speed` CLI tool to achieve full feature parity with the official Cloudflare Speed Test at speed.cloudflare.com. The current implementation has partial functionality with several features commented out or incomplete. This document outlines the complete set of requirements to match the official speed test's measurement capabilities, including download/upload bandwidth, latency (idle and loaded), jitter, packet loss, and AIM scoring.

## Glossary

- **Speed_Test_Engine**: The core component that orchestrates all network measurements
- **Download_Test**: A test that measures download bandwidth by requesting data blocks from the Cloudflare API
- **Upload_Test**: A test that measures upload bandwidth by posting data blocks to the Cloudflare API
- **Latency_Measurement**: The round-trip time (RTT) for a packet to travel to Cloudflare's network and back
- **Idle_Latency**: Latency measured when no data transfer is occurring
- **Loaded_Latency**: Latency measured while download or upload tests are running
- **Jitter**: The average variation between consecutive latency measurements
- **Packet_Loss**: The percentage of UDP packets that fail to reach their destination
- **TURN_Server**: A relay server used for packet loss measurement via WebRTC/UDP
- **AIM_Score**: Aggregated Internet Measurement score that categorizes connection quality for streaming, gaming, and video conferencing
- **Data_Block**: A predefined size of data used in bandwidth measurements
- **TTFB**: Time To First Byte - the time from request start to receiving the first response byte
- **Server_Timing**: HTTP header containing server-side processing time to subtract from measurements
- **Bandwidth**: Data transfer rate measured in bits per second (bps)
- **Percentile**: A statistical measure indicating the value below which a given percentage of observations fall

## Requirements

### Requirement 1: Connection Metadata Retrieval

**User Story:** As a user, I want to see information about my connection and the test server, so that I understand the context of my speed test results.

#### Acceptance Criteria

1. WHEN the speed test starts, THE Speed_Test_Engine SHALL retrieve connection metadata from the `/meta` endpoint
2. THE Speed_Test_Engine SHALL display the server location (city and IATA code)
3. THE Speed_Test_Engine SHALL display the user's ISP name and ASN
4. THE Speed_Test_Engine SHALL display the user's IP address and country code
5. WHEN JSON output is requested, THE Speed_Test_Engine SHALL include all metadata fields in the output

### Requirement 2: Idle Latency Measurement

**User Story:** As a user, I want to measure my connection's idle latency, so that I understand the baseline round-trip time to Cloudflare's network.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL measure idle latency by sending requests with zero bytes to the download endpoint
2. THE Speed_Test_Engine SHALL perform a minimum of 20 latency measurements
3. THE Speed_Test_Engine SHALL calculate latency as the round-trip time from request start to response start (TTFB)
4. THE Speed_Test_Engine SHALL report the median (50th percentile) latency value
5. THE Speed_Test_Engine SHALL display latency in milliseconds with 2 decimal places precision
6. WHEN calculating latency, THE Speed_Test_Engine SHALL subtract server processing time from the measurement

### Requirement 3: Idle Jitter Measurement

**User Story:** As a user, I want to measure my connection's jitter, so that I understand the stability of my network connection.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL calculate jitter as the mean absolute difference between consecutive latency measurements
2. THE Speed_Test_Engine SHALL require at least 2 latency measurements to calculate jitter
3. THE Speed_Test_Engine SHALL display jitter in milliseconds with 2 decimal places precision
4. WHEN fewer than 2 measurements exist, THE Speed_Test_Engine SHALL report jitter as unavailable

### Requirement 4: Download Bandwidth Measurement

**User Story:** As a user, I want to measure my download speed across multiple file sizes, so that I understand my connection's download performance.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL perform download tests with the following data block sizes and counts:
   - 100KB: 10 measurements (with 1 initial estimation)
   - 1MB: 8 measurements
   - 10MB: 6 measurements
   - 25MB: 4 measurements
   - 100MB: 3 measurements
   - 250MB: 2 measurements (optional, for high-bandwidth connections)
2. THE Speed_Test_Engine SHALL calculate bandwidth by dividing transfer size (in bits) by duration (excluding server time)
3. THE Speed_Test_Engine SHALL use the 90th percentile of measurements for the final download speed
4. THE Speed_Test_Engine SHALL only include measurements longer than 10ms in bandwidth calculations
5. THE Speed_Test_Engine SHALL stop testing larger file sizes once a measurement set reaches 1000ms duration
6. THE Speed_Test_Engine SHALL display download speed in Mbps with 2 decimal places precision
7. THE Speed_Test_Engine SHALL display individual speeds for each file size category

### Requirement 5: Upload Bandwidth Measurement

**User Story:** As a user, I want to measure my upload speed across multiple file sizes, so that I understand my connection's upload performance.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL perform upload tests with the following data block sizes and counts:
   - 100KB: 8 measurements
   - 1MB: 6 measurements
   - 10MB: 4 measurements
   - 25MB: 4 measurements
   - 50MB: 3 measurements
2. THE Speed_Test_Engine SHALL POST data to the `/__up` endpoint
3. THE Speed_Test_Engine SHALL calculate bandwidth by dividing transfer size (in bits) by duration (excluding server time)
4. THE Speed_Test_Engine SHALL use the 90th percentile of measurements for the final upload speed
5. THE Speed_Test_Engine SHALL only include measurements longer than 10ms in bandwidth calculations
6. THE Speed_Test_Engine SHALL stop testing larger file sizes once a measurement set reaches 1000ms duration
7. THE Speed_Test_Engine SHALL display upload speed in Mbps with 2 decimal places precision

### Requirement 6: Loaded Latency Measurement

**User Story:** As a user, I want to measure latency while my connection is under load, so that I can detect bufferbloat and understand real-world performance.

#### Acceptance Criteria

1. WHILE download tests are running, THE Speed_Test_Engine SHALL perform concurrent latency measurements
2. WHILE upload tests are running, THE Speed_Test_Engine SHALL perform concurrent latency measurements
3. THE Speed_Test_Engine SHALL throttle loaded latency requests to every 400ms
4. THE Speed_Test_Engine SHALL only include latency measurements taken during requests longer than 250ms
5. THE Speed_Test_Engine SHALL keep a maximum of 20 loaded latency data points per direction
6. THE Speed_Test_Engine SHALL report separate loaded latency values for download and upload
7. THE Speed_Test_Engine SHALL calculate loaded jitter for both download and upload directions

### Requirement 7: Packet Loss Measurement

**User Story:** As a user, I want to measure packet loss on my connection, so that I understand if packets are being dropped.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL measure packet loss using UDP packets via a TURN server
2. THE Speed_Test_Engine SHALL send 1000 packets for packet loss measurement
3. THE Speed_Test_Engine SHALL wait 3000ms for responses after sending all packets
4. THE Speed_Test_Engine SHALL calculate packet loss as the ratio of lost packets to total packets sent
5. THE Speed_Test_Engine SHALL display packet loss as a percentage with 2 decimal places
6. IF TURN server credentials are not configured, THEN THE Speed_Test_Engine SHALL skip packet loss measurement and indicate it is unavailable
7. THE Speed_Test_Engine SHALL support configurable TURN server URI and credentials

### Requirement 8: AIM Score Calculation

**User Story:** As a user, I want to see quality scores for different use cases, so that I understand how well my connection supports streaming, gaming, and video calls.

#### Acceptance Criteria

1. WHEN all measurements are complete, THE Speed_Test_Engine SHALL calculate AIM scores
2. THE Speed_Test_Engine SHALL provide separate scores for:
   - Video Streaming quality
   - Online Gaming quality  
   - Video Conferencing quality
3. THE Speed_Test_Engine SHALL categorize each score as "Great", "Good", "Average", or "Poor"
4. THE Speed_Test_Engine SHALL base scores on download bandwidth, upload bandwidth, latency, jitter, and packet loss
5. WHEN JSON output is requested, THE Speed_Test_Engine SHALL include all AIM scores in the output

### Requirement 9: Test Execution Sequence

**User Story:** As a user, I want the speed test to run efficiently, so that I get accurate results in a reasonable time.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL execute measurements in the following sequence:
   - Initial latency estimation (1 packet)
   - Initial download estimation (100KB, 1 request)
   - Full latency measurement (20 packets)
   - Download tests (ramping up file sizes)
   - Upload tests (ramping up file sizes)
   - Packet loss measurement (if configured)
2. THE Speed_Test_Engine SHALL interleave download and upload tests of similar sizes
3. THE Speed_Test_Engine SHALL run measurements concurrently where appropriate
4. THE Speed_Test_Engine SHALL complete all tests within a reasonable time (under 60 seconds for typical connections)

### Requirement 10: Output Formatting

**User Story:** As a user, I want flexible output options, so that I can use the results in different contexts.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL support human-readable colored terminal output (default)
2. THE Speed_Test_Engine SHALL support JSON output via `--json` flag
3. THE Speed_Test_Engine SHALL support pretty-printed JSON via `--json --pretty` flags
4. WHEN outputting JSON, THE Speed_Test_Engine SHALL include:
   - Timestamp
   - All metadata fields
   - All latency measurements (idle and loaded)
   - All jitter values
   - All bandwidth measurements by file size
   - Final download and upload speeds
   - Packet loss (if measured)
   - AIM scores
5. THE Speed_Test_Engine SHALL display progress as tests complete in human-readable mode
6. THE Speed_Test_Engine SHALL use consistent units throughout output (Mbps for speed, ms for latency)

### Requirement 11: Error Handling

**User Story:** As a user, I want clear error messages when something goes wrong, so that I can understand and resolve issues.

#### Acceptance Criteria

1. IF the network is unavailable, THEN THE Speed_Test_Engine SHALL display a clear error message
2. IF the Cloudflare API returns an error, THEN THE Speed_Test_Engine SHALL display the error details
3. IF a measurement times out, THEN THE Speed_Test_Engine SHALL retry up to 3 times before failing
4. IF all retries fail, THEN THE Speed_Test_Engine SHALL continue with remaining tests and note the failure
5. THE Speed_Test_Engine SHALL use appropriate exit codes (0 for success, non-zero for errors)
6. WHEN verbose mode is enabled, THE Speed_Test_Engine SHALL log detailed timing information

### Requirement 12: Timing Precision

**User Story:** As a developer, I want accurate timing measurements, so that the speed test results are reliable.

#### Acceptance Criteria

1. THE Speed_Test_Engine SHALL measure DNS lookup time separately
2. THE Speed_Test_Engine SHALL measure TCP handshake time separately
3. THE Speed_Test_Engine SHALL measure TLS handshake time separately
4. THE Speed_Test_Engine SHALL measure TTFB (time to first byte) separately
5. THE Speed_Test_Engine SHALL extract server processing time from the `server-timing` header
6. THE Speed_Test_Engine SHALL subtract server processing time from bandwidth calculations
7. THE Speed_Test_Engine SHALL use high-precision timing (sub-millisecond accuracy)
8. THE Speed_Test_Engine SHALL use f64 for all timing calculations to maintain precision
