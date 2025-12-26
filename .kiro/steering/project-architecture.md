---
inclusion: always
---

# Project Architecture

## Overview

`cloud-speed` is a CLI tool for measuring network speed and consistency using Cloudflare's speed test infrastructure. The architecture is organized around three main concerns: API client, measurements, and statistics.

## Module Structure

### `src/cloudflare/`
Contains all Cloudflare API interaction code:
- `client.rs` - HTTP client wrapper for making requests
- `requests/` - Request types and implementations
  - `download.rs` - Download test requests
  - `upload.rs` - Upload test requests
  - `locations.rs` - Server location queries
  - `meta.rs` - Metadata about the connection
- `tests/` - Test implementations that use the client

### `src/measurements.rs`
Network measurement calculations:
- Latency calculation (median of measurements)
- Jitter calculation (variance between consecutive measurements)
- Supports both `Duration` and `f64` types for flexibility

### `src/stats.rs`
Statistical functions for analyzing test results:
- `median()` - Find the middle value in a dataset
- `mean()` - Calculate average
- `quartile()` - Calculate percentile values (e.g., 90th percentile)

### `src/main.rs`
CLI entry point and orchestration:
- Argument parsing with `clap`
- Test execution coordination
- Output formatting (human-readable and JSON)

## Design Patterns

### Request/Response Pattern
All API interactions follow a consistent pattern:
1. Define a request type implementing the `Request` trait
2. Specify the HTTP method, endpoint, headers, and body
3. Define the expected response type
4. Use `client.send(request)` to execute

### Measurement Collection
Tests are run multiple times to gather statistical data:
1. Create a vector of test futures
2. Execute them concurrently with `join_all`
3. Collect timing measurements
4. Calculate statistics (median, quartile, etc.)

### Separation of Concerns
- **Client layer**: Handles HTTP communication
- **Request layer**: Defines API contracts
- **Test layer**: Implements specific test types
- **Measurement layer**: Calculates network metrics
- **Stats layer**: Provides statistical analysis
- **Main**: Orchestrates everything and handles I/O

## Key Dependencies

- `tokio` - Async runtime for concurrent operations
- `reqwest` - HTTP client for API requests
- `clap` - Command-line argument parsing
- `serde` - Serialization/deserialization
- `colored` - Terminal output formatting
- `rustls` - TLS implementation (no OpenSSL dependency)

## Build Process

The `build.rs` script runs at compile time to:
- Embed git commit hash into the binary
- Generate version information for `--version` output

## Output Modes

1. **Human-readable** (default): Colored, formatted output for terminal use
2. **JSON**: Machine-readable output with `--json` flag
3. **Pretty JSON**: Formatted JSON with `--json --pretty` flags

## Future Considerations

When adding new features:
- Keep the request/response pattern consistent
- Add new test types in `src/cloudflare/tests/`
- Add new measurements in `src/measurements.rs`
- Add new statistics in `src/stats.rs`
- Update CLI arguments in `main.rs` if needed
