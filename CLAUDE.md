# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cloud-speed is a Rust CLI tool for measuring network speed and consistency using Cloudflare's speed.cloudflare.com infrastructure. It provides real-time TUI visualization, JSON output for scripting, and calculates AIM (Aggregated Internet Measurement) quality scores for streaming, gaming, and video conferencing.

## Build Commands

```bash
# Development build
cargo build

# Release build
cargo build --release

# Optimized release with LTO (used for distribution)
cargo build --profile release-lto

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Run with verbose logging
cargo run -- -v      # info level
cargo run -- -vv     # debug level
cargo run -- -vvv    # trace level
```

## Architecture

### Core Modules

- **`main.rs`** - CLI entry point, orchestrates test execution and output modes (TUI/JSON/Silent)
- **`cloudflare/`** - HTTP client and API interactions with speed.cloudflare.com
  - `client.rs` - Request/response handling via reqwest
  - `requests/` - API request types (metadata, locations)
  - `tests/` - Speed test implementations
    - `engine.rs` - Main test orchestration, configurable data block sizes, progress callbacks
    - `download.rs` / `upload.rs` - Bandwidth test implementations
    - `packet_loss.rs` - Optional TURN-based packet loss measurement
- **`tui/`** - Terminal UI with ratatui/crossterm
  - `controller.rs` - Lifecycle management, alternate screen handling
  - `state.rs` - Shared state between test engine and renderer
  - `renderer.rs` - Frame rendering
  - `progress.rs` - Progress events and callbacks
  - `display_mode.rs` - TUI/JSON/Silent mode detection
- **`measurements.rs`** - Bandwidth/latency calculations, loaded latency collection
- **`scoring.rs`** - AIM quality score calculations (Great/Good/Average/Poor)
- **`stats.rs`** - Statistical functions (median, percentile)
- **`errors.rs`** - Error types with exit codes and user-friendly messages
- **`retry.rs`** - Exponential backoff retry logic

### Key Patterns

- **Blocking I/O in Async Context**: Download/upload tests use `tokio::task::spawn_blocking` for all TLS/TCP I/O (`rustls_connector` is synchronous). Data passed in must be `'static` — use owned types (`Url`, `String`, `Arc<Vec<u8>>`) not references. Error boundaries require `.map_err(|e| e as Box<dyn Error>)` to convert `Send + Sync` errors.
- **HTTP Status Checking**: Raw HTTP responses are validated with `extract_http_status()` in `mod.rs`. Cloudflare returns 429/403 for rate-limited requests — these propagate as errors through retry logic.
- **Progress Callbacks**: TestEngine accepts an optional `ProgressCallback` for real-time updates. The TUI uses this to update state via Arc<Mutex<TuiState>>.
- **Display Modes**: `DisplayMode::detect(json_flag, is_tty)` determines output format. JSON mode suppresses all TUI output until final results.
- **90th Percentile**: Final bandwidth calculations use 90th percentile of measurements (configurable via `bandwidth_percentile`).
- **Loaded Latency**: Collected during bandwidth tests with FIFO queue (max 20 samples per direction), filtered by minimum request duration.

### Build Script

`build.rs` captures the git commit hash at compile time, exposed via `CLOUDSPEED_BUILD_GIT_HASH` environment variable for version display.

## Docker

Docker setup in `docker/` for continuous monitoring:
- `docker/runner/Dockerfile` - Uses pre-built binaries on `ubuntu:24.04` (must match GitHub Actions runner glibc)
- `docker/opensearch/` - OpenSearch integration for storing results

Build Docker image:
```bash
docker build -f docker/runner/Dockerfile -t cloud-speed .
```

## Output Formats

```bash
# Interactive TUI (default in terminal)
cloud-speed

# JSON output (for scripting/piping)
cloud-speed --json

# Pretty-printed JSON
cloud-speed --json --pretty
```

## Test Configuration

### Manual Testing Caveat

Cloudflare rate-limits repeated speed test requests. Running `cloud-speed` multiple times in quick succession will trigger 429/403 responses for larger test sizes (10MB+). Wait ~45 minutes between full test runs when debugging.

Default test sizes in `TestConfig`:
- Download: 100KB(10), 1MB(8), 10MB(6), 25MB(4), 100MB(3) measurements
- Upload: 100KB(8), 1MB(6), 10MB(4), 25MB(4), 50MB(3) measurements
- Tests terminate early when measurements reach `bandwidth_finish_duration_ms` (1000ms default)
