# agentspan-read (GitHub Action)

A composite GitHub Action that reads and extracts the content of a web page using an
[AgentSpan](https://github.com/) gateway. It calls `GET /api/v1/read?url=<url>` and exposes
the extracted title and body as step outputs (and, optionally, writes the body to a file).

It is implemented in pure bash + `curl` + `jq` (both preinstalled on GitHub-hosted runners),
so there is **no Node build step**.

## Inputs

| Input         | Required | Default                  | Description                                                        |
| ------------- | -------- | ------------------------ | ------------------------------------------------------------------ |
| `url`         | yes      | —                        | The URL of the page to read and extract content from.              |
| `server-url`  | no       | `http://localhost:8080`  | Base URL of the AgentSpan gateway.                                 |
| `api-key`     | no       | `""`                     | Optional API key, sent as the `X-API-Key` header.                  |
| `output-file` | no       | `""`                     | Optional path; when set, the extracted body is written to it.      |

## Outputs

| Output    | Description                          |
| --------- | ------------------------------------ |
| `content` | The extracted body text of the page. |
| `title`   | The extracted page title.            |

## Usage

```yaml
name: Summarize a page with AgentSpan
on:
  workflow_dispatch:
    inputs:
      page:
        description: "URL to read"
        required: true

jobs:
  read:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Start (or reach) your AgentSpan gateway however you like — a service
      # container, a previous step, or a remote deployment. Here we assume it is
      # reachable at the default http://localhost:8080.

      - name: Read page
        id: agentspan
        uses: your-org/agentspan/integrations/github-action@v1
        with:
          url: ${{ github.event.inputs.page }}
          # server-url: https://agentspan.internal.example.com
          # api-key: ${{ secrets.AGENTSPAN_API_KEY }}
          output-file: page.txt

      - name: Use the extracted content
        run: |
          echo "Title: ${{ steps.agentspan.outputs.title }}"
          echo "----"
          echo "${{ steps.agentspan.outputs.content }}" | head -n 20
          echo "----"
          ls -l page.txt
```

### Pointing at a self-hosted / remote gateway

```yaml
      - uses: your-org/agentspan/integrations/github-action@v1
        with:
          url: https://example.org/article
          server-url: https://agentspan.example.com
          api-key: ${{ secrets.AGENTSPAN_API_KEY }}
```

## Notes

- The action expects the AgentSpan response shape `{ "content": { "title": ..., "body": ... } }`.
- The target `url` is URL-encoded before being placed in the query string, so URLs containing
  query parameters or special characters are handled safely.
- If the gateway returns an empty body, the step succeeds but emits a workflow warning.
- A non-2xx HTTP response causes the step to fail (`curl --fail`).
