FROM debian:bookworm-slim

# Install runtime dependencies for TLS
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy platform-specific pre-built binary
# TARGETARCH is automatically set by BuildKit to "amd64" or "arm64"
ARG TARGETARCH
COPY linux/${TARGETARCH}/cloud-speed /usr/local/bin/cloud-speed

RUN chmod +x /usr/local/bin/cloud-speed

CMD ["cloud-speed", "--json"]
