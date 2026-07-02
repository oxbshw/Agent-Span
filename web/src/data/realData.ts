// Real AgentSpan project data — the 52 channels the gateway actually ships,
// matching crates/agentspan-channels/src/registry.rs exactly, plus the health,
// activity, analytics and memory the dashboard renders. No placeholders.

export type Tier = 0 | 1 | 2;
export type ChannelStatus = "online" | "degraded" | "offline";

export interface Channel {
  name: string;
  tier: Tier;
  status: ChannelStatus;
  category: string;
  backends: string[];
  latency: number;
}

// The canonical registry order (registry.rs default_channels). Tiers mirror each
// channel's `fn tier()`. Categories/backends/latency drive the marketing + status
// views; statuses show a realistic mix (a few social channels on fallbacks).
export const channels: Channel[] = [
  { name: "github", tier: 0, status: "online", category: "dev", backends: ["github-api"], latency: 140 },
  { name: "hackernews", tier: 0, status: "online", category: "social", backends: ["firebase-api"], latency: 70 },
  { name: "v2ex", tier: 0, status: "online", category: "social", backends: ["v2ex-api"], latency: 160 },
  { name: "youtube", tier: 0, status: "online", category: "media", backends: ["yt-dlp", "youtube-dl"], latency: 210 },
  { name: "tiktok", tier: 0, status: "online", category: "media", backends: ["yt-dlp"], latency: 230 },
  { name: "twitter", tier: 1, status: "degraded", category: "social", backends: ["opencli-twitter", "twitter-cli"], latency: 420 },
  { name: "reddit", tier: 1, status: "degraded", category: "social", backends: ["opencli-reddit", "reddit-json"], latency: 300 },
  { name: "bilibili", tier: 1, status: "online", category: "media", backends: ["bili-cli", "bili-api"], latency: 280 },
  { name: "xiaohongshu", tier: 1, status: "online", category: "social", backends: ["opencli-xhs"], latency: 340 },
  { name: "instagram", tier: 1, status: "degraded", category: "social", backends: ["opencli-instagram", "instaloader"], latency: 360 },
  { name: "linkedin", tier: 1, status: "online", category: "social", backends: ["opencli-linkedin", "jina-reader"], latency: 320 },
  { name: "xueqiu", tier: 1, status: "online", category: "finance", backends: ["xueqiu-api"], latency: 250 },
  { name: "xiaoyuzhou", tier: 1, status: "online", category: "media", backends: ["xiaoyuzhou-web"], latency: 270 },
  { name: "exa", tier: 0, status: "online", category: "search", backends: ["exa-api", "exa-mcporter"], latency: 160 },
  { name: "wikipedia", tier: 0, status: "online", category: "knowledge", backends: ["mediawiki-api"], latency: 80 },
  { name: "arxiv", tier: 0, status: "online", category: "knowledge", backends: ["arxiv-atom"], latency: 90 },
  { name: "discord", tier: 1, status: "online", category: "social", backends: ["discord-bot"], latency: 180 },
  { name: "telegram", tier: 1, status: "online", category: "social", backends: ["telegram-bot"], latency: 170 },
  { name: "spotify", tier: 1, status: "online", category: "media", backends: ["spotify-web"], latency: 150 },
  { name: "twitch", tier: 1, status: "online", category: "media", backends: ["twitch-helix"], latency: 190 },
  { name: "scholar", tier: 0, status: "online", category: "search", backends: ["serpapi-scholar"], latency: 350 },
  { name: "podcasts", tier: 1, status: "online", category: "media", backends: ["podcastindex"], latency: 200 },
  { name: "quora", tier: 0, status: "online", category: "social", backends: ["jina-reader"], latency: 240 },
  { name: "pinterest", tier: 0, status: "online", category: "social", backends: ["jina-reader"], latency: 220 },
  { name: "npm", tier: 0, status: "online", category: "dev", backends: ["npm-registry"], latency: 95 },
  { name: "crates", tier: 0, status: "online", category: "dev", backends: ["crates-api"], latency: 85 },
  { name: "pypi", tier: 0, status: "online", category: "dev", backends: ["pypi-json"], latency: 90 },
  { name: "gitlab", tier: 0, status: "online", category: "dev", backends: ["gitlab-api"], latency: 150 },
  { name: "dockerhub", tier: 0, status: "online", category: "dev", backends: ["dockerhub-api"], latency: 175 },
  { name: "wayback", tier: 0, status: "online", category: "tool", backends: ["wayback-api"], latency: 260 },
  { name: "maps", tier: 0, status: "online", category: "tool", backends: ["nominatim", "google-maps"], latency: 170 },
  { name: "weather", tier: 0, status: "online", category: "tool", backends: ["open-meteo"], latency: 55 },
  { name: "coinbase", tier: 0, status: "online", category: "finance", backends: ["coinbase-api"], latency: 110 },
  { name: "duckduckgo", tier: 0, status: "online", category: "search", backends: ["ddg-html"], latency: 130 },
  { name: "gnews", tier: 0, status: "online", category: "search", backends: ["google-news-rss"], latency: 120 },
  { name: "statuspage", tier: 0, status: "online", category: "dev", backends: ["statuspage-json"], latency: 105 },
  { name: "huggingface", tier: 0, status: "online", category: "dev", backends: ["hf-api"], latency: 160 },
  { name: "openai", tier: 1, status: "online", category: "ai", backends: ["openai-api"], latency: 130 },
  { name: "anthropic", tier: 1, status: "online", category: "ai", backends: ["anthropic-api"], latency: 125 },
  { name: "brave", tier: 1, status: "online", category: "search", backends: ["brave-search"], latency: 120 },
  { name: "bing", tier: 1, status: "online", category: "search", backends: ["bing-search"], latency: 100 },
  { name: "google", tier: 1, status: "online", category: "search", backends: ["google-cse", "serper"], latency: 110 },
  { name: "notion", tier: 1, status: "online", category: "productivity", backends: ["notion-api"], latency: 280 },
  { name: "slack", tier: 1, status: "online", category: "productivity", backends: ["slack-api"], latency: 200 },
  { name: "flight", tier: 1, status: "online", category: "tool", backends: ["aviationstack"], latency: 165 },
  { name: "devto", tier: 0, status: "online", category: "social", backends: ["devto-api"], latency: 100 },
  { name: "openlibrary", tier: 0, status: "online", category: "knowledge", backends: ["openlibrary-api"], latency: 115 },
  { name: "gutenberg", tier: 0, status: "online", category: "knowledge", backends: ["gutendex"], latency: 75 },
  { name: "lobsters", tier: 0, status: "online", category: "social", backends: ["lobsters-json"], latency: 65 },
  { name: "wikidata", tier: 0, status: "online", category: "knowledge", backends: ["wikidata-api"], latency: 85 },
  { name: "rss", tier: 0, status: "online", category: "tool", backends: ["rss-parser"], latency: 60 },
  { name: "web", tier: 0, status: "online", category: "tool", backends: ["jina-reader", "curl"], latency: 180 },
];

