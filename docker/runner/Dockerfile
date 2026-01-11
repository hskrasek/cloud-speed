# Build stage - compile for Linux target
FROM rust:1-alpine AS builder

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apk \
    apk add --no-cache \
    git \
    musl-dev \
    openssl-dev \
    openssl-libs-static

WORKDIR /app

# Copy source code (including .git for version hash)
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY .git ./.git

# Build optimized release binary
RUN cargo build --profile release-lto

# Runtime stage - minimal image (no Rust toolchain needed)
FROM alpine:latest

# Install runtime dependencies for TLS
RUN apk add --no-cache ca-certificates

# Copy the built binary from builder stage
COPY --from=builder /app/target/release-lto/cloud-speed /usr/local/bin/cloud-speed

# Set permissions
RUN chmod +x /usr/local/bin/cloud-speed

# Run the speed test
CMD ["cloud-speed", "--json"]
