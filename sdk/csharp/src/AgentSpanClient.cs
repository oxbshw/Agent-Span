using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace AgentSpan;

/// <summary>Base class for all AgentSpan SDK errors.</summary>
public class AgentSpanException : Exception
{
    public AgentSpanException(string message) : base(message) { }
}

/// <summary>Thrown on HTTP 401.</summary>
public sealed class AuthenticationException(string message) : AgentSpanException(message);

/// <summary>Thrown on HTTP 429.</summary>
public sealed class RateLimitException(string message, int? retryAfter) : AgentSpanException(message)
{
    public int? RetryAfter { get; } = retryAfter;
}

/// <summary>Thrown for other non-2xx responses.</summary>
public sealed class ApiException(int status, string message) : AgentSpanException(message)
{
    public int Status { get; } = status;
}

/// <summary>Thrown when the server embeds {"error":...} in a 200 body.</summary>
public sealed class ChannelException(string message) : AgentSpanException(message);

/// <summary>Async client for the AgentSpan gateway.</summary>
public sealed class AgentSpanClient
{
    private readonly string _baseUrl;
    private readonly string? _apiKey;
    private readonly HttpClient _http;

    public AgentSpanClient(string baseUrl = "http://localhost:8080", string? apiKey = null, HttpClient? http = null)
    {
        _baseUrl = baseUrl.TrimEnd('/');
        _apiKey = apiKey;
        _http = http ?? new HttpClient();
    }

    private HttpRequestMessage Build(HttpMethod method, string path, object? body = null)
    {
        var req = new HttpRequestMessage(method, _baseUrl + path);
        if (_apiKey is not null)
            req.Headers.Add("X-API-Key", _apiKey);
        if (body is not null)
            req.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
        return req;
    }

    private async Task<JsonNode> SendAsync(HttpRequestMessage req)
    {
        var res = await _http.SendAsync(req).ConfigureAwait(false);
        var text = await res.Content.ReadAsStringAsync().ConfigureAwait(false);
        var node = string.IsNullOrEmpty(text) ? new JsonObject() : JsonNode.Parse(text)!;
        if ((int)res.StatusCode >= 400)
            throw Classify((int)res.StatusCode, node, res);
        return node;
    }

    private static AgentSpanException Classify(int status, JsonNode node, HttpResponseMessage res)
    {
        var message = node["error"]?.GetValue<string>() ?? $"HTTP {status}";
        return status switch
        {
            401 => new AuthenticationException(message),
            429 => new RateLimitException(
                message,
                res.Headers.TryGetValues("Retry-After", out var v) && int.TryParse(v.FirstOrDefault(), out var n)
                    ? n
                    : null),
            _ => new ApiException(status, message),
        };
    }

    private static string Enc(string s) => Uri.EscapeDataString(s);

    /// <summary>Read a URL via the best matching channel.</summary>
    public async Task<JsonNode> ReadAsync(string url, bool forceRefresh = false)
    {
        var data = await SendAsync(Build(HttpMethod.Get,
            $"/api/v1/read?url={Enc(url)}&force_refresh={forceRefresh.ToString().ToLowerInvariant()}"));
        if (data["error"] is not null)
            throw new ChannelException(data["error"]!.GetValue<string>());
        return data["content"]!;
    }

    /// <summary>Search a platform via a named channel.</summary>
    public async Task<JsonNode> SearchAsync(string channel, string query, int limit = 10)
    {
        var data = await SendAsync(Build(HttpMethod.Get,
            $"/api/v1/channels/{Enc(channel)}/search?q={Enc(query)}&limit={limit}"));
        if (data["error"] is not null)
            throw new ChannelException(data["error"]!.GetValue<string>());
        return data["results"]!;
    }

    /// <summary>List available channels.</summary>
    public async Task<JsonNode> ListChannelsAsync() =>
        (await SendAsync(Build(HttpMethod.Get, "/api/v1/channels")))["channels"]!;

    /// <summary>Run health diagnostics across all channels.</summary>
    public async Task<JsonNode> DoctorAsync() =>
        await SendAsync(Build(HttpMethod.Get, "/api/v1/doctor"));

    /// <summary>Fetch the non-secret configuration view.</summary>
    public async Task<JsonNode> GetConfigAsync() =>
        await SendAsync(Build(HttpMethod.Get, "/api/v1/config"));

    /// <summary>Read many URLs in parallel (server-side batch).</summary>
    public async Task<JsonNode> BatchReadAsync(IEnumerable<string> urls, bool forceRefresh = false) =>
        (await SendAsync(Build(HttpMethod.Post, "/api/v1/batch/read",
            new { urls, force_refresh = forceRefresh })))["results"]!;

    /// <summary>Run many queries against one channel in parallel.</summary>
    public async Task<JsonNode> BatchSearchAsync(string channel, IEnumerable<string> queries, int limit = 10) =>
        (await SendAsync(Build(HttpMethod.Post, "/api/v1/batch/search",
            new { channel, queries, limit })))["results"]!;

    /// <summary>Return true when the server's /health endpoint is OK.</summary>
    public async Task<bool> HealthAsync()
    {
        try
        {
            var res = await _http.SendAsync(Build(HttpMethod.Get, "/health"));
            return res.StatusCode == HttpStatusCode.OK;
        }
        catch
        {
            return false;
        }
    }
}
