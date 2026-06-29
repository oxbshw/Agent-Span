// Read a URL and search Hacker News — using the AgentSpan JS SDK.
//
// Prereq: agentspan serve --port 8080
// Run:    node examples/javascript/quickstart.mjs

import { AgentSpanClient } from "../../sdk/js/src/index.js";

const client = new AgentSpanClient({ baseUrl: "http://localhost:8080" });

// Read any URL — AgentSpan auto-detects the right channel.
const content = await client.read("https://news.ycombinator.com");
console.log(`Title: ${content.title || "(none)"}`);
console.log(`Body: ${content.body.slice(0, 200)}...`);
console.log(`Cached: ${content.cached}`);
console.log();

// Search Hacker News.
const results = await client.search("hackernews", "rust async", { limit: 5 });
console.log(`HN search results (${results.length}):`);
for (const r of results) {
  console.log(`  - ${r.title}`);
  console.log(`    ${r.url}`);
}
