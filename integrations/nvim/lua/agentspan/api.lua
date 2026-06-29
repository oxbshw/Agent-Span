-- agentspan.api
-- Thin HTTP helpers around `curl`, used by the AgentSpan Neovim plugin.
--
-- These helpers build a curl argv (list form, no shell quoting headaches),
-- run it asynchronously via vim.system when available, and otherwise fall back
-- to the synchronous vim.fn.system. Both paths invoke the caller's callback
-- with `(err, decoded_json)`.

local M = {}

-- Module-level config, populated by init.setup().
-- Kept here so api.get/post can be called without passing config every time.
M.config = {
  server_url = "http://localhost:8080",
  api_key = nil,
  timeout = 30, -- seconds, passed to curl --max-time
}

--- Merge user options into the module config.
--- @param opts table|nil
function M.configure(opts)
  M.config = vim.tbl_deep_extend("force", M.config, opts or {})
  -- Normalise: strip a single trailing slash so URL joins are predictable.
  if type(M.config.server_url) == "string" then
    M.config.server_url = M.config.server_url:gsub("/+$", "")
  end
  return M.config
end

--- URL-encode a string for use in a query parameter.
--- @param str string
--- @return string
function M.url_encode(str)
  if str == nil then
    return ""
  end
  str = tostring(str)
  -- Encode everything that is not an unreserved character.
  str = str:gsub("([^%w%-_%.~])", function(c)
    return string.format("%%%02X", string.byte(c))
  end)
  return str
end

--- Build a full URL from a path (already including any query string).
--- @param path string e.g. "/api/v1/read?url=..."
--- @return string
function M.build_url(path)
  if path:match("^https?://") then
    return path
  end
  if path:sub(1, 1) ~= "/" then
    path = "/" .. path
  end
  return M.config.server_url .. path
end

--- Common curl args (headers, timeout, fail-on-error handling).
--- We deliberately do NOT pass -f so that we can surface the server's JSON
--- error body to the user instead of an opaque curl exit code.
--- @return table argv list (without the URL or method-specific bits)
local function base_curl_args()
  local args = {
    "curl",
    "-sS", -- silent but still show errors
    "--max-time",
    tostring(M.config.timeout or 30),
    "-H",
    "Accept: application/json",
  }
  if M.config.api_key and M.config.api_key ~= "" then
    table.insert(args, "-H")
    table.insert(args, "X-API-Key: " .. M.config.api_key)
  end
  return args
end

--- Decode a curl stdout buffer into a Lua table.
--- @param stdout string
--- @return any|nil decoded, string|nil err
local function decode_response(stdout)
  if stdout == nil or stdout == "" then
    return nil, "empty response from server"
  end
  local ok, decoded = pcall(vim.json.decode, stdout)
  if not ok then
    -- Trim, then truncate noisy HTML/error pages for the notification.
    local preview = vim.trim(stdout):sub(1, 200)
    return nil, "failed to parse JSON response: " .. preview
  end
  return decoded, nil
end

--- Run a built curl argv, calling cb(err, decoded) on the main loop.
--- @param args table curl argv
--- @param cb fun(err: string|nil, decoded: any|nil)
local function run(args, cb)
  -- Always hand results back on the main event loop so callers can touch buffers.
  local function finish(err, decoded)
    vim.schedule(function()
      cb(err, decoded)
    end)
  end

  if vim.system then
    -- Neovim 0.10+: async, no shell involved.
    vim.system(args, { text = true }, function(obj)
      if obj.code ~= 0 then
        local msg = obj.stderr and vim.trim(obj.stderr) or ""
        if msg == "" then
          msg = "curl exited with code " .. tostring(obj.code)
        end
        -- The server may still have returned a JSON error body on stdout.
        local decoded = nil
        if obj.stdout and obj.stdout ~= "" then
          decoded = select(1, decode_response(obj.stdout))
        end
        if decoded and type(decoded) == "table" and decoded.error then
          finish(tostring(decoded.error), nil)
        else
          finish(msg, nil)
        end
        return
      end
      local decoded, derr = decode_response(obj.stdout)
      finish(derr, decoded)
    end)
    return
  end

  -- Fallback for older Neovim: synchronous vim.fn.system.
  -- Shell-quote each argument so paths/queries with spaces survive.
  local cmd = table.concat(
    vim.tbl_map(function(a)
      return vim.fn.shellescape(a)
    end, args),
    " "
  )
  local stdout = vim.fn.system(cmd)
  if vim.v.shell_error ~= 0 then
    local decoded = select(1, decode_response(stdout))
    if decoded and type(decoded) == "table" and decoded.error then
      finish(tostring(decoded.error), nil)
    else
      local msg = vim.trim(stdout)
      if msg == "" then
        msg = "curl exited with code " .. tostring(vim.v.shell_error)
      end
      finish(msg, nil)
    end
    return
  end
  local decoded, derr = decode_response(stdout)
  finish(derr, decoded)
end

--- HTTP GET.
--- @param path string path (with query string) or absolute URL
--- @param cb fun(err: string|nil, decoded: any|nil)
function M.get(path, cb)
  local args = base_curl_args()
  table.insert(args, M.build_url(path))
  run(args, cb)
end

--- HTTP POST with a JSON body.
--- @param path string path or absolute URL
--- @param body table will be JSON-encoded
--- @param cb fun(err: string|nil, decoded: any|nil)
function M.post(path, body, cb)
  local args = base_curl_args()
  table.insert(args, "-X")
  table.insert(args, "POST")
  table.insert(args, "-H")
  table.insert(args, "Content-Type: application/json")
  table.insert(args, "--data")
  table.insert(args, vim.json.encode(body or {}))
  table.insert(args, M.build_url(path))
  run(args, cb)
end

return M
