#!/usr/bin/env bash
set -euo pipefail

echo "Setting up AgentSpan development environment..."

# Rust
if ! command -v cargo &>/dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Build the workspace
echo "Building workspace..."
cargo build --workspace

# Python SDK
if command -v pip &>/dev/null; then
    echo "Installing Python SDK..."
    pip install -e sdk/python[dev]
fi

# Web dashboard
if command -v npm &>/dev/null; then
    echo "Installing web dependencies..."
    (cd web && npm install)
fi

# Verify
echo "Running doctor..."
cargo run --bin agentspan -- doctor

echo "Done. Run 'cargo run --bin agentspan -- serve' to start the API."
