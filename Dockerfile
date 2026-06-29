# syntax=docker/dockerfile:1

# Stage 1: Build the Rust workspace
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies.
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first to cache dependency compilation.
COPY Cargo.toml ./
COPY crates/agentspan-core/Cargo.toml crates/agentspan-core/
COPY crates/agentspan-probe/Cargo.toml crates/agentspan-probe/
COPY crates/agentspan-router/Cargo.toml crates/agentspan-router/
COPY crates/agentspan-cache/Cargo.toml crates/agentspan-cache/
COPY crates/agentspan-auth/Cargo.toml crates/agentspan-auth/
COPY crates/agentspan-channels/Cargo.toml crates/agentspan-channels/
COPY crates/agentspan-mcp/Cargo.toml crates/agentspan-mcp/
COPY crates/agentspan-api/Cargo.toml crates/agentspan-api/
COPY crates/agentspan-cli/Cargo.toml crates/agentspan-cli/
# sdk/rust and tests/integration are workspace members, so cargo needs their
# manifests to resolve the workspace even though the image only builds the API.
COPY sdk/rust/Cargo.toml sdk/rust/
COPY tests/integration/Cargo.toml tests/integration/

# Build dependencies against stub sources so they cache independently of code.
RUN for c in core probe router cache auth channels; do \
        mkdir -p crates/agentspan-$c/src && echo '' > crates/agentspan-$c/src/lib.rs; \
    done && \
    mkdir -p crates/agentspan-mcp/src && echo '' > crates/agentspan-mcp/src/lib.rs && echo 'fn main() {}' > crates/agentspan-mcp/src/main.rs && \
    for c in api cli; do \
        mkdir -p crates/agentspan-$c/src && echo 'fn main() {}' > crates/agentspan-$c/src/main.rs; \
    done && \
    mkdir -p sdk/rust/src && echo '' > sdk/rust/src/lib.rs && \
    mkdir -p tests/integration/src && echo '' > tests/integration/src/lib.rs && \
    cargo build --release --bin agentspan-api && \
    rm -rf crates/*/src

# Copy real source and build the API binary.
COPY crates ./crates
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
