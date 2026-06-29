using System.Net;
using System.Text;
using AgentSpan;
using Xunit;

namespace AgentSpan.Tests;

/// <summary>Stub handler returning canned responses (no network).</summary>
file sealed class StubHandler(Func<HttpRequestMessage, HttpResponseMessage> responder) : HttpMessageHandler
{
    public HttpRequestMessage? Last { get; private set; }

    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
    {
        Last = request;
        return Task.FromResult(responder(request));
    }
}

public class ClientTests
{
    private static AgentSpanClient Client(Func<HttpRequestMessage, HttpResponseMessage> responder, out StubHandler handler)
    {
        handler = new StubHandler(responder);
        return new AgentSpanClient("http://test", "k", new HttpClient(handler));
    }

    private static HttpResponseMessage Json(int status, string body, params (string, string)[] headers)
    {
        var res = new HttpResponseMessage((HttpStatusCode)status)
        {
            Content = new StringContent(body, Encoding.UTF8, "application/json"),
        };
        foreach (var (k, v) in headers)
            res.Headers.TryAddWithoutValidation(k, v);
        return res;
    }

    [Fact]
    public async Task ReadReturnsContent()
    {
        var client = Client(_ => Json(200, "{\"channel\":\"web\",\"content\":{\"body\":\"hi\"}}"), out _);
        var content = await client.ReadAsync("https://x");
        Assert.Equal("hi", content["body"]!.GetValue<string>());
    }

    [Fact]
    public async Task ReadChannelError()
    {
        var client = Client(_ => Json(200, "{\"error\":\"no channel\"}"), out _);
        await Assert.ThrowsAsync<ChannelException>(() => client.ReadAsync("ftp://x"));
    }

    [Fact]
    public async Task SearchMapsResults()
    {
        var client = Client(_ => Json(200, "{\"results\":[{\"title\":\"Rust\"}]}"), out _);
        var results = await client.SearchAsync("hackernews", "rust", 7);
        Assert.Equal("Rust", results[0]!["title"]!.GetValue<string>());
    }

    [Fact]
    public async Task AuthError()
    {
        var client = Client(_ => Json(401, "{\"error\":\"bad\"}"), out _);
        await Assert.ThrowsAsync<AuthenticationException>(() => client.ListChannelsAsync());
    }

    [Fact]
    public async Task RateLimitCarriesRetryAfter()
    {
        var client = Client(_ => Json(429, "{\"error\":\"slow\"}", ("Retry-After", "12")), out _);
        var ex = await Assert.ThrowsAsync<RateLimitException>(() => client.ReadAsync("https://x"));
        Assert.Equal(12, ex.RetryAfter);
    }

    [Fact]
    public async Task BatchRead()
    {
        var client = Client(_ => Json(200, "{\"count\":2,\"results\":[{\"ok\":true},{\"ok\":false}]}"), out _);
        var results = await client.BatchReadAsync(new[] { "a", "b" });
        Assert.Equal(2, results.AsArray().Count);
    }

    [Fact]
    public async Task SendsApiKey()
    {
        var client = Client(_ => Json(200, "{\"channels\":[]}"), out var handler);
        await client.ListChannelsAsync();
        Assert.True(handler.Last!.Headers.Contains("X-API-Key"));
    }
}
