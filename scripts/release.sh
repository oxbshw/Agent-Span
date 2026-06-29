#!/usr/bin/env bash
set -euo pipefail

version="${1:?usage: release.sh <version>}"

echo "Cutting AgentSpan v${version}..."

# Update version in workspace Cargo.toml
sed -i.bak "s/^version = .*/version = \"${version}\"/" Cargo.toml
rm Cargo.toml.bak

# Update CHANGELOG date
today=$(date +%Y-%m-%d)
sed -i.bak "s/## \[Unreleased\]/## [${version}] — ${today}/" CHANGELOG.md
rm CHANGELOG.md.bak

# Verify
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

echo "Ready to commit and tag:"
echo "  git add -A && git commit -m 'release: v${version}'"
echo "  git tag v${version}"
echo "  git push && git push --tags"
