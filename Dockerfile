# Fusebox Docker Image
FROM rust:1.78-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release
RUN cargo build --release --bin fusebox-proxy

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/fusebox-proxy /usr/local/bin/fusebox

# Copy pricing data
COPY pricing /app/pricing

# Copy default config
COPY fusebox.yaml.example /app/fusebox.yaml

# Create data directory
RUN mkdir -p /data/.fusebox

# Expose ports
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/fusebox", "doctor"] || exit 1

# Run as non-root user
RUN useradd -m -u 1000 fusebox && chown -R fusebox:fusebox /app /data
USER fusebox

ENV FUSEBOX_CONFIG=/app/fusebox.yaml
ENV FUSEBOX_DATA_DIR=/data/.fusebox

CMD ["/usr/local/bin/fusebox", "start"]
