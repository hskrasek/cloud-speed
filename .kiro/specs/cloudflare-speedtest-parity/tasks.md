# Implementation Plan: Cloudflare Speed Test Parity

## Overview

This implementation plan breaks down the work needed to achieve full feature parity with speed.cloudflare.com. The tasks are organized to build incrementally, starting with core statistical functions, then measurement infrastructure, followed by test implementations, and finally output formatting and scoring.

## Tasks

- [-] 1. Enhance statistics module with percentile and improved calculations
  - [x] 1.1 Implement `percentile_f64` function for calculating arbitrary percentiles
    - Add function to `src/stats.rs` that calculates the p-th percentile of a slice
    - Handle edge cases: empty slice, single element, p=0, p=1
    - Use linear interpolation between values for non-integer positions
    - _Requirements: 4.3, 5.4_
  - [x] 1.2 Write property test for percentile calculation
    - **Property 4: Percentile Aggregation Correctness**
    - **Validates: Requirements 4.3, 5.4**
  - [x] 1.3 Implement `mean_f64` function for calculating arithmetic mean
    - Add function to `src/stats.rs`
    - Handle empty slice case
    - _Requirements: 3.1_
  - [x] 1.4 Write property test for median calculation
    - **Property 1: Median Calculation Correctness**
    - **Validates: Requirements 2.4**

- [x] 2. Enhance measurements module with bandwidth and jitter calculations
  - [x] 2.1 Implement `calculate_bandwidth_bps` function
    - Add function to `src/measurements.rs`
    - Calculate bandwidth as (bytes * 8) / (duration - server_time) in seconds
    - Handle edge case where duration <= server_time
    - _Requirements: 4.2, 2.6, 5.3_
  - [x] 2.2 Write property test for bandwidth calculation
    - **Property 3: Bandwidth Calculation Correctness**
    - **Validates: Requirements 4.2, 2.6, 5.3**
  - [x] 2.3 Implement `calculate_speed_mbps` function
    - Convert bandwidth in bps to Mbps (divide by 1e6)
    - _Requirements: 4.6, 5.7_
  - [x] 2.4 Refactor `jitter_f64` to use mean of absolute differences
    - Update existing function in `src/measurements.rs`
    - Ensure it returns None for fewer than 2 measurements
    - _Requirements: 3.1, 3.2_
  - [x] 2.5 Write property test for jitter calculation
    - **Property 2: Jitter Calculation Correctness**
    - **Validates: Requirements 3.1**

- [x] 3. Checkpoint - Ensure statistical functions are correct
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Implement bandwidth measurement aggregation
  - [x] 4.1 Create `BandwidthMeasurement` struct
    - Add to `src/measurements.rs`
    - Fields: bytes, bandwidth_bps, duration_ms, server_time_ms, ttfb_ms
    - _Requirements: 4.2_
  - [x] 4.2 Implement `aggregate_bandwidth` function with filtering
    - Filter out measurements with duration < min_duration_ms (10ms default)
    - Calculate percentile of remaining measurements
    - Return None if all measurements filtered out
    - _Requirements: 4.4, 5.5_
  - [x] 4.3 Write property test for minimum duration filtering
    - **Property 5: Minimum Duration Filtering**
    - **Validates: Requirements 4.4, 5.5**

- [x] 5. Implement server-timing header parsing
  - [x] 5.1 Add `parse_server_timing` function to extract server processing time
    - Parse header format: `cfRequestDuration;dur=X.XX`
    - Return Duration from the extracted milliseconds
    - Handle missing or malformed headers gracefully
    - _Requirements: 12.5_
  - [x] 5.2 Write property test for server-timing parsing
    - **Property 11: Server-Timing Header Parsing**
    - **Validates: Requirements 12.5**

- [x] 6. Refactor download test implementation
  - [x] 6.1 Update `TestResults` struct with complete timing breakdown
    - Ensure all timing fields are populated correctly
    - Add server_time field extracted from server-timing header
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_
  - [x] 6.2 Update download test to calculate bandwidth correctly
    - Use transfer duration (end - ttfb) minus server time
    - Calculate bandwidth_bps for each measurement
    - _Requirements: 4.2_
  - [x] 6.3 Implement loaded latency measurement during downloads
    - Add concurrent latency requests during download
    - Throttle to every 400ms
    - Only include latency during requests >= 250ms
    - _Requirements: 6.1, 6.3, 6.4_

- [x] 7. Implement upload test
  - [x] 7.1 Create `UploadTest` struct in `src/cloudflare/tests/upload.rs`
    - Similar structure to DownloadTest
    - Generate payload data of specified size
    - _Requirements: 5.1, 5.2_
  - [x] 7.2 Implement upload test execution with timing
    - POST data to `/__up` endpoint
    - Extract timing breakdown (DNS, TCP, TLS, TTFB, transfer)
    - Parse server-timing header for server processing time
    - _Requirements: 5.2, 12.1, 12.2, 12.3, 12.4, 12.5_
  - [x] 7.3 Implement loaded latency measurement during uploads
    - Add concurrent latency requests during upload
    - Throttle to every 400ms
    - Only include latency during requests >= 250ms
    - _Requirements: 6.2, 6.3, 6.4_

