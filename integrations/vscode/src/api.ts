import * as vscode from "vscode";

/**
 * Typed client for the AgentSpan HTTP gateway.
 *
 * Reads `agentspan.serverUrl` and `agentspan.apiKey` from the workspace
 * configuration on every request, so changes take effect without reloading.
 *
 * Relies on the global `fetch` available in modern VS Code's Node runtime
 * (Node 18+, which ships with VS Code 1.82+).
 */

export interface HealthResponse {
  status: string;
}

export interface ReadContent {
  url: string;
  title?: string;
  body?: string;
  [key: string]: unknown;
}

export interface ReadResponse {
  url: string;
  channel: string;
  content: ReadContent;
}

export interface Channel {
  name: string;
  description: string;
  tier: string;
}

export interface ChannelsResponse {
  channels: Channel[];
}

export interface SearchResult {
  title: string;
  url: string;
  snippet: string;
}

export interface ChannelSearchResponse {
  channel: string;
  results: SearchResult[];
}

export interface FederatedResult {
  channels: string[];
  title: string;
  url: string;
  snippet: string;
}

export interface FederatedSearchResponse {
  query: string;
  searched: string[];
  results: FederatedResult[];
}

export interface FederatedSearchRequest {
  query: string;
  channels?: string[];
  limit?: number;
}

/** Thrown when the gateway returns a non-2xx response. */
export class AgentSpanError extends Error {
  constructor(
    message: string,
    public readonly status?: number,
    public readonly url?: string
  ) {
    super(message);
    this.name = "AgentSpanError";
  }
}

export class AgentSpanClient {
  private readonly section = "agentspan";

  /** Base URL with any trailing slash stripped. */
  private get serverUrl(): string {
    const raw = vscode.workspace
      .getConfiguration(this.section)
      .get<string>("serverUrl", "http://localhost:8080");
    return raw.replace(/\/+$/, "");
  }

  private get apiKey(): string {
    return vscode.workspace
      .getConfiguration(this.section)
      .get<string>("apiKey", "")
      .trim();
  }

  private headers(extra?: Record<string, string>): Record<string, string> {
    const headers: Record<string, string> = {
      Accept: "application/json",
      ...extra,
    };
    const key = this.apiKey;
    if (key) {
      headers["X-API-Key"] = key;
    }
    return headers;
  }

  private buildUrl(path: string, params?: Record<string, string | number | undefined>): string {
    const url = new URL(`${this.serverUrl}${path}`);
    if (params) {
      for (const [name, value] of Object.entries(params)) {
        if (value !== undefined && value !== "") {
          url.searchParams.set(name, String(value));
        }
      }
    }
    return url.toString();
  }

  private async request<T>(url: string, init?: RequestInit): Promise<T> {
    let response: Response;
    try {
      response = await fetch(url, {
        ...init,
        headers: this.headers(init?.headers as Record<string, string> | undefined),
      });
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);
      throw new AgentSpanError(
        `Could not reach AgentSpan at ${url}: ${detail}`,
        undefined,
        url
      );
    }

    if (!response.ok) {
      let body = "";
      try {
        body = await response.text();
      } catch {
        /* ignore body read failures */
      }
      const suffix = body ? ` - ${body.slice(0, 200)}` : "";
      throw new AgentSpanError(
        `AgentSpan returned ${response.status} ${response.statusText}${suffix}`,
        response.status,
        url
      );
    }

    return (await response.json()) as T;
  }

  /** GET /health */
  async health(): Promise<HealthResponse> {
    return this.request<HealthResponse>(this.buildUrl("/health"));
  }

  /** GET /api/v1/read?url=<url> */
  async read(targetUrl: string): Promise<ReadResponse> {
    return this.request<ReadResponse>(
      this.buildUrl("/api/v1/read", { url: targetUrl })
    );
  }

  /** GET /api/v1/channels */
  async channels(): Promise<ChannelsResponse> {
    return this.request<ChannelsResponse>(this.buildUrl("/api/v1/channels"));
  }

  /** GET /api/v1/channels/{name}/search?q=<q>&limit=<limit> */
  async searchChannel(
    channel: string,
    query: string,
    limit = 10
  ): Promise<ChannelSearchResponse> {
    return this.request<ChannelSearchResponse>(
      this.buildUrl(`/api/v1/channels/${encodeURIComponent(channel)}/search`, {
        q: query,
        limit,
      })
    );
  }

  /** POST /api/v1/search/federated */
  async federatedSearch(
    body: FederatedSearchRequest
  ): Promise<FederatedSearchResponse> {
    return this.request<FederatedSearchResponse>(
      this.buildUrl("/api/v1/search/federated"),
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ limit: 10, ...body }),
      }
    );
  }
}