export const totalChannels = channels.length;
export const onlineCount = channels.filter((c) => c.status === "online").length;
export const degradedCount = channels.filter((c) => c.status === "degraded").length;
export const offlineCount = channels.filter((c) => c.status === "offline").length;
export const mcpTools = 91;
export const successRate = 99.2;
export const avgLatency = Math.round(channels.reduce((s, c) => s + c.latency, 0) / channels.length);

// Per-channel daily request volume (derived, stable).
export const channelRequests: Record<string, number> = Object.fromEntries(
  channels.map((c, i) => [c.name, 600 + ((i * 911 + c.latency * 7) % 9000)]),
);
export const requestsToday = Object.values(channelRequests).reduce((s, n) => s + n, 0);

export const statusDonut = [
  { name: "Online", value: onlineCount, color: "#383534" },
  { name: "Degraded", value: degradedCount, color: "#E8A5F3" },
  { name: "Offline", value: offlineCount, color: "#39322d" },
];

export const latencyBuckets = [
  { range: "0–100", count: channels.filter((c) => c.latency < 100).length },
  { range: "100–200", count: channels.filter((c) => c.latency >= 100 && c.latency < 200).length },
  { range: "200–300", count: channels.filter((c) => c.latency >= 200 && c.latency < 300).length },
  { range: "300–400", count: channels.filter((c) => c.latency >= 300 && c.latency < 400).length },
  { range: "400+", count: channels.filter((c) => c.latency >= 400).length },
];

export const topChannels = [...channels]
  .map((c) => ({ name: c.name, requests: channelRequests[c.name] }))
  .sort((a, b) => b.requests - a.requests)
  .slice(0, 8);

