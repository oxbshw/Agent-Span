# AgentSpan.Sdk (C# / .NET)

Official C# SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```bash
dotnet add package AgentSpan.Sdk
```

```csharp
using AgentSpan;

var client = new AgentSpanClient("http://localhost:8080", "as_...");
var content = await client.ReadAsync("https://example.com");
Console.WriteLine(content["body"]!.GetValue<string>());

var results = await client.SearchAsync("hackernews", "rust", 10);
var batch = await client.BatchReadAsync(new[] { "https://a", "https://b" });
```

Built on `HttpClient` + `System.Text.Json` (.NET 8). See
[docs/api-reference.md](../../docs/api-reference.md). Tests: `dotnet test`
(xUnit + stub `HttpMessageHandler`).
