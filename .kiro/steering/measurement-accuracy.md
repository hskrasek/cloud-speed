---
inclusion: always
---

# Measurement Accuracy Guidelines

## Core Principle

This tool's primary purpose is to provide accurate network measurements. Every design decision should prioritize measurement accuracy and precision.

## Timing Precision

### Use Appropriate Types
- Use `f64` for millisecond measurements to maintain sub-millisecond precision
- Use `Duration` from `std::time` for raw timing data
- Convert to `f64` using `as_secs_f64() * 1000.0` for millisecond precision
- Avoid integer milliseconds (`as_millis()`) when precision matters

### Measurement Points
When measuring network operations, capture:
1. **DNS lookup time** - Time to resolve hostname
2. **TCP handshake time** - Time to establish connection
3. **SSL/TLS handshake time** - Time for secure connection setup
4. **TTFB (Time To First Byte)** - Time until first response byte
5. **Total duration** - Complete request/response cycle

### Minimize Measurement Overhead
- Start timers immediately before the operation
- Stop timers immediately after the operation
- Avoid logging or computation between timer start/stop
- Use `Instant::now()` for high-precision timing

## Statistical Accuracy

### Sample Sizes
- **Latency**: Minimum 10 measurements for reliable median
- **Download tests**: 3-10 measurements per file size (larger files need fewer samples)
- **Upload tests**: 3-8 measurements per size
- More samples = better accuracy but longer test duration

### Statistical Methods
- **Median** over mean for central tendency (resistant to outliers)
- **90th percentile** for "typical best" performance
- **Jitter** as mean absolute difference between consecutive measurements
- Avoid using minimum values (too susceptible to noise)

### Handling Outliers
- Don't automatically discard outliers - they represent real network conditions
- Use robust statistics (median, percentiles) instead of filtering
- If filtering is necessary, document the criteria clearly

## Network Considerations

### Test Progression
Run tests from smallest to largest file sizes:
1. Small files (100KB) - Establish baseline, minimal buffering effects
2. Medium files (1MB, 10MB) - Typical usage patterns
3. Large files (25MB, 100MB) - Sustained throughput measurement

### Concurrent Requests
- Use `join_all` for parallel test execution when appropriate
- Be careful not to saturate the connection during latency tests
- Consider rate limiting to avoid triggering server-side throttling

### Connection Reuse
- The `reqwest` client reuses connections by default
- This is good for realistic measurements (matches browser behavior)
- First request may be slower due to connection establishment

## Avoiding Common Pitfalls

### Don't Measure What You Don't Need
- Separate download time from TTFB: `end_duration - ttfb_duration`
- Don't include parsing time in network measurements
- Exclude logging overhead from timing measurements

### Precision Loss
- Don't convert to integers prematurely
- Be careful with division order: `(bytes * 8.0) / (duration / 1000.0)` not `(bytes * 8) / duration`
- Use `total_cmp` for sorting floats (handles NaN correctly)

### Async Timing
- Use `tokio::time::Instant` for async contexts
- Don't use `std::time::Instant` across await points
- Measure wall-clock time, not CPU time

## Validation

When implementing new measurements:
1. Compare results with Cloudflare's web interface
2. Test on various network conditions (fast, slow, unstable)
3. Verify units are correct (bits vs bytes, seconds vs milliseconds)
4. Check that results are reasonable (e.g., speed can't exceed physical limits)

## Reporting Results

### Precision in Output
- Display speeds to 2 decimal places: `{:.2} Mbps`
- Display latency to 2 decimal places: `{:.2} ms`
- Use consistent units throughout the output

### Speed Calculation
```rust
// Correct: bytes to megabits per second
fn measure_speed(bytes: f64, duration_ms: f64) -> f64 {
    (bytes * 8.0) / (duration_ms / 1000.0) / 1e6
}
```

### Units
- **Speed**: Megabits per second (Mbps)
- **Latency**: Milliseconds (ms)
- **Jitter**: Milliseconds (ms)
- **File sizes**: Bytes (B), Kilobytes (KB), Megabytes (MB)
