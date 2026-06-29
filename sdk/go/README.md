# agentspan-go

Official Go SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```bash
go get github.com/oxbshw/Agent-Span-go
```

```go
package main

import (
	"context"
	"fmt"
	agentspan "github.com/oxbshw/Agent-Span-go"
)

func main() {
	client := agentspan.New("http://localhost:8080").WithAPIKey("as_...")
	content, err := client.Read(context.Background(), "https://example.com", false)
	if err != nil {
		panic(err)
	}
	fmt.Println(content.Body)
}
```

Standard library only (`net/http`). See [docs/api-reference.md](../../docs/api-reference.md).
Tests: `go test ./...` (uses `net/http/httptest`, no external deps).
