# Builder stage
FROM rust:bullseye AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Create dummy project to cache dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
COPY Cargo.toml .
COPY Cargo.lock* .
RUN cargo build --release

# Remove the dummy files
RUN rm -rf src

# Copy the actual source code
COPY . .
# Touch main.rs to force recompile
RUN touch src/main.rs
# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

WORKDIR /app

# Install minimal runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl libssl1.1 net-tools procps && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/livesync-proxy /app/livesync-proxy

# Copy static files
COPY --from=builder /app/static /app/static

# Create a health check script
RUN echo '#!/bin/sh\ncurl -f http://localhost:3000/health || exit 1' > /app/health-check.sh && \
    chmod +x /app/health-check.sh

# Create a non-root user
RUN useradd -m appuser
USER appuser

# Set environment variables
ENV RUST_LOG=info

# Expose the port
EXPOSE 3000

# Set health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 CMD ["/app/health-check.sh"]

# Run the application
CMD ["/app/livesync-proxy"]