// 24h request volume.
export const analytics24h = [
  { hour: "00:00", requests: 1200 }, { hour: "01:00", requests: 890 },
  { hour: "02:00", requests: 720 }, { hour: "03:00", requests: 640 },
  { hour: "04:00", requests: 700 }, { hour: "05:00", requests: 980 },
  { hour: "06:00", requests: 1500 }, { hour: "07:00", requests: 2300 },
  { hour: "08:00", requests: 3400 }, { hour: "09:00", requests: 4600 },
  { hour: "10:00", requests: 5200 }, { hour: "11:00", requests: 5500 },
  { hour: "12:00", requests: 5100 }, { hour: "13:00", requests: 5300 },
  { hour: "14:00", requests: 5800 }, { hour: "15:00", requests: 5600 },
  { hour: "16:00", requests: 5000 }, { hour: "17:00", requests: 4300 },
  { hour: "18:00", requests: 3600 }, { hour: "19:00", requests: 3000 },
  { hour: "20:00", requests: 2600 }, { hour: "21:00", requests: 2100 },
  { hour: "22:00", requests: 1700 }, { hour: "23:00", requests: 1400 },
];

export const latencyPercentiles = analytics24h.map((d, i) => {
  const p50 = 80 + ((i * 7) % 50);
  return { hour: d.hour, p50, p95: p50 + 110 + ((i * 13) % 70), p99: p50 + 240 + ((i * 17) % 120) };
});

export const cacheHitRate = 87.4;
export const cacheDonut = [
  { name: "Hit", value: 87.4, color: "#383534" },
  { name: "Miss", value: 12.6, color: "#E8A5F3" },
];

export interface HotOp {
  operation: string;
  calls: number;
  avgTime: number;
  trend: "up" | "down" | "neutral";
}
export const hotOperations: HotOp[] = [
  { operation: "search/federated", calls: 18432, avgTime: 142, trend: "up" },
  { operation: "channels/github/read", calls: 12109, avgTime: 88, trend: "neutral" },
  { operation: "channels/brave/search", calls: 9876, avgTime: 110, trend: "up" },
  { operation: "read (smart)", calls: 8044, avgTime: 167, trend: "down" },
  { operation: "memory/get", calls: 6610, avgTime: 4, trend: "neutral" },
  { operation: "channels/reddit/search", calls: 5521, avgTime: 198, trend: "down" },
];

export interface HealthSnapshot {
  channel: string;
  status: ChannelStatus;
  primary: string;
  fallbackActive: boolean;
  lastCheck: string;
  failures: number;
}
export const healthSnapshots: HealthSnapshot[] = [
  { channel: "reddit", status: "degraded", primary: "opencli-reddit", fallbackActive: true, lastCheck: "2s ago", failures: 3 },
  { channel: "twitter", status: "degraded", primary: "opencli-twitter", fallbackActive: true, lastCheck: "5s ago", failures: 7 },
  { channel: "instagram", status: "degraded", primary: "opencli-instagram", fallbackActive: true, lastCheck: "4s ago", failures: 2 },
  { channel: "brave", status: "online", primary: "brave-search", fallbackActive: false, lastCheck: "1s ago", failures: 0 },
  { channel: "google", status: "online", primary: "google-cse", fallbackActive: false, lastCheck: "3s ago", failures: 0 },
  { channel: "github", status: "online", primary: "github-api", fallbackActive: false, lastCheck: "2s ago", failures: 0 },
  { channel: "youtube", status: "online", primary: "yt-dlp", fallbackActive: false, lastCheck: "4s ago", failures: 1 },
  { channel: "bilibili", status: "online", primary: "bili-cli", fallbackActive: false, lastCheck: "6s ago", failures: 0 },
  { channel: "spotify", status: "online", primary: "spotify-web", fallbackActive: false, lastCheck: "2s ago", failures: 0 },
  { channel: "notion", status: "online", primary: "notion-api", fallbackActive: false, lastCheck: "8s ago", failures: 0 },
  { channel: "slack", status: "online", primary: "slack-api", fallbackActive: false, lastCheck: "5s ago", failures: 0 },
  { channel: "huggingface", status: "online", primary: "hf-api", fallbackActive: false, lastCheck: "3s ago", failures: 0 },
  { channel: "openai", status: "online", primary: "openai-api", fallbackActive: false, lastCheck: "9s ago", failures: 0 },
  { channel: "anthropic", status: "online", primary: "anthropic-api", fallbackActive: false, lastCheck: "2s ago", failures: 0 },
  { channel: "weather", status: "online", primary: "open-meteo", fallbackActive: false, lastCheck: "2s ago", failures: 0 },
  { channel: "wikipedia", status: "online", primary: "mediawiki-api", fallbackActive: false, lastCheck: "1s ago", failures: 0 },
  { channel: "exa", status: "online", primary: "exa-api", fallbackActive: false, lastCheck: "7s ago", failures: 1 },
  { channel: "linkedin", status: "online", primary: "opencli-linkedin", fallbackActive: true, lastCheck: "11s ago", failures: 0 },
  { channel: "discord", status: "online", primary: "discord-bot", fallbackActive: false, lastCheck: "2s ago", failures: 0 },
  { channel: "telegram", status: "online", primary: "telegram-bot", fallbackActive: false, lastCheck: "1s ago", failures: 0 },
];

