# @agentspan/sdk

Official JavaScript/TypeScript SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```bash
npm install @agentspan/sdk
```

```js
import { AgentSpanClient } from "@agentspan/sdk";

const client = new AgentSpanClient({ baseUrl: "http://localhost:8080", apiKey: "as_..." });

const content = await client.read("https://example.com");
console.log(content.body);

const results = await client.search("hackernews", "rust", 10);
const many = await client.batchRead(["https://a", "https://b"]);

// Live events (SSE)
const stop = client.streamEvents((e) => console.log(e));
```

Works in Node ≥18 and modern browsers (uses global `fetch`). Ships TypeScript
types (`index.d.ts`). See [docs/api-reference.md](../../docs/api-reference.md) for
the full method set. Tests run with `npm test` (Node's built-in runner, no deps).
