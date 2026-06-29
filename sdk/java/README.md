# AgentSpan Java SDK

Official Java SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```xml
<dependency>
  <groupId>io.agentspan</groupId>
  <artifactId>agentspan-sdk</artifactId>
  <version>0.4.0</version>
</dependency>
```

```java
import io.agentspan.AgentSpanClient;

var client = new AgentSpanClient("http://localhost:8080", "as_...");
var content = client.read("https://example.com", false);
System.out.println(content.get("body").asText());

var results = client.search("hackernews", "rust", 10);
```

Built on `java.net.http.HttpClient` + Jackson (Java 17+). See
[docs/api-reference.md](../../docs/api-reference.md). Tests: `mvn test`
(JUnit 5 + OkHttp MockWebServer).