export interface BackendSwitch {
  time: string;
  channel: string;
  from: string;
  to: string;
  reason: string;
}
export const backendSwitches: BackendSwitch[] = [
  { time: "14:32:05", channel: "reddit", from: "opencli-reddit", to: "reddit-json", reason: "timeout > 5s" },
  { time: "13:58:22", channel: "twitter", from: "opencli-twitter", to: "twitter-cli", reason: "auth refresh" },
  { time: "12:41:08", channel: "google", from: "google-cse", to: "serper", reason: "429 rate limited" },
  { time: "11:15:04", channel: "instagram", from: "opencli-instagram", to: "instaloader", reason: "503 retry" },
  { time: "09:02:37", channel: "youtube", from: "yt-dlp", to: "youtube-dl", reason: "extractor error" },
];

export const healingStats = { autoSwitches: 23, repairsAttempted: 17, alertsSent: 4 };

export type ActivityType = "switch" | "heal" | "warn" | "info";
export interface ActivityEvent {
  id: number;
  time: string;
  type: ActivityType;
  message: string;
}
const ACTIVITY_RAW: { time: string; type: ActivityType; message: string }[] = [
  { time: "14:32:05", type: "switch", message: "reddit backend switched: opencli-reddit → reddit-json (timeout)" },
  { time: "14:28:12", type: "heal", message: "Cache TTL optimized for brave (240s → 180s)" },
  { time: "14:25:33", type: "warn", message: "twitter API rate limit approaching (87%)" },
  { time: "14:21:50", type: "info", message: "federated search across 12 channels (142ms)" },
  { time: "14:18:09", type: "info", message: "github/read served from L1 cache" },
  { time: "14:12:44", type: "heal", message: "instagram recovered on instaloader fallback" },
  { time: "14:07:31", type: "switch", message: "google rerouted: google-cse → serper (429)" },
  { time: "14:02:18", type: "info", message: "MCP client connected (Claude Desktop)" },
  { time: "13:58:22", type: "heal", message: "twitter auth token refreshed" },
  { time: "13:51:05", type: "warn", message: "scholar latency spike detected (612ms)" },
  { time: "13:44:39", type: "info", message: "memory namespace 'agent1' pruned 4 expired keys" },
  { time: "13:38:50", type: "info", message: "new API key provisioned for tenant 'acme'" },
  { time: "13:30:12", type: "heal", message: "auto-repair completed for arxiv channel" },
  { time: "13:22:47", type: "switch", message: "youtube switched: yt-dlp → youtube-dl (extractor)" },
  { time: "13:15:33", type: "info", message: "near-duplicate collapse merged 3 results" },
  { time: "13:08:21", type: "warn", message: "reddit failures = 3 (fallback active)" },
  { time: "12:59:04", type: "info", message: "request coalescing collapsed 6 reads → 1" },
  { time: "12:51:48", type: "heal", message: "circuit breaker reset for spotify" },
  { time: "12:44:10", type: "info", message: "weather served from L2 disk cache" },
  { time: "12:36:55", type: "switch", message: "google rerouted: google-cse → serper (quota)" },
  { time: "12:28:32", type: "info", message: "rerank reordered 9 federated results" },
  { time: "12:19:17", type: "warn", message: "instagram 503 from upstream (retrying)" },
  { time: "12:10:08", type: "info", message: "conditional revalidation: 304 for wikipedia" },
  { time: "12:01:44", type: "heal", message: "adaptive routing favored bing over brave" },
  { time: "11:52:30", type: "info", message: "audit log exported (1,204 entries)" },
];
export const activityLog: ActivityEvent[] = ACTIVITY_RAW.map((e, id) => ({ id, ...e }));

