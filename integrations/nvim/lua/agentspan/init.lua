-- agentspan
-- Neovim client for AgentSpan, an HTTP gateway that lets AI agents (and you)
-- read & search the web.
--
-- Public API:
--   require("agentspan").setup({ server_url = "...", api_key = "..." })
--   require("agentspan").read(url)
--   require("agentspan").search(channel, query)
--   require("agentspan").federated(query)
--   require("agentspan").channels()
--   require("agentspan").health()  -- statusline helper

local api = require("agentspan.api")

local M = {}

--- @class agentspan.Config
--- @field server_url string Base URL of the AgentSpan server.
--- @field api_key string|nil Optional X-API-Key value.
M.config = {
  server_url = "http://localhost:8080",
  api_key = nil,
}

----------------------------------------------------------------------
-- Setup
----------------------------------------------------------------------

--- Configure the plugin. Safe to call multiple times.
--- @param opts agentspan.Config|nil
function M.setup(opts)
  M.config = vim.tbl_deep_extend("force", M.config, opts or {})
  -- Push config into the HTTP layer.
  api.configure(M.config)
end

----------------------------------------------------------------------
-- Buffer helpers
----------------------------------------------------------------------

--- Open a scratch buffer in a split with the given lines and filetype.
--- @param name string buffer name shown in the statusline
--- @param lines string[] buffer contents (split into lines)
--- @param filetype string|nil filetype to set (default "markdown")
--- @return integer bufnr
local function open_scratch(name, lines, filetype)
  local buf = vim.api.nvim_create_buf(false, true) -- nofile, scratch

  vim.bo[buf].buftype = "nofile"
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].swapfile = false
  vim.bo[buf].filetype = filetype or "markdown"

  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false

  -- Give it a unique, friendly name (ignore clashes if reused).
  pcall(vim.api.nvim_buf_set_name, buf, name)

  -- Open in a new split and focus it.
  vim.cmd("botright split")
  vim.api.nvim_win_set_buf(0, buf)

  -- Convenience: q closes the scratch buffer.
  vim.keymap.set("n", "q", "<cmd>close<cr>", { buffer = buf, nowait = true, silent = true })

  return buf
end

--- Split a possibly-multiline string into a list of lines for a buffer.
--- @param str string|nil
--- @return string[]
local function to_lines(str)
  if str == nil then
    return { "" }
  end
  -- Normalise CRLF, then split on \n.
  str = tostring(str):gsub("\r\n", "\n"):gsub("\r", "\n")
  return vim.split(str, "\n", { plain = true })
end

----------------------------------------------------------------------
-- Commands / public actions
----------------------------------------------------------------------

--- Read a URL through AgentSpan and open the extracted body in a scratch buffer.
--- @param url string
function M.read(url)
  if not url or url == "" then
    vim.notify("AgentSpan: usage :AgentSpanRead <url>", vim.log.levels.WARN)
    return
  end

  local path = "/api/v1/read?url=" .. api.url_encode(url)
  vim.notify("AgentSpan: reading " .. url .. " ...", vim.log.levels.INFO)

  api.get(path, function(err, data)
    if err then
      vim.notify("AgentSpan read failed: " .. err, vim.log.levels.ERROR)
      return
    end

    local content = data and data.content or {}
    local title = content.title or url
    local body = content.body or ""

    local lines = {}
    vim.list_extend(lines, { "# " .. title, "", "<" .. url .. ">", "", "---", "" })
    vim.list_extend(lines, to_lines(body))

    open_scratch("AgentSpan: " .. title, lines, "markdown")
  end)
end

--- Render a list of search results as markdown lines.
--- @param results table[] each { title, url, snippet, channels? }
--- @return string[]
local function results_to_lines(results)
  local lines = {}
  for i, r in ipairs(results) do
    local title = r.title or r.url or ("result " .. i)
    table.insert(lines, string.format("## %d. %s", i, title))
    if r.url then
      table.insert(lines, "<" .. r.url .. ">")
    end
    if r.channels and #r.channels > 0 then
      table.insert(lines, "_channels: " .. table.concat(r.channels, ", ") .. "_")
    end
    if r.snippet and r.snippet ~= "" then
      table.insert(lines, "")
      vim.list_extend(lines, to_lines(r.snippet))
    end
    table.insert(lines, "")
  end
  if #lines == 0 then
    lines = { "_No results._" }
  end
  return lines
end

--- Offer results via vim.ui.select; opening the chosen URL with M.read.
--- Falls back silently if vim.ui.select is unavailable.
--- @param results table[]
--- @return boolean handled
local function offer_select(results)
  if type(vim.ui) ~= "table" or type(vim.ui.select) ~= "function" then
    return false
  end
  vim.ui.select(results, {
    prompt = "AgentSpan results (select to read):",
    format_item = function(r)
      return (r.title or r.url or "result") .. (r.url and ("  -  " .. r.url) or "")
    end,
  }, function(choice)
    if choice and choice.url then
      M.read(choice.url)
    end
  end)
  return true
