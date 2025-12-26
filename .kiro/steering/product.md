# Product Overview

cloud-speed is a CLI tool for measuring network speed and consistency using Cloudflare's speed test infrastructure (speed.cloudflare.com).

## Purpose
- Measure download and upload speeds
- Calculate network latency and jitter
- Provide both human-readable and JSON output formats

## Key Features
- Latency measurement via TCP handshake timing
- Jitter calculation from consecutive latency measurements
- Download speed tests with multiple file sizes (100KB to 100MB)
- Upload speed tests
- Server location and network metadata display
- JSON output for scripting/automation (`--json`, `--pretty`)

## Output Metrics
- Server location (Cloudflare edge)
- Network info (ISP, ASN)
- Client IP and country
- Latency (median of measurements, in ms)
- Jitter (mean absolute difference between consecutive measurements, in ms)
- Download/upload speeds (Mbps)
