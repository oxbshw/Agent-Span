#!/usr/bin/env bash
# Run AgentSpan workspace tests.
# On Windows with the GNU toolchain, ensure a full MinGW-w64 installation is on PATH.

set -euo pipefail

# Adjust if your MinGW installation lives elsewhere.
if [[ -d /f/mingw64/bin ]]; then
    export PATH="/f/mingw64/bin:$PATH"
fi

cd "$(dirname "$0")/.."
cargo test "$@"
