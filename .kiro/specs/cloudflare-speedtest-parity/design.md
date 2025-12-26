# Design Document

## Overview

This design document describes the architecture and implementation approach for completing the `cloud-speed` CLI tool to achieve full feature parity with Cloudflare's official speed test. The design builds upon the existing codebase, fixing incomplete implementations and adding missing features including loaded latency, packet loss measurement, and AIM scoring.

## Architecture

The system follows a layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Layer                             │
│                      (src/main.rs)                          │
│         Argument parsing, output formatting, orchestration   │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Test Engine Layer                         │
│                (src/cloudflare/tests/)                       │
│    Download, Upload, Latency, PacketLoss test implementations│
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Client Layer                              │
│               (src/cloudflare/client.rs)                     │
│         HTTP client, request execution, timing extraction    │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                  Measurement Layer                           │
│          (src/measurements.rs, src/stats.rs)                 │
│    Statistical calculations, bandwidth, latency, jitter      │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Scoring Layer                             │
│                   (src/scoring.rs)                           │
│              AIM score calculation and categorization        │
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. Test Engine (`src/cloudflare/tests/mod.rs`)

The test engine orchestrates all measurements and manages the test sequence.

```rust
pub struct TestEngine {
    client: Client,
    config: TestConfig,
}

pub struct TestConfig {
    pub download_sizes: Vec<DataBlock>,
    pub upload_sizes: Vec<DataBlock>,
    pub latency_packets: usize,
    pub loaded_latency_throttle_ms: u64,
    pub bandwidth_finish_duration_ms: u64,
    pub bandwidth_min_duration_ms: u64,
    pub loaded_request_min_duration_ms: u64,
    pub packet_loss_config: Option<PacketLossConfig>,
}

pub struct DataBlock {
    pub bytes: u64,
    pub count: usize,
    pub bypass_min_duration: bool,
}

impl TestEngine {
    pub async fn run(&self) -> Result<TestResults, Box<dyn Error>>;
    pub async fn run_latency(&self, num_packets: usize) -> Result<Vec<f64>, Box<dyn Error>>;
    pub async fn run_download(&self, bytes: u64, count: usize) -> Result<Vec<BandwidthMeasurement>, Box<dyn Error>>;
    pub async fn run_upload(&self, bytes: u64, count: usize) -> Result<Vec<BandwidthMeasurement>, Box<dyn Error>>;
    pub async fn run_packet_loss(&self) -> Result<PacketLossResult, Box<dyn Error>>;
}
```

### 2. Download Test (`src/cloudflare/tests/download.rs`)

Enhanced download test with loaded latency support.

```rust
pub struct DownloadTest {
    bytes: u64,
}

pub struct DownloadResult {
    pub timing: TimingBreakdown,
    pub bandwidth_bps: f64,
    pub transfer_size: u64,
}

pub struct TimingBreakdown {
    pub dns_duration: Duration,
    pub tcp_duration: Duration,
    pub tls_duration: Duration,
    pub ttfb_duration: Duration,
    pub transfer_duration: Duration,
    pub total_duration: Duration,
    pub server_time: Duration,
}

impl DownloadTest {
    pub async fn run(&self) -> Result<DownloadResult, Box<dyn Error>>;
    pub async fn run_with_loaded_latency(
        &self,
        latency_tx: mpsc::Sender<f64>,
        throttle_ms: u64,
    ) -> Result<DownloadResult, Box<dyn Error>>;
}
```

### 3. Upload Test (`src/cloudflare/tests/upload.rs`)

New upload test implementation with proper timing.

```rust
pub struct UploadTest {
    bytes: u64,
    data: Vec<u8>,
}

pub struct UploadResult {
    pub timing: TimingBreakdown,
    pub bandwidth_bps: f64,
    pub transfer_size: u64,
}

impl UploadTest {
    pub fn new(bytes: u64) -> Self;
    pub async fn run(&self) -> Result<UploadResult, Box<dyn Error>>;
    pub async fn run_with_loaded_latency(
        &self,
        latency_tx: mpsc::Sender<f64>,
        throttle_ms: u64,
    ) -> Result<UploadResult, Box<dyn Error>>;
}
```

### 4. Packet Loss Test (`src/cloudflare/tests/packet_loss.rs`)

New component for UDP packet loss measurement.

```rust
pub struct PacketLossConfig {
    pub turn_server_uri: String,
    pub turn_username: String,
    pub turn_password: String,
    pub num_packets: usize,
    pub response_wait_time_ms: u64,
    pub batch_size: usize,
    pub batch_wait_time_ms: u64,
}

pub struct PacketLossResult {
    pub packet_loss_ratio: f64,
    pub total_packets: usize,
    pub packets_sent: usize,
    pub packets_lost: usize,
}

pub struct PacketLossTest {
    config: PacketLossConfig,
}

impl PacketLossTest {
    pub async fn run(&self) -> Result<PacketLossResult, Box<dyn Error>>;
}
```

