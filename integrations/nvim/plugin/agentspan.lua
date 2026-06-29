-- plugin/agentspan.lua
-- User command registration for the AgentSpan plugin.
--
-- This file is sourced automatically by Neovim on startup. It only registers
-- the :AgentSpan* commands; all real work is deferred to the lua module so the
-- plugin stays lazy-friendly and cheap to load.

-- Guard against double-loading.
if vim.g.loaded_agentspan then
  return
end
vim.g.loaded_agentspan = true

-- Require Neovim (uses vim.api / vim.json / vim.system).
if vim.fn.has("nvim-0.7") == 0 then
  vim.notify("agentspan.nvim requires Neovim 0.7+", vim.log.levels.ERROR)
  return
end

local function agentspan()
  return require("agentspan")
end

-- :AgentSpanRead <url>
vim.api.nvim_create_user_command("AgentSpanRead", function(opts)
  agentspan().read(vim.trim(opts.args))
end, {
  nargs = 1,
  desc = "AgentSpan: read & extract a URL into a scratch buffer",
})

-- :AgentSpanSearch <channel> <query...>
vim.api.nvim_create_user_command("AgentSpanSearch", function(opts)
  -- First whitespace-delimited token is the channel; the rest is the query.
  local args = vim.trim(opts.args)
  local channel, query = args:match("^(%S+)%s+(.+)$")
  if not channel then
    vim.notify("AgentSpan: usage :AgentSpanSearch <channel> <query>", vim.log.levels.WARN)
    return
  end
  agentspan().search(channel, query)
end, {
  nargs = "+",
  desc = "AgentSpan: search a single channel (<channel> <query>)",
})

-- :AgentSpanFederated <query...>
vim.api.nvim_create_user_command("AgentSpanFederated", function(opts)
  agentspan().federated(vim.trim(opts.args))
end, {
  nargs = "+",
  desc = "AgentSpan: federated search across all channels",
})

-- :AgentSpanChannels
vim.api.nvim_create_user_command("AgentSpanChannels", function()
  agentspan().channels()
end, {
  nargs = 0,
  desc = "AgentSpan: list available channels",
})

-- :AgentSpanHealth
vim.api.nvim_create_user_command("AgentSpanHealth", function()
  -- health() returns a cached value and refreshes async; notify the live result.
  require("agentspan.api").get("/health", function(err, _)
    if err then
      vim.notify("AgentSpan: server unreachable (" .. err .. ")", vim.log.levels.WARN)
    else
      vim.notify("AgentSpan: server ok", vim.log.levels.INFO)
    end
  end)
end, {
  nargs = 0,
  desc = "AgentSpan: ping the server /health endpoint",
})
