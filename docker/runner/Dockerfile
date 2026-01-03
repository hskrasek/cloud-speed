# Build stage - compile for Linux target
FROM rust:1.85-bookworm AS builder

# Install git for build.rs (captures git hash for version string)
RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy source code (including .git for version hash)
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY .git ./.git

# Build optimized release binary
RUN cargo build --profile release-lto

# Runtime stage - minimal image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    cron \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from builder stage
COPY --from=builder /app/target/release-lto/cloud-speed /usr/local/bin/cloud-speed

# Copy runner script and cron config
COPY docker/runner/run-speedtest.sh /usr/local/bin/run-speedtest.sh
COPY docker/runner/crontab /etc/cron.d/speedtest-cron

# Set permissions
RUN chmod +x /usr/local/bin/cloud-speed && \
    chmod +x /usr/local/bin/run-speedtest.sh && \
    chmod 0644 /etc/cron.d/speedtest-cron && \
    crontab /etc/cron.d/speedtest-cron

# Create log directory
RUN mkdir -p /var/log/cloud-speed

# Start cron in foreground
CMD ["cron", "-f"]