### 5. Statistics Module (`src/stats.rs`)

Enhanced statistics with percentile calculations.

```rust
pub fn median_f64(values: &mut [f64]) -> Option<f64>;
pub fn percentile_f64(values: &mut [f64], p: f64) -> Option<f64>;
pub fn mean_f64(values: &[f64]) -> Option<f64>;
pub fn jitter_f64(values: &[f64]) -> Option<f64>;
```

### 6. Measurements Module (`src/measurements.rs`)

Bandwidth and latency calculation functions.

```rust
pub fn calculate_bandwidth_bps(bytes: u64, duration: Duration, server_time: Duration) -> f64;
pub fn calculate_speed_mbps(bandwidth_bps: f64) -> f64;
pub fn aggregate_bandwidth(measurements: &[BandwidthMeasurement], percentile: f64, min_duration_ms: f64) -> Option<f64>;
```

### 7. Scoring Module (`src/scoring.rs`)

AIM score calculation.

```rust
pub struct AimScores {
    pub streaming: QualityScore,
    pub gaming: QualityScore,
    pub video_conferencing: QualityScore,
}

pub enum QualityScore {
    Great,
    Good,
    Average,
    Poor,
}

pub struct ConnectionMetrics {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub latency_ms: f64,
    pub jitter_ms: f64,
    pub packet_loss: Option<f64>,
    pub loaded_latency_down_ms: Option<f64>,
    pub loaded_latency_up_ms: Option<f64>,
}

pub fn calculate_aim_scores(metrics: &ConnectionMetrics) -> AimScores;
```

## Data Models

### Test Results

```rust
pub struct SpeedTestResults {
    pub timestamp: DateTime<Utc>,
    pub meta: ConnectionMeta,
    pub location: ServerLocation,
    pub latency: LatencyResults,
    pub download: BandwidthResults,
    pub upload: BandwidthResults,
    pub packet_loss: Option<PacketLossResult>,
    pub scores: AimScores,
}

pub struct LatencyResults {
    pub idle_ms: f64,
    pub idle_jitter_ms: f64,
    pub loaded_down_ms: Option<f64>,
    pub loaded_down_jitter_ms: Option<f64>,
    pub loaded_up_ms: Option<f64>,
    pub loaded_up_jitter_ms: Option<f64>,
}

pub struct BandwidthResults {
    pub speed_mbps: f64,
    pub measurements: Vec<SizeMeasurement>,
}

pub struct SizeMeasurement {
    pub bytes: u64,
    pub speed_mbps: f64,
    pub count: usize,
}
```

### Bandwidth Measurement

```rust
pub struct BandwidthMeasurement {
    pub bytes: u64,
    pub bandwidth_bps: f64,
    pub duration_ms: f64,
    pub server_time_ms: f64,
    pub ttfb_ms: f64,
}
```



## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Based on the prework analysis, the following correctness properties have been identified:

### Property 1: Median Calculation Correctness

*For any* non-empty slice of f64 values, the median function SHALL return a value that is:
- Equal to the middle element when the slice has odd length
- Equal to the average of the two middle elements when the slice has even length
- Always between the minimum and maximum values in the slice (inclusive)

**Validates: Requirements 2.4**

### Property 2: Jitter Calculation Correctness

*For any* sequence of at least 2 latency measurements, the jitter calculation SHALL return the mean of absolute differences between consecutive measurements. Specifically:
- jitter = sum(|measurements[i+1] - measurements[i]|) / (n-1) for i in 0..n-1
- Jitter SHALL always be non-negative

**Validates: Requirements 3.1**

### Property 3: Bandwidth Calculation Correctness

*For any* transfer with known bytes, duration, and server processing time:
- bandwidth_bps = (bytes * 8) / (duration_seconds - server_time_seconds)
- The calculation SHALL exclude server processing time from the duration
- bandwidth_bps SHALL be positive when duration > server_time

**Validates: Requirements 4.2, 2.6, 5.3**

### Property 4: Percentile Aggregation Correctness

*For any* non-empty slice of bandwidth measurements and percentile value p (0 < p < 1):
- The percentile function SHALL return a value at or below which p% of the data falls
- For p=0.9 (90th percentile), the result SHALL be greater than or equal to at least 90% of the values
- The result SHALL always be between the minimum and maximum values (inclusive)

**Validates: Requirements 4.3, 5.4**

### Property 5: Minimum Duration Filtering

*For any* set of bandwidth measurements with varying durations:
- Only measurements with duration >= 10ms SHALL be included in bandwidth aggregation
- Measurements with duration < 10ms SHALL be excluded from the final calculation
- The filtered set SHALL preserve the relative ordering of included measurements

