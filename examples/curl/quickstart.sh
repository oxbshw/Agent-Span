#!/bin/bash
# AgentSpan curl examples — no SDK needed.
#
# Prereq: agentspan serve --port 8080
# Run:    bash examples/curl/quickstart.sh

BASE="http://localhost:8080"

echo "=== Health ==="
curl -s "$BASE/health" | jq .

echo -e "\n=== List channels ==="
curl -s "$BASE/api/v1/channels" | jq '.channels | length'

echo -e "\n=== Read a URL (smart — auto-detects channel) ==="
curl -s "$BASE/api/v1/read?url=https://example.com" | jq '{channel, title: .content.title, cached: .content.cached}'

echo -e "\n=== Search Hacker News ==="
curl -s "$BASE/api/v1/channels/hackernews/search?q=rust&limit=3" | jq '.results[] | {title, url}'

echo -e "\n=== Doctor (health check all channels) ==="
curl -s "$BASE/api/v1/doctor" | jq '{ok, total}'

echo -e "\n=== Federated search ==="
curl -s -X POST "$BASE/api/v1/search/federated" \
  -H 'content-type: application/json' \
  -d '{"query":"rust async","channels":["hackernews","lobsters"],"limit":5}' | jq '{query, searched, result_count: (.results | length)}'

echo -e "\n=== OpenAPI spec ==="
curl -s "$BASE/openapi.json" | jq '.info.title, .info.version'

echo -e "\n=== Stats ==="
curl -s "$BASE/api/v1/stats" | jq '{channels, audit_entries}'