end

--- Search a single channel.
--- @param channel string channel name (e.g. "duckduckgo")
--- @param query string
--- @param opts table|nil { limit = 10, select = true }
function M.search(channel, query, opts)
  opts = opts or {}
  if not channel or channel == "" or not query or query == "" then
    vim.notify("AgentSpan: usage :AgentSpanSearch <channel> <query>", vim.log.levels.WARN)
    return
  end

  local limit = opts.limit or 10
  local path = string.format(
    "/api/v1/channels/%s/search?q=%s&limit=%d",
    api.url_encode(channel),
    api.url_encode(query),
    limit
  )

  vim.notify(string.format("AgentSpan: searching %s for %q ...", channel, query), vim.log.levels.INFO)

  api.get(path, function(err, data)
    if err then
      vim.notify("AgentSpan search failed: " .. err, vim.log.levels.ERROR)
      return
    end

    local results = (data and data.results) or {}

    local header = {
      string.format("# AgentSpan search: %s", channel),
      string.format("_query: %s — %d result(s)_", query, #results),
      "",
    }
    local lines = vim.list_extend(header, results_to_lines(results))
    open_scratch(string.format("AgentSpan search: %s", channel), lines, "markdown")

    -- Also offer a quick "select to read" if the UI supports it.
    if opts.select ~= false and #results > 0 then
      offer_select(results)
    end
  end)
end

--- Federated search across all channels.
--- @param query string
--- @param opts table|nil { limit = 10, select = true }
function M.federated(query, opts)
  opts = opts or {}
  if not query or query == "" then
    vim.notify("AgentSpan: usage :AgentSpanFederated <query>", vim.log.levels.WARN)
    return
  end

  local limit = opts.limit or 10
  vim.notify(string.format("AgentSpan: federated search for %q ...", query), vim.log.levels.INFO)

  api.post("/api/v1/search/federated", { query = query, limit = limit }, function(err, data)
    if err then
      vim.notify("AgentSpan federated search failed: " .. err, vim.log.levels.ERROR)
      return
    end

    local results = (data and data.results) or {}

    local header = {
      "# AgentSpan federated search",
      string.format("_query: %s — %d result(s)_", query, #results),
      "",
    }
    local lines = vim.list_extend(header, results_to_lines(results))
    open_scratch("AgentSpan federated search", lines, "markdown")

    if opts.select ~= false and #results > 0 then
      offer_select(results)
    end
  end)
end

--- List available channels in a scratch buffer.
function M.channels()
  api.get("/api/v1/channels", function(err, data)
    if err then
      vim.notify("AgentSpan channels failed: " .. err, vim.log.levels.ERROR)
      return
    end

    local channels = (data and data.channels) or {}
    local lines = { "# AgentSpan channels", "" }
    for _, c in ipairs(channels) do
      local name = c.name or "?"
      local tier = c.tier and (" `[" .. c.tier .. "]`") or ""
      table.insert(lines, string.format("- **%s**%s — %s", name, tier, c.description or ""))
    end
    if #channels == 0 then
      table.insert(lines, "_No channels reported._")
    end
    open_scratch("AgentSpan channels", lines, "markdown")
  end)
end

----------------------------------------------------------------------
-- Health / statusline helper
----------------------------------------------------------------------

--- Synchronous-ish health check for statuslines.
--- Returns the last known status immediately and refreshes in the background.
--- Designed to be cheap to call on every statusline redraw.
---
--- @return string status one of "" | "AgentSpan: ok" | "AgentSpan: down"
local _health_state = {
  text = "",
  checking = false,
  last_check = 0,
}

--- Trigger an async refresh of the cached health state.
--- @param min_interval number|nil seconds between live checks (default 5)
local function refresh_health(min_interval)
  min_interval = min_interval or 5
  local now = vim.loop and vim.loop.now() / 1000 or os.time()
  if _health_state.checking then
    return
  end
  if (now - _health_state.last_check) < min_interval then
    return
  end
  _health_state.checking = true
  _health_state.last_check = now

  api.get("/health", function(err, _)
    _health_state.checking = false
    if err then
      _health_state.text = "AgentSpan: down"
    else
      _health_state.text = "AgentSpan: ok"
    end
    -- Ask statuslines to redraw with the fresh value.
    pcall(vim.cmd, "redrawstatus")
  end)
end

--- Statusline component. Returns cached health string and refreshes in the
--- background. Safe to embed directly in 'statusline' via %{...}.
--- @return string
function M.health()
  refresh_health()
  return _health_state.text
end

return M