**Validates: Requirements 4.4, 5.5**

### Property 6: Early Termination Logic

*For any* sequence of measurement sets with increasing file sizes:
- WHEN any measurement in a set has duration >= 1000ms, subsequent larger file sizes SHALL be skipped
- Measurements already collected SHALL be preserved
- The termination check SHALL be performed after each measurement set completes

**Validates: Requirements 4.5, 5.6**

### Property 7: Loaded Latency Duration Filtering

*For any* set of loaded latency measurements taken during bandwidth tests:
- Only latency measurements taken during requests with duration >= 250ms SHALL be included
- Latency measurements during shorter requests SHALL be excluded
- This filtering SHALL apply independently to download and upload directions

**Validates: Requirements 6.4**

### Property 8: Loaded Latency Capacity Constraint

*For any* collection of loaded latency measurements:
- The collection SHALL contain at most 20 data points per direction (download/upload)
- WHEN more than 20 measurements are available, only the most recent 20 SHALL be kept
- Older measurements SHALL be discarded in FIFO order

**Validates: Requirements 6.5**

### Property 9: Packet Loss Ratio Calculation

*For any* packet loss measurement with packets_sent > 0:
- packet_loss_ratio = packets_lost / packets_sent
- packet_loss_ratio SHALL be in the range [0.0, 1.0]
- packets_lost = packets_sent - packets_received
- packets_lost SHALL be non-negative

**Validates: Requirements 7.4**

### Property 10: AIM Score Categorization

*For any* set of connection metrics (download, upload, latency, jitter, packet_loss):
- Each AIM score (streaming, gaming, video_conferencing) SHALL be exactly one of: Great, Good, Average, Poor
- The categorization SHALL be deterministic (same inputs always produce same outputs)
- Better metrics SHALL never produce a worse score than poorer metrics

**Validates: Requirements 8.3**

### Property 11: Server-Timing Header Parsing

*For any* valid server-timing header in the format `cfRequestDuration;dur=X.XX`:
- The parser SHALL extract the duration value as f64 milliseconds
- Parsing then formatting SHALL produce an equivalent value (round-trip)
- Invalid headers SHALL return an error or default value, not panic

**Validates: Requirements 12.5**

## Error Handling

### Network Errors

- Connection failures: Retry up to 3 times with exponential backoff
- Timeout errors: Use 30-second timeout for individual requests
- DNS resolution failures: Fall back to system resolver, then fail gracefully

### API Errors

- HTTP 4xx errors: Log and report to user, continue with other tests
- HTTP 5xx errors: Retry up to 3 times, then skip affected test
- Invalid JSON responses: Log error, use default values where safe

### Measurement Errors

- Empty measurement sets: Report as unavailable, don't calculate statistics
- All measurements filtered out: Report as unavailable
- Negative durations (clock skew): Discard measurement, log warning

### TURN Server Errors

- Connection failure: Skip packet loss test, report as unavailable
- Authentication failure: Log error, skip packet loss test
- Timeout: Use partial results if available

## Testing Strategy

### Unit Tests

Unit tests verify specific examples and edge cases:

1. **Statistics functions** (`src/stats.rs`)
   - Median with odd/even length arrays
   - Median with single element
   - Percentile at boundaries (0%, 50%, 100%)
   - Empty array handling

2. **Measurement calculations** (`src/measurements.rs`)
   - Bandwidth calculation with known values
   - Speed conversion (bps to Mbps)
   - Jitter with known sequences

3. **Header parsing**
   - Valid server-timing headers
   - Malformed headers
   - Missing headers

4. **Scoring logic** (`src/scoring.rs`)
   - Score boundaries for each category
   - Edge cases at category transitions

### Property-Based Tests

Property-based tests verify universal properties across many generated inputs. Each test runs minimum 100 iterations.

1. **Statistical Properties**
   - Median is always between min and max
   - Percentile ordering (p1 < p2 implies percentile(p1) <= percentile(p2))
   - Jitter is always non-negative

2. **Calculation Properties**
   - Bandwidth calculation correctness
   - Round-trip for server-timing parsing

3. **Filtering Properties**
   - Duration filtering preserves valid measurements
   - Capacity constraints are enforced

### Integration Tests

Integration tests verify component interactions:

1. **API Client Tests** (`src/cloudflare/tests/`)
   - Mock HTTP responses for download/upload endpoints
   - Verify timing extraction from responses
   - Test error handling for various HTTP status codes

2. **Test Engine Tests**
   - Verify measurement sequence execution
   - Test early termination logic
   - Verify loaded latency collection during bandwidth tests

### Test Configuration

- Use `proptest` crate for property-based testing in Rust
- Configure minimum 100 iterations per property test
- Tag each property test with the design property it validates
- Example tag format: `// Feature: cloudflare-speedtest-parity, Property 1: Median Calculation Correctness`
