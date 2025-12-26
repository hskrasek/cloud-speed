# Tech Stack

## Language & Runtime
- Rust 1.85.0+ (2021 edition)
- Tokio async runtime

## Key Dependencies
- `reqwest` - HTTP client for API requests
- `tokio` - Async runtime with multi-threaded executor
- `clap` - CLI argument parsing (derive macros)
- `serde` / `serde_json` - Serialization
- `rustls` - TLS (no OpenSSL dependency)
- `hickory-resolver` - DNS resolution
- `colored` - Terminal output formatting
- `ttfb` - Time-to-first-byte measurements
- `proptest` - Property-based testing (dev dependency)

## Build System
- Cargo (standard Rust build tool)
- `build.rs` - Embeds git commit hash at compile time

## Common Commands

```bash
# Build
cargo build
cargo build --release
cargo build --profile release-lto  # Optimized production build

# Run
cargo run
cargo run -- --json --pretty

# Test
cargo test
cargo test -- --nocapture  # With output
cargo test test_name       # Specific test

# Lint & Format
cargo clippy
cargo fmt
cargo fmt --check
```

## Build Profiles
- `dev` - Fast compilation, no optimization
- `release` - Standard release with debug symbols
- `release-lto` - Full optimization with LTO, symbol stripping, single codegen unit

## Code Style
- `rustfmt.toml` configured: max_width=79, reorder imports/modules
- Use `clippy` for linting