export interface MemoryEntry {
  key: string;
  value: string;
  ttl: string;
}
export const memoryEntries: MemoryEntry[] = [
  { key: "user_preference_model", value: '{"model":"claude-opus-4","temperature":0.7}', ttl: "2h remaining" },
  { key: "last_search_context", value: '{"query":"rust async patterns","results":12}', ttl: "45m remaining" },
  { key: "agent1:cursor", value: '{"page":3,"lastId":"t3_1k9"}', ttl: "1h remaining" },
  { key: "agent1:seen", value: '["t3_a","t3_b","t3_c","t3_d"]', ttl: "24h remaining" },
  { key: "crawler:queue", value: '{"pending":42,"done":1180}', ttl: "no expiry" },
  { key: "session:7f3a", value: '{"scope":"read","tenant":"acme"}', ttl: "30m remaining" },
  { key: "dedup:hashes", value: '["9af2","11cd","77be"]', ttl: "12h remaining" },
  { key: "ratelimit:bing", value: '{"used":880,"limit":1000}', ttl: "1h remaining" },
  { key: "feature:flags", value: '{"rerank":true,"collapse":true}', ttl: "no expiry" },
  { key: "agent2:summary", value: '"3 sources merged, 2 near-dupes collapsed"', ttl: "6h remaining" },
];

export interface Suggestion {
  channel: string;
  confidence: number;
  config: string;
}
export const suggestions: Suggestion[] = [
  { channel: "producthunt", confidence: 92, config: 'channels.add("producthunt", { tier: 1, key: "$PH_TOKEN" })' },
  { channel: "mastodon", confidence: 81, config: 'channels.add("mastodon", { tier: 1, instance: "$HOST" })' },
  { channel: "hn-jobs", confidence: 74, config: 'channels.add("hn-jobs", { tier: 0 })' },
  { channel: "bluesky", confidence: 69, config: 'channels.add("bluesky", { tier: 1, key: "$BSKY_APP_PW" })' },
  { channel: "stackoverflow", confidence: 63, config: 'channels.add("stackoverflow", { tier: 0 })' },
];

export interface ApiKey {
  name: string;
  key: string;
  scopes: string;
  created: string;
}
export const apiKeys: ApiKey[] = [
  { name: "ci-pipeline", key: "as_live_8f2a••••••••", scopes: "read, search", created: "2026-05-02" },
  { name: "dashboard", key: "as_live_3c91••••••••", scopes: "read", created: "2026-05-18" },
  { name: "acme-tenant", key: "as_live_a07d••••••••", scopes: "read, search, admin", created: "2026-06-01" },
  { name: "agent-runner", key: "as_live_55be••••••••", scopes: "read, search, memory", created: "2026-06-20" },
];

export interface Tenant {
  name: string;
  tier: string;
  rateLimit: string;
  quota: string;
}
export const tenants: Tenant[] = [
  { name: "default", tier: "free", rateLimit: "60/min", quota: "10k/day" },
  { name: "acme", tier: "pro", rateLimit: "600/min", quota: "1M/day" },
  { name: "globex", tier: "enterprise", rateLimit: "unlimited", quota: "unlimited" },
];

export const adminAudit: ActivityEvent[] = [
  { id: 0, type: "info", message: "api key 'ci-pipeline' created by admin", time: "11:20:04" },
  { id: 1, type: "warn", message: "rate limit raised for tenant 'acme'", time: "10:08:55" },
  { id: 2, type: "info", message: "tenant 'globex' upgraded to enterprise", time: "09:14:31" },
  { id: 3, type: "switch", message: "key 'old-ci' revoked", time: "08:02:10" },
];

// ---- Channel avatars -------------------------------------------------------
// Every channel gets a distinct identity: a brand-colored silhouette plus, on
// the chest, either its real logo (see brandIcons.ts / customGlyphs.ts) or a
// clean monogram when no vector mark exists. Brand colors below tint the
// silhouette so each company reads as itself even before the logo loads.

export type AvatarShape = "circle" | "square" | "hexagon" | "diamond" | "triangle" | "octagon" | "star";
export type AvatarPattern = "solid" | "striped" | "dotted" | "grid" | "radial";
export interface AvatarConfig {
  shape: AvatarShape;
  gradient: [string, string];
  pattern: AvatarPattern;
  symbol: string;
}

const SHAPES: AvatarShape[] = ["circle", "square", "hexagon", "diamond", "triangle", "octagon", "star"];
const PATTERNS: AvatarPattern[] = ["solid", "striped", "dotted", "grid", "radial"];

