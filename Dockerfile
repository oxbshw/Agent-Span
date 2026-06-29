# syntax=docker/dockerfile:1

# Stage 1: Build the Rust workspace
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies.
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the whole Cargo workspace (all members must be present to resolve it) and
# build the API binary. Cargo's own incremental cache + the registry cache make
# rebuilds fast; BuildKit caches the layer when these inputs are unchanged.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY sdk/rust ./sdk/rust
COPY tests/integration ./tests/integration

RUN cargo build --release --bin agentspan-api

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/agentspan-api /usr/local/bin/agentspan-api

ENV AGENTSPAN_SERVER__HOST=0.0.0.0 \
    AGENTSPAN_SERVER__PORT=8080 \
    RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://localhost:8080/health || exit 1

CMD ["agentspan-api"]
