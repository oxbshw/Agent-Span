// Official JavaScript/TypeScript SDK for the AgentSpan gateway.
//
// Thin client over the AgentSpan REST API (see docs/api-reference.md). Uses the
// global `fetch` (Node >=18, all modern browsers); a custom `fetch` can be
// injected for testing.

export class AgentSpanError extends Error {}

export class AuthenticationError extends AgentSpanError {
  constructor(message) {
    super(message);
    this.name = "AuthenticationError";
    this.status = 401;
  }
}

export class RateLimitError extends AgentSpanError {
  constructor(message, retryAfter) {
    super(message);
    this.name = "RateLimitError";
    this.status = 429;
    this.retryAfter = retryAfter ?? null;
  }
}

export class APIError extends AgentSpanError {
  constructor(status, message) {
    super(message);
    this.name = "APIError";
    this.status = status;
  }
}

export class ChannelError extends AgentSpanError {
  constructor(message) {
    super(message);
    this.name = "ChannelError";
  }
}

/** Async client for the AgentSpan gateway. */
export class AgentSpanClient {
  /**
   * @param {{apiKey?: string, baseUrl?: string, fetch?: typeof fetch}} [options]
   */
  constructor(options = {}) {
    this.apiKey = options.apiKey ?? null;
    this.baseUrl = (options.baseUrl ?? "http://localhost:8080").replace(/\/+$/, "");
    this._fetch = options.fetch ?? globalThis.fetch;
    if (!this._fetch) {
      throw new AgentSpanError("no fetch implementation available; pass options.fetch");
    }
  }

  _headers(extra = {}) {
    const headers = { ...extra };
    if (this.apiKey) headers["X-API-Key"] = this.apiKey;
    return headers;
  }

  async _request(method, path, { query, body } = {}) {
    let url = this.baseUrl + path;
    if (query) {
      const qs = new URLSearchParams(query).toString();
      if (qs) url += "?" + qs;
    }
    const init = { method, headers: this._headers() };
    if (body !== undefined) {
      init.headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(body);
    }
    const res = await this._fetch(url, init);
    await this._check(res);
    if (res.status === 204) return null;
    return res.json();
  }

  async _check(res) {
    if (res.status < 400) return;
    let message = "";
    try {
      const data = await res.clone().json();
      message = data.error ?? JSON.stringify(data);
    } catch {
      message = await res.text().catch(() => res.statusText);
    }
    if (res.status === 401) throw new AuthenticationError(message);
    if (res.status === 429) {
      const retry = res.headers.get("Retry-After");
      throw new RateLimitError(message, retry ? parseInt(retry, 10) : null);
    }
    throw new APIError(res.status, message);
  }

  /** Read a URL via the best matching channel. */
  async read(url, forceRefresh = false) {
    const data = await this._request("GET", "/api/v1/read", {
      query: { url, force_refresh: String(forceRefresh) },
    });
    if (data.error) throw new ChannelError(data.error);
    return data.content;
  }

  /** Search a platform via a named channel. */
  async search(channel, query, limit = 10) {
    const data = await this._request(
      "GET",
      `/api/v1/channels/${encodeURIComponent(channel)}/search`,
      { query: { q: query, limit: String(limit) } }
    );
    if (data.error) throw new ChannelError(data.error);
    return data.results ?? [];
  }

  /** List available channels. */
  async listChannels() {
    const data = await this._request("GET", "/api/v1/channels");
    return data.channels ?? [];
  }

  /** Run health diagnostics across all channels. */
  async doctor() {
    return this._request("GET", "/api/v1/doctor");
  }

  /** Fetch the non-secret server configuration view. */
  async getConfig() {
    return this._request("GET", "/api/v1/config");
  }

  /** Read many URLs in parallel (server-side batch). */
  async batchRead(urls, forceRefresh = false) {
    const data = await this._request("POST", "/api/v1/batch/read", {
      body: { urls, force_refresh: forceRefresh },
    });
    return data.results ?? [];
  }

  /** Run many queries against one channel in parallel (server-side batch). */
  async batchSearch(channel, queries, limit = 10) {
    const data = await this._request("POST", "/api/v1/batch/search", {
      body: { channel, queries, limit },
    });
    return data.results ?? [];
  }

  /** Return true when the server's /health endpoint is OK. */
  async health() {
    try {
      const res = await this._fetch(this.baseUrl + "/health", {
        headers: this._headers(),
      });
      return res.status === 200;
    } catch {
      return false;
    }
  }

  /** Create a new API key (requires admin scope). */
  async createKey(name, scopes = ["read"], tenantId = "default") {
    return this._request("POST", "/api/v1/auth/keys", {
      body: { name, scopes, tenant_id: tenantId },
    });
  }

  /** Revoke an API key by id (requires admin scope). */
  async revokeKey(id) {
    await this._request("DELETE", `/api/v1/auth/keys/${encodeURIComponent(id)}`);
  }

  /**
   * Stream live server events (SSE). Calls `onEvent(parsedJson)` per frame.
   * Returns a function that aborts the stream.
   */
  streamEvents(onEvent) {
    const controller = new AbortController();
    (async () => {
      const res = await this._fetch(this.baseUrl + "/api/v1/events/stream", {
        headers: this._headers({ Accept: "text/event-stream" }),
        signal: controller.signal,
      });
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        const frames = buffer.split("\n\n");
        buffer = frames.pop() ?? "";
        for (const frame of frames) {
          const line = frame.split("\n").find((l) => l.startsWith("data:"));
          if (line) {
            try {
              onEvent(JSON.parse(line.slice(5).trim()));
            } catch {
              /* ignore malformed frame */
            }
          }
        }
      }
    })().catch(() => {});
    return () => controller.abort();
  }
}

export default AgentSpanClient;