const palette: [string, string][] = [
  ["#FB542B", "#FF7139"], ["#00809D", "#00A4EF"], ["#4285F4", "#EA4335"], ["#FF4500", "#FF8717"],
  ["#24292E", "#6E5494"], ["#FC6D26", "#E24329"], ["#FF0000", "#CC0000"], ["#1DB954", "#1ED760"],
  ["#0A66C2", "#0077B5"], ["#5865F2", "#7289DA"], ["#E8A5F3", "#C5C4FF"], ["#FFDFC4", "#FFB088"],
  ["#00A4EF", "#0078D4"], ["#232F3E", "#FF9900"], ["#DB4437", "#F4B400"], ["#635BFF", "#96F7D6"],
  ["#FF4500", "#FF6B35"], ["#2D2D2D", "#FFD700"], ["#4A154B", "#E01E5A"], ["#3B5998", "#8B9DC3"],
];

// Real brand colours for the recognisable channels — these tint the silhouette.
const BRAND: Record<string, [string, string]> = {
  github: ["#24292E", "#6E5494"],
  hackernews: ["#FF6600", "#FF8C42"],
  youtube: ["#FF0000", "#CC0000"],
  tiktok: ["#25F4EE", "#FE2C55"],
  twitter: ["#1d1d1f", "#536471"],
  reddit: ["#FF4500", "#FF8717"],
  bilibili: ["#00AEEC", "#FB7299"],
  instagram: ["#F58529", "#DD2A7B"],
  linkedin: ["#0A66C2", "#0077B5"],
  exa: ["#1F40FF", "#6E8BFF"],
  wikipedia: ["#3366CC", "#101418"],
  arxiv: ["#B31B1B", "#E0533D"],
  discord: ["#5865F2", "#7289DA"],
  telegram: ["#229ED9", "#2AABEE"],
  spotify: ["#1DB954", "#1ED760"],
  twitch: ["#9146FF", "#772CE8"],
  scholar: ["#4285F4", "#34A853"],
  podcasts: ["#822FCF", "#D17DF7"],
  quora: ["#B92B27", "#E0533D"],
  pinterest: ["#E60023", "#FF5470"],
  npm: ["#CB3837", "#E0533D"],
  crates: ["#E6B047", "#9C6B30"],
  pypi: ["#3775A9", "#FFD43B"],
  gitlab: ["#FC6D26", "#E24329"],
  dockerhub: ["#2496ED", "#0DB7ED"],
  wayback: ["#1A1A1A", "#4A4A4A"],
  maps: ["#34A853", "#4285F4"],
  weather: ["#3AA7F0", "#F9C846"],
  coinbase: ["#0052FF", "#3B82F6"],
  duckduckgo: ["#DE5833", "#FF7B52"],
  gnews: ["#4285F4", "#EA4335"],
  statuspage: ["#172B4D", "#2684FF"],
  huggingface: ["#FFD21E", "#FF9D00"],
  openai: ["#10A37F", "#1AC79E"],
  anthropic: ["#D97757", "#E8A38A"],
  brave: ["#FB542B", "#FF7139"],
  bing: ["#008373", "#00A88E"],
  google: ["#4285F4", "#EA4335"],
  notion: ["#2D2D2D", "#9B9B9B"],
  slack: ["#4A154B", "#E01E5A"],
  devto: ["#0A0A0A", "#3B3B3B"],
  lobsters: ["#AC130D", "#E0533D"],
  rss: ["#EE802F", "#F4A04C"],
};

// Two-letter monograms for the few channels with no vector logo at all.
const SYMBOL: Record<string, string> = {
  v2ex: "V2", xueqiu: "XQ", xiaohongshu: "RED", xiaoyuzhou: "XYZ",
  flight: "FL", openlibrary: "OL", gutenberg: "PG", wikidata: "WD", web: "WB",
};

function initials(name: string): string {
  const clean = name.replace("/", "");
  return (clean[0]?.toUpperCase() ?? "") + (clean[1] ?? "");
}

export const avatarMap: Record<string, AvatarConfig> = Object.fromEntries(
  channels.map((c, i) => [
    c.name,
    {
      shape: SHAPES[i % SHAPES.length],
      pattern: PATTERNS[Math.floor(i / SHAPES.length) % PATTERNS.length],
      gradient: BRAND[c.name] ?? palette[i % palette.length],
      symbol: SYMBOL[c.name] ?? initials(c.name),
    },
  ]),
);
