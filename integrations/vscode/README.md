# AgentSpan for VS Code

Read and search the web directly from VS Code through the
[AgentSpan](https://github.com/) HTTP gateway. AgentSpan is a gateway that lets
AI agents (and you) read pages and search across many "channels" behind one
simple API; this extension is a thin client over that gateway.

## Features

- **AgentSpan: Search Web** — search a single channel and open any result.
- **AgentSpan: Federated Search** — search across several channels at once
  (or let the gateway pick its defaults).
- **AgentSpan: Read URL** — fetch and render a single URL in a clean reader
  panel.
- **Status bar health indicator** — polls the gateway's `/health` endpoint every
  ~30 seconds and shows `✓ AgentSpan` (healthy) or `✗ AgentSpan` (unreachable /
  unhealthy). Click it to jump straight into a web search.

Search results are shown in a Quick Pick (and mirrored to the **AgentSpan**
output channel); selecting one reads that URL and opens it in a webview reader.

## Requirements

A running AgentSpan gateway reachable from your machine. By default the
extension talks to `http://localhost:8080`.

> The extension uses the global `fetch` available in modern VS Code's Node
> runtime, so VS Code **1.85.0 or newer** is recommended.

## Configuration

Open *Settings* and search for "AgentSpan", or edit `settings.json`:

| Setting              | Default                  | Description                                                        |
| -------------------- | ------------------------ | ------------------------------------------------------------------ |
| `agentspan.serverUrl`| `http://localhost:8080`  | Base URL of the AgentSpan gateway (no trailing slash).             |
| `agentspan.apiKey`   | `""`                     | Optional API key, sent as the `X-API-Key` header. Empty = none.    |

```jsonc
{
  "agentspan.serverUrl": "https://agentspan.internal.example.com",
  "agentspan.apiKey": "sk-your-key-here"
}
```

Configuration is read on every request, so changes take effect immediately —
no window reload required.

## Commands

| Command                    | Title                        |
| -------------------------- | ---------------------------- |
| `agentspan.searchWeb`      | AgentSpan: Search Web        |
| `agentspan.readUrl`        | AgentSpan: Read URL          |
| `agentspan.federatedSearch`| AgentSpan: Federated Search  |

Run them from the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`).

## API endpoints used

The extension is a client for the following AgentSpan REST endpoints:

- `GET  /health`
- `GET  /api/v1/read?url=<url>`
- `GET  /api/v1/channels`
- `GET  /api/v1/channels/{name}/search?q=<q>&limit=10`
- `POST /api/v1/search/federated`

## Development

```bash
npm install      # install @types/vscode + typescript
npm run compile  # type-check and emit to ./out
# or, for an incremental loop:
npm run watch
```

Then press **F5** in VS Code to launch an *Extension Development Host* with the
extension loaded. Use the Command Palette there to try the commands against a
running gateway.

### Project layout

```
integrations/vscode/
├─ package.json        # manifest: commands, settings, activation
├─ tsconfig.json       # TypeScript config (Node16 modules, strict)
├─ src/
│  ├─ api.ts           # typed AgentSpanClient over the REST API
│  └─ extension.ts     # commands, status bar, webview reader
├─ README.md
├─ .vscodeignore
└─ .gitignore
```

## License

MIT
