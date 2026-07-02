// Marketing copy for the landing sections. Channels live in ./realData.ts.

export const site = {
  name: "AgentSpan",
  tagline: "Web Access Gateway for AI Agents",
  description: "52 channels. 91 MCP tools. Self-healing backends. Built in Rust.",
  githubUrl: "https://github.com/oxbshw/Agent-Span",
  cratesUrl: "https://crates.io/crates/agentspan",
  docsUrl: "https://github.com/oxbshw/Agent-Span#readme",
  version: "0.5.0",
  year: 2026,
};

export const stats = [
  { label: "Channels", value: "52" },
  { label: "MCP Tools", value: "91" },
  { label: "SDKs", value: "9" },
  { label: "Core", value: "Rust" },
];

export const heroLabels = {
  left: ["ROUTE THE WEB", "SELF-HEAL"],
  right: ["91 MCP TOOLS", "RUST-POWERED"],
};

export const features = [
  { title: "52 Channels", description: "Search, social, media, dev tools — from Brave to GitHub, one gateway.", icon: "Globe" },
  { title: "91 MCP Tools", description: "A full Model Context Protocol server your agent discovers at runtime.", icon: "Cpu" },
  { title: "Self-Healing", description: "Auto-failover, repair, and continuous health monitoring on every channel.", icon: "HeartPulse" },
  { title: "3-Tier Cache", description: "L1 memory → L2 disk → L3 Redis with smart, adaptive TTL.", icon: "Database" },
  { title: "Circuit Breaker", description: "Exponential backoff and retry with jitter, built into every request.", icon: "Shield" },
  { title: "Agent Memory", description: "A key-value scratchpad for persistent context across sessions.", icon: "Brain" },
];

export const channelCategories = [
  { no: "01", name: "Search", count: 7, list: ["brave", "bing", "google", "duckduckgo", "exa", "scholar", "gnews"] },
  { no: "02", name: "Social", count: 13, list: ["reddit", "hackernews", "twitter", "linkedin", "discord", "telegram", "devto"] },
  { no: "03", name: "Media", count: 7, list: ["youtube", "tiktok", "spotify", "twitch", "bilibili", "podcasts", "xiaoyuzhou"] },
  { no: "04", name: "Developer", count: 8, list: ["github", "gitlab", "npm", "crates", "pypi", "dockerhub", "huggingface"] },
];

export const architecture = [
  { name: "agentspan-core", desc: "Shared types, errors, config" },
  { name: "agentspan-router", desc: "Routing, retry, circuit breaker" },
  { name: "agentspan-channels", desc: "52 channel implementations" },
  { name: "agentspan-cache", desc: "3-tier cache + optimizer" },
  { name: "agentspan-auth", desc: "API keys, tenants, RBAC" },
  { name: "agentspan-api", desc: "Axum REST API + SSE" },
  { name: "agentspan-mcp", desc: "MCP server (91 tools)" },
  { name: "agentspan-cli", desc: "Doctor, serve, benchmark" },
];

export const showcaseStats = [
  { value: "< 10ms", label: "Cache-hit latency" },
  { value: "10K+", label: "Requests / second" },
  { value: "99.2%", label: "Uptime success" },
  { value: "9", label: "Rust crates" },
];

export const installSteps = [
  { c: "# Install", p: "cargo", rest: " install agentspan" },
  { c: "# Start the gateway", p: "agentspan", rest: " serve" },
  { c: "# Query any channel", p: "curl", rest: " localhost:8080/api/v1/channels/google/search?q=rust" },
];
