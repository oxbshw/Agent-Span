# AgentSpan justfile — common dev tasks.
# Install just: https://github.com/casey/just

default:
    @just --list

# Build all crates
build:
    cargo build --workspace

# Build release
release:
    cargo build --workspace --release

# Run all tests
test:
    cargo test --workspace

# Run tests for one crate (e.g. `just test-crate channels`)
test-crate crate:
    cargo test -p agentspan-{{crate}}

# Lint
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Format check
fmt:
    cargo fmt --all -- --check

# Format fix
fmt-fix:
    cargo fmt --all

# Start the API
serve:
    cargo run --bin agentspan -- serve

# Start the MCP server (stdio)
mcp:
    cargo run --bin agentspan-mcp

# Health check
doctor:
    cargo run --bin agentspan -- doctor

# Python SDK tests
test-python:
    cd sdk/python && pip install -e ".[dev]" && pytest -q

# Web dashboard
web-dev:
    cd web && npm run dev

web-build:
    cd web && npm run build

# Docker full stack
docker-up:
    docker compose up --build

docker-down:
    docker compose down

# CI: everything a PR must pass
ci: fmt clippy test
    @echo "All CI checks passed."
