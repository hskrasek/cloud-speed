# Project Structure

```
cloud-speed/
├── src/
│   ├── main.rs              # CLI entry point, argument parsing, orchestration
│   ├── stats.rs             # Statistical functions (median, mean, percentile)
│   ├── measurements.rs      # Network measurement calculations (latency, jitter, bandwidth)
│   └── cloudflare/
│       ├── mod.rs           # Module exports
│       ├── client.rs        # HTTP client wrapper for Cloudflare API
│       ├── requests/        # Request type definitions
│       │   ├── mod.rs       # Request trait and common types
│       │   ├── download.rs  # Download test request
│       │   ├── upload.rs    # Upload test request
│       │   ├── locations.rs # Server location queries
│       │   └── meta.rs      # Connection metadata request
│       └── tests/           # Test implementations (network tests, not unit tests)
│           ├── mod.rs       # Test trait and TestResults struct
│           └── download.rs  # Download test with detailed timing
├── build.rs                 # Build script (git hash embedding)
├── Cargo.toml               # Dependencies and build config
└── rustfmt.toml             # Code formatting rules
```

## Architecture Patterns

### Request/Response Pattern
All API interactions use a `Request` trait:
- Define request type with `endpoint()`, `headers()`, `body()`
- Specify response type for deserialization
- Execute via `client.send(request)`

### Test Trait
Network tests implement the `Test` trait:
- `endpoint()` - API endpoint
- `run(bytes)` - Execute test, return `TestResults` with timing breakdown

### Separation of Concerns
- `client.rs` - HTTP communication
- `requests/` - API contracts
- `tests/` - Network test implementations
- `measurements.rs` - Metric calculations
- `stats.rs` - Statistical analysis
- `main.rs` - CLI and orchestration

## Testing Organization
- Unit tests: `#[cfg(test)] mod tests` in same file as implementation
- Integration tests: `src/cloudflare/tests/` directory
- Property-based tests: Use `proptest` for statistical function validation