- [x] 8. Checkpoint - Ensure download and upload tests work correctly
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Implement loaded latency collection and filtering
  - [x] 9.1 Create `LoadedLatencyCollector` struct
    - Maintain separate collections for download and upload directions
    - Enforce maximum 20 data points per direction
    - Implement FIFO eviction when capacity exceeded
    - _Requirements: 6.5_
  - [x] 9.2 Write property test for loaded latency capacity constraint
    - **Property 8: Loaded Latency Capacity Constraint**
    - **Validates: Requirements 6.5**
  - [x] 9.3 Implement loaded latency duration filtering
    - Filter out latency measurements taken during requests < 250ms
    - _Requirements: 6.4_
  - [x] 9.4 Write property test for loaded latency duration filtering
    - **Property 7: Loaded Latency Duration Filtering**
    - **Validates: Requirements 6.4**

- [x] 10. Implement test engine with measurement sequence
  - [x] 10.1 Create `TestEngine` struct with configuration
    - Define default data block sizes and counts for download/upload
    - Configure latency packet count, throttle intervals, duration thresholds
    - _Requirements: 4.1, 5.1, 9.1_
  - [x] 10.2 Implement measurement sequence execution
    - Execute in order: initial latency, initial download, full latency, downloads, uploads
    - Interleave download and upload tests of similar sizes
    - _Requirements: 9.1, 9.2_
  - [x] 10.3 Implement early termination logic
    - Stop testing larger file sizes when measurement reaches 1000ms
    - Apply separately to download and upload directions
    - _Requirements: 4.5, 5.6_
  - [x] 10.4 Write property test for early termination logic
    - **Property 6: Early Termination Logic**
    - **Validates: Requirements 4.5, 5.6**

- [x] 11. Implement packet loss measurement (optional feature)
  - [x] 11.1 Create `PacketLossConfig` struct for TURN server configuration
    - Fields: turn_server_uri, username, password, num_packets, timeouts
    - _Requirements: 7.7_
  - [x] 11.2 Implement UDP packet loss test using TURN server
    - Send specified number of UDP packets
    - Wait for responses with timeout
    - Calculate packet loss ratio
    - _Requirements: 7.1, 7.2, 7.3, 7.4_
  - [x] 11.3 Write property test for packet loss ratio calculation
    - **Property 9: Packet Loss Ratio Calculation**
    - **Validates: Requirements 7.4**
  - [x] 11.4 Handle missing TURN configuration gracefully
    - Skip packet loss test if not configured
    - Report as unavailable in results
    - _Requirements: 7.6_

- [x] 12. Checkpoint - Ensure all measurements work correctly
  - Ensure all tests pass, ask the user if questions arise.

- [x] 13. Implement AIM scoring
  - [x] 13.1 Create `src/scoring.rs` module with score types
    - Define `QualityScore` enum: Great, Good, Average, Poor
    - Define `AimScores` struct with streaming, gaming, video_conferencing fields
    - Define `ConnectionMetrics` struct for input data
    - _Requirements: 8.2_
  - [x] 13.2 Implement `calculate_aim_scores` function
    - Calculate scores based on download, upload, latency, jitter, packet_loss
    - Apply appropriate thresholds for each use case
    - _Requirements: 8.1, 8.4_
  - [x] 13.3 Write property test for AIM score categorization
    - **Property 10: AIM Score Categorization**
    - **Validates: Requirements 8.3**

- [x] 14. Implement result data structures
  - [x] 14.1 Create `SpeedTestResults` struct
    - Include all measurement results, metadata, and scores
    - Implement Serialize for JSON output
    - _Requirements: 10.4_
  - [x] 14.2 Create `LatencyResults` struct
    - Include idle and loaded latency/jitter for both directions
    - _Requirements: 2.4, 3.1, 6.6, 6.7_
  - [x] 14.3 Create `BandwidthResults` struct
    - Include final speed and per-size measurements
    - _Requirements: 4.7_

- [x] 15. Update main.rs with complete test execution
  - [x] 15.1 Integrate TestEngine into main function
    - Replace existing ad-hoc test execution with TestEngine
    - Execute full measurement sequence
    - _Requirements: 9.1_
  - [x] 15.2 Implement human-readable output formatting
    - Display all measurements with proper formatting
    - Use colors for different metric types
    - Show progress as tests complete
    - _Requirements: 10.1, 10.5, 10.6_
  - [x] 15.3 Implement JSON output formatting
    - Serialize SpeedTestResults to JSON
    - Support pretty-printing with --pretty flag
    - _Requirements: 10.2, 10.3, 10.4_
  - [x] 15.4 Add CLI arguments for TURN server configuration
    - Add optional flags for TURN server URI and credentials
    - _Requirements: 7.7_

- [x] 16. Implement error handling and retry logic
  - [x] 16.1 Add retry logic for failed measurements
    - Retry up to 3 times with exponential backoff
    - Continue with remaining tests on persistent failure
    - _Requirements: 11.3, 11.4_
  - [x] 16.2 Implement proper error messages
    - Display clear messages for network errors
    - Show API error details when available
    - _Requirements: 11.1, 11.2_
  - [x] 16.3 Set appropriate exit codes
    - Exit 0 on success
    - Exit non-zero on errors
    - _Requirements: 11.5_

- [x] 17. Final checkpoint - Full integration testing
  - Ensure all tests pass, ask the user if questions arise.
  - Verify output matches expected format
  - Test with various network conditions

## Notes

- Tasks marked with `*` are optional property-based tests that can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties using the `proptest` crate
- Unit tests validate specific examples and edge cases
