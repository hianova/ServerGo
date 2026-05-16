# Build stage
FROM rust:1.81-slim AS builder

WORKDIR /usr/src/app

# Install dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire workspace
COPY . .

# Build with tiered-storage feature by default
RUN cargo build --release --features tiered-storage

# Runtime stage
FROM debian:bookworm-slim

# Install iproute2 for 'tc' command used in chaos testing
RUN apt-get update && apt-get install -y \
    iproute2 \
    procps \
    python3 \
    python3-redis \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

# Copy the binary
COPY --from=builder /usr/src/app/target/release/ServerGo .

# Copy tools for benchmarking and chaos
COPY --from=builder /usr/src/app/tools ./tools

# Expose RESP port
EXPOSE 6379

ENTRYPOINT ["./ServerGo"]
