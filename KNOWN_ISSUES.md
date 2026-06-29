# Known Issues & Limitations

An honest list of current rough edges. If you hit one of these, you're not doing
anything wrong — they're tracked here on purpose.

## Build / toolchain

- **Slow incremental builds on `x86_64-pc-windows-gnu`.** A clean-ish incremental
  `cargo build` can take ~10 minutes on this toolchain. `cargo check` and
  `cargo clippy`/`cargo test` (once deps are built) are much faster. Prefer
  `CARGO_INCREMENTAL=0` when disk is tight.
- **`axum`'s `ws` feature does not build on windows-gnu** (it pulls
  `tungstenite -> rand 0.9 -> getrandom 0.3`, which fails to link with
  "dlltool.exe not found"). WebSocket is therefore gated behind the
  `websocket` cargo feature (off by default). SSE (`/api/v1/events/stream`)
  is the default real-time transport and works on all targets. Enable
  WebSocket with `cargo build -p agentspan-api --features websocket` on
  Linux/macOS or in the Docker release image.

## Runtime / features

- **No live browser-cookie decryption.** `config --from-browser` guides you to
  the Cookie-Editor export flow rather than reading the browser's cookie DB
  directly. DPAPI/AES extraction is gated behind an optional feature that isn't
  built by default.
- **Transcription does not chunk files larger than ~25 MB.** Whisper-style
  upload limits apply; long audio should be split upstream for now.
- **No plugin hot-reload.** Channels are compiled-in Rust types, not dynamically
  loaded objects, so the server cannot reload channel code without a restart.
  The `plugin` command manages the registry/config, not live code.
- **Credentialed channels need their environment set.** Discord, Telegram,
  Spotify, Twitch, Podcast Index, and Google Scholar require API keys / tokens;
  without them those channels probe as `warn`/`missing` and degrade gracefully.
- **CLI shell-out backends require the upstream tool installed.** OpenCLI,
  yt-dlp, twitter-cli, etc. are probed; when absent, the router falls back to
  the next backend (or a raw fallback) rather than failing hard.

## Architecture notes (intentional tradeoffs)

- **WebSocket is feature-gated, SSE is the default.** The spec originally
  promised `/ws/v1/stream` unconditionally; axum's `ws` feature doesn't
  build on windows-gnu, so WebSocket is now behind the `websocket` cargo
  feature. SSE at `/api/v1/events/stream` works everywhere and covers the
  one-way push use case. Enable WebSocket on Linux/Docker for bidirectional
  streaming.
- **MCP HTTP transport is feature-gated.** The default MCP binary serves
  stdio only. Build with `cargo build -p agentspan-mcp --features http` and
  run `agentspan-mcp --http [addr]` for the HTTP/SSE transport.
- **Benchmark latency on Windows is timer-bound.** The OS timer granularity
  (~15.6 ms) inflates reported p50 for sub-millisecond cache hits. Treat the
  *relative* hit-vs-miss numbers as meaningful, not the absolute sub-ms figures.

## Tests

- One channel test is `#[ignore]`d because it depends on live network access.
  Run it explicitly with `cargo test -- --ignored` when you have connectivity.
