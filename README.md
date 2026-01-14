> [!IMPORTANT]
>
> ## 🇵🇸 Support Palestine 🇵🇸
>
> In light of recent events in Gaza, I encourage everyone to educate themselves on the ongoing issues in Palestine and consider supporting the people there. Here are some resources and donation links:
>
> - [Decolonize Palestine](https://decolonizepalestine.com/) - An informative resource to better understand the situation in Palestine. Please take the time to read it.
> - [One Ummah - Gaza Emergency Appeal](https://donate.oneummah.org.uk/gazaemergencyappeal48427259) - A platform to provide direct donations to help the people in Gaza.
> - [Islamic Relief US - Palestine Appeal](https://islamic-relief.org/appeals/palestine-emergency-appeal/) - Another trusted platform to provide support for those affected in Palestine.
>
> Thank you for taking a moment to bring awareness and make a difference. 🇵🇸❤️

# cloud-speed

[![Release](https://github.com/hskrasek/cloud-speed/actions/workflows/release.yml/badge.svg)](https://github.com/hskrasek/cloud-speed/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/cloud-speed.svg)](https://crates.io/crates/cloud-speed)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL%203.0-blue.svg)](LICENSE.md)

A fast, feature-rich CLI for measuring network speed and quality using Cloudflare's speed test infrastructure.

![cloud-speed demo](demo.gif)

## Features

- **Accurate Speed Testing** – Download and upload measurements using Cloudflare's global network
- **Quality Scores** - AIM-based ratings for streaming, gaming, and video conferencing
- **Beautiful TUI** - Real-time terminal visualization with live graphs
- **JSON Output** - Machine-readable output for scripting and automation
- **Docker Ready** - Run as a container for scheduled monitoring
- **Fast & Lightweight** – Minimal binary size, no runtime dependencies

## Installation

### Pre-built Binaries

Download the latest release for your platform from [Releases](https://github.com/hskrasek/cloud-speed/releases):

| Platform              | Download                      |
|-----------------------|-------------------------------|
| Linux (x64)           | `cloud-speed-linux-x64`       |
| Linux (ARM64)         | `cloud-speed-linux-arm64`     |
| macOS (Intel)         | `cloud-speed-macos-x64`       |
| macOS (Apple Silicon) | `cloud-speed-macos-arm64`     |
| Windows (x64)         | `cloud-speed-windows-x64.exe` |

### Cargo (Rust)

```bash
cargo install cloud-speed
```

### Docker

```bash
docker run --rm ghcr.io/hskrasek/cloud-speed
```

### From Source

```bash
git clone https://github.com/hskrasek/cloud-speed
cd cloud-speed
cargo build --release
```

The binary will be at `target/release/cloud-speed`.

## Usage

### Interactive Mode (TUI)

```bash
cloud-speed
```

Launches an interactive terminal UI with real-time speed graphs and progress.

### JSON Output

```bash
# Compact JSON (for scripting)
cloud-speed --json

# Pretty-printed JSON
cloud-speed --json --pretty
```

### Verbose Logging

```bash
cloud-speed -v      # info level
cloud-speed -vv     # debug level
cloud-speed -vvv    # trace level
```

## Output

### JSON Output Example

```json
{
  "timestamp": "2026-01-13T12:00:00Z",
  "server": {
    "city": "Chicago",
    "iata": "ORD"
  },
  "connection": {
    "ip": "203.0.113.1",
    "isp": "Example ISP",
    "country": "US"
  },
  "download": {
    "speed_mbps": 450.5,
    "latency_ms": 28.3
  },
  "upload": {
    "speed_mbps": 42.3,
    "latency_ms": 35.1
  },
  "latency": {
    "idle_ms": 12.5,
    "jitter_ms": 1.2
  },
  "scores": {
    "streaming": "Great",
    "gaming": "Good",
    "video_conferencing": "Great"
  }
}
```

## Docker

### Quick Run

```bash
docker run --rm ghcr.io/hskrasek/cloud-speed
```

### Build Locally

```bash
docker build -t cloud-speed .
docker run --rm cloud-speed
```

### Available Tags

- `latest` - Most recent release
- `x.y.z` - Specific version (e.g., `0.8.0`)
- `x.y` - Minor version (e.g., `0.8`)
- `x` - Major version (e.g., `0`)

## Quality Scores

cloud-speed calculates AIM (Aggregated Internet Measurement) quality scores based on your connection's performance:

| Score       | Meaning                          |
|-------------|----------------------------------|
| **Great**   | Excellent for this use case      |
| **Good**    | Should work well with no issues  |
| **Average** | May experience occasional issues |
| **Poor**    | Likely to have problems          |

### Categories

- **Streaming** – Video streaming services (Netflix, YouTube, Twitch, etc.)
- **Gaming** – Online gaming latency and stability requirements
- **Video Conferencing** – Video calls (Zoom, Teams, Meet, etc.)

Scores are calculated based on download/upload speeds, latency, jitter, and packet loss.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the AGPL-3.0 License - see the [LICENSE.md](LICENSE.md) file for details.
