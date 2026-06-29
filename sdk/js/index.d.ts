// Type declarations for @agentspan/sdk.

export interface Content {
  url: string;
  title: string | null;
  body: string;
  metadata: unknown;
  cached: boolean;
}

export interface SearchResult {
  title: string;
  url: string;
  snippet: string;
  author: string | null;
  timestamp: string | null;
  metadata: unknown;
}

export interface ChannelInfo {
  name: string;
  description: string;
  tier: string;
}

export interface ApiKey {
  id: string;
  secret?: string;
  name: string;
  tenant_id: string;
}

export interface BatchReadResult {
  url: string;
  ok: boolean;
  channel?: string;
  content?: Content;
  error?: string;
}

export interface BatchSearchResult {
  query: string;
  ok: boolean;
  results?: SearchResult[];
  error?: string;
}

export interface ClientOptions {
  apiKey?: string;
  baseUrl?: string;
  fetch?: typeof fetch;
}

export class AgentSpanError extends Error {}
export class AuthenticationError extends AgentSpanError {
  status: 401;
}
export class RateLimitError extends AgentSpanError {
  status: 429;
  retryAfter: number | null;
}
export class APIError extends AgentSpanError {
  status: number;
}
export class ChannelError extends AgentSpanError {}

export class AgentSpanClient {
  constructor(options?: ClientOptions);
  apiKey: string | null;
  baseUrl: string;
  read(url: string, forceRefresh?: boolean): Promise<Content>;
  search(channel: string, query: string, limit?: number): Promise<SearchResult[]>;
  listChannels(): Promise<ChannelInfo[]>;
  doctor(): Promise<unknown>;
  getConfig(): Promise<Record<string, unknown>>;
  batchRead(urls: string[], forceRefresh?: boolean): Promise<BatchReadResult[]>;
  batchSearch(channel: string, queries: string[], limit?: number): Promise<BatchSearchResult[]>;
  health(): Promise<boolean>;
  createKey(name: string, scopes?: string[], tenantId?: string): Promise<ApiKey>;
  revokeKey(id: string): Promise<void>;
  streamEvents(onEvent: (event: unknown) => void): () => void;
}

export default AgentSpanClient;
