# agentspan.nvim

A small Neovim plugin for [AgentSpan](https://github.com/) — an HTTP gateway
that lets AI agents (and you) **read and search the web** from inside the editor.

It talks to a running AgentSpan server over its REST API using `curl`, so it has
**no Lua dependencies** (no plenary required). It uses `vim.system` on Neovim
0.10+ and transparently falls back to `vim.fn.system` on older versions.

## Requirements

- Neovim **0.7+** (0.10+ recommended for async `vim.system`)
- `curl` available on your `$PATH`
- A **running AgentSpan server** (default `http://localhost:8080`).
  The plugin only talks to the API — it does not start the server for you.

## Install

### lazy.nvim

```lua
{
  "your-org/agentspan.nvim",
  -- if you vendor it from the AgentSpan repo:
  -- dir = "/path/to/agentspan/integrations/nvim",
  cmd = {
    "AgentSpanRead",
    "AgentSpanSearch",
    "AgentSpanFederated",
    "AgentSpanChannels",
    "AgentSpanHealth",
  },
  opts = {
    server_url = "http://localhost:8080",
    api_key = nil, -- set if your server requires an X-API-Key
  },
  -- `opts` is passed straight to require("agentspan").setup(opts)
}
```

### packer.nvim

```lua
use({
  "your-org/agentspan.nvim",
  config = function()
    require("agentspan").setup({
      server_url = "http://localhost:8080",
      api_key = nil,
    })
  end,
})
```

### Manual / vendored

Add this directory to your `runtimepath` (or symlink it into your `pack/`),
then call `setup` somewhere in your config:

```lua
require("agentspan").setup({
  server_url = "http://localhost:8080",
  api_key = nil,
})
```

## Configuration

`setup(opts)` accepts:

| Option       | Type            | Default                   | Description                                  |
| ------------ | --------------- | ------------------------- | -------------------------------------------- |
| `server_url` | `string`        | `"http://localhost:8080"` | Base URL of the AgentSpan server.            |
| `api_key`    | `string \| nil` | `nil`                     | Sent as the `X-API-Key` header when present. |
| `timeout`    | `number`        | `30`                      | Per-request timeout in seconds (curl).       |

Calling `setup` is optional — if you skip it the defaults above are used.

## Commands

| Command                          | Description                                                              |
| -------------------------------- | ------------------------------------------------------------------------ |
| `:AgentSpanRead <url>`           | Fetch & extract a URL; opens the body in a markdown scratch buffer.      |
| `:AgentSpanSearch <channel> <q>` | Search a single channel; results in a buffer plus a `vim.ui.select` pick. |
| `:AgentSpanFederated <query>`    | Federated search across all channels; results in a buffer.               |
| `:AgentSpanChannels`             | List the channels the server exposes.                                    |
| `:AgentSpanHealth`               | Ping the server `/health` endpoint and notify the result.                |

Examples:

```vim
:AgentSpanRead https://neovim.io
:AgentSpanSearch duckduckgo rust async runtime
:AgentSpanFederated best lua plugin manager
:AgentSpanChannels
```

In any result buffer, press `q` to close it. After a search, a
`vim.ui.select` prompt lets you pick a result to open with `:AgentSpanRead`.

## Lua API

```lua
local agentspan = require("agentspan")

agentspan.read("https://example.com")
agentspan.search("duckduckgo", "neovim lsp", { limit = 10 })
agentspan.federated("rust web framework", { limit = 10 })
agentspan.channels()
```

## Statusline

`require("agentspan").health()` returns a cached health string
(`"AgentSpan: ok"`, `"AgentSpan: down"`, or `""` before the first check) and
refreshes the value asynchronously in the background. It is cheap to call on
every redraw, so you can embed it directly:

```lua
-- native statusline
vim.o.statusline = vim.o.statusline .. "%{v:lua.require'agentspan'.health()}"
```

```lua
-- lualine
require("lualine").setup({
  sections = {
    lualine_x = {
      function() return require("agentspan").health() end,
    },
  },
})
```

## API endpoints used

The plugin calls the following AgentSpan REST endpoints:

- `GET  /api/v1/read?url=<url>`
- `GET  /api/v1/channels/{name}/search?q=<q>&limit=10`
- `POST /api/v1/search/federated` — body `{ query, limit }`
- `GET  /api/v1/channels`
- `GET  /health`

When `api_key` is configured, every request includes an `X-API-Key` header.

## Notes

- Make sure the AgentSpan server is running and reachable at `server_url`
  before using the commands; otherwise you'll get a "server unreachable" /
  request-failed notification.
- All network calls are asynchronous on Neovim 0.10+ and will not block the UI.
