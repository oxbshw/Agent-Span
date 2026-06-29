// Tests for AgentSpanClient using an injected stub fetch (no network, no deps).
// Run with: node --test

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  AgentSpanClient,
  AuthenticationError,
  RateLimitError,
  APIError,
  ChannelError,
} from "../src/index.js";

/** Build a stub fetch that records the last request and returns `responder`. */
function stubFetch(responder) {
  const calls = [];
  const fn = async (url, init = {}) => {
    calls.push({ url, init });
    return responder(url, init, calls.length - 1);
  };
  fn.calls = calls;
  return fn;
}

function jsonResponse(status, body, headers = {}) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });
}

function makeClient(fetchImpl) {
  return new AgentSpanClient({ apiKey: "k", baseUrl: "http://test", fetch: fetchImpl });
}

test("read returns content", async () => {
  const fetchImpl = stubFetch((url) => {
    assert.ok(url.startsWith("http://test/api/v1/read"));
    assert.ok(url.includes("url=https"));
    return jsonResponse(200, {
      channel: "web",
      content: { url: "https://x", title: "Title", body: "hello", metadata: null, cached: false },
    });
  });
  const client = makeClient(fetchImpl);
  const content = await client.read("https://x");
  assert.equal(content.body, "hello");
  assert.equal(content.title, "Title");
});

test("read raises ChannelError on embedded error", async () => {
  const client = makeClient(stubFetch(() => jsonResponse(200, { error: "no channel" })));
  await assert.rejects(() => client.read("ftp://x"), ChannelError);
});

test("search maps results and passes limit", async () => {
  const fetchImpl = stubFetch((url) => {
    assert.ok(url.includes("/api/v1/channels/hackernews/search"));
    assert.ok(url.includes("limit=7"));
    return jsonResponse(200, { results: [{ title: "Rust", url: "https://r", snippet: "s" }] });
  });
  const client = makeClient(fetchImpl);
  const results = await client.search("hackernews", "rust", 7);
  assert.equal(results[0].title, "Rust");
});

test("listChannels returns the array", async () => {
  const client = makeClient(
    stubFetch(() => jsonResponse(200, { channels: [{ name: "web", description: "d", tier: "Zero" }] }))
  );
  const channels = await client.listChannels();
  assert.equal(channels.length, 1);
  assert.equal(channels[0].name, "web");
});

test("authentication error on 401", async () => {
  const client = makeClient(stubFetch(() => jsonResponse(401, { error: "invalid API key" })));
  await assert.rejects(() => client.listChannels(), AuthenticationError);
});

test("rate limit error carries retry-after", async () => {
  const client = makeClient(
    stubFetch(() => jsonResponse(429, { error: "slow down" }, { "Retry-After": "12" }))
  );
  await assert.rejects(
    () => client.read("https://x"),
    (err) => err instanceof RateLimitError && err.retryAfter === 12
  );
});

test("generic API error on 500", async () => {
  const client = makeClient(stubFetch(() => jsonResponse(500, { error: "boom" })));
  await assert.rejects(() => client.listChannels(), APIError);
});

test("getConfig returns the config object", async () => {
  const client = makeClient(stubFetch(() => jsonResponse(200, { cache: { enabled: true } })));
  const cfg = await client.getConfig();
  assert.equal(cfg.cache.enabled, true);
});

test("batchRead posts urls and returns results", async () => {
  const fetchImpl = stubFetch((url, init) => {
    assert.equal(init.method, "POST");
    const body = JSON.parse(init.body);
    assert.deepEqual(body.urls, ["https://a", "https://b"]);
    return jsonResponse(200, {
      count: 2,
      results: [
        { url: "https://a", ok: true },
        { url: "https://b", ok: false, error: "x" },
      ],
    });
  });
  const client = makeClient(fetchImpl);
  const results = await client.batchRead(["https://a", "https://b"]);
  assert.equal(results.length, 2);
  assert.equal(results[0].ok, true);
});

test("batchSearch posts channel + queries", async () => {
  const fetchImpl = stubFetch((url, init) => {
    const body = JSON.parse(init.body);
    assert.equal(body.channel, "hackernews");
    assert.deepEqual(body.queries, ["rust", "go"]);
    return jsonResponse(200, { results: [{ query: "rust", ok: true, results: [] }] });
  });
  const client = makeClient(fetchImpl);
  const results = await client.batchSearch("hackernews", ["rust", "go"]);
  assert.equal(results[0].query, "rust");
});

test("health is true on 200, false on error", async () => {
  const ok = makeClient(stubFetch(() => new Response("", { status: 200 })));
  assert.equal(await ok.health(), true);
  const bad = makeClient(stubFetch(() => new Response("", { status: 503 })));
  assert.equal(await bad.health(), false);
});

test("createKey posts and returns the key", async () => {
  const fetchImpl = stubFetch((url, init) => {
    assert.equal(init.method, "POST");
    return jsonResponse(201, { id: "abc", secret: "as_secret", name: "ci", tenant_id: "default" });
  });
  const client = makeClient(fetchImpl);
  const key = await client.createKey("ci", ["read"]);
  assert.equal(key.secret, "as_secret");
});

test("revokeKey issues a DELETE", async () => {
  const fetchImpl = stubFetch((url, init) => {
    assert.equal(init.method, "DELETE");
    assert.ok(url.endsWith("/api/v1/auth/keys/abc"));
    return new Response(null, { status: 204 });
  });
  const client = makeClient(fetchImpl);
  await client.revokeKey("abc");
});

test("sends X-API-Key header when configured", async () => {
  const fetchImpl = stubFetch((url, init) => {
    assert.equal(init.headers["X-API-Key"], "k");
    return jsonResponse(200, { channels: [] });
  });
  const client = makeClient(fetchImpl);
  await client.listChannels();
});
