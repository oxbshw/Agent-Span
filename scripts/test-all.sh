#!/usr/bin/env bash
set -euo pipefail

echo "Running all AgentSpan tests..."

echo "==> Rust workspace tests"
cargo test --workspace

echo "==> Clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> Format check"
cargo fmt --all -- --check

echo "==> Python SDK tests"
if command -v pytest &>/dev/null; then
    (cd sdk/python && pytest -q)
else
    echo "  (skipped: pytest not installed)"
fi

echo "All tests passed."
