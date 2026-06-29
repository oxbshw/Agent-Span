<h1 align="center">🛰️ AgentSpan</h1>

<p align="center"><strong>AI エージェントのための Web アクセスゲートウェイ — マルチプラットフォーム、マルチテナント、超高速。</strong></p>

<p align="center"><a href="../README.md">English</a> · <a href="README_zh.md">中文</a> · 日本語 · <a href="README_ko.md">한국어</a></p>

AgentSpan は非同期 Rust コアの上で、AI エージェントに **24 のインターネット
プラットフォーム** への永続的・スケーラブル・**キャッシュ付き** のアクセスを提供します:
統一 REST API、SSE イベントストリーム、ネイティブ **MCP サーバー（36 ツール）**、
**9 言語の SDK**、CLI、React ダッシュボード。

## なぜ AgentSpan か

[Agent Reach](https://github.com/Panniantong/Agent-Reach) は「ケイパビリティ層」
（インストール → 診断 → ルーティング）モデルを実証しました。AgentSpan はそれを
本物の **ゲートウェイ** に進化させ、読み取り自体を 1 つの API の背後で行います。

| 機能 | Agent Reach | AgentSpan |
|---|:---:|:---:|
| アーキテクチャ | Python インストーラ | **非同期 Rust ゲートウェイ** |
| チャネル | 13 | **24** |
| キャッシュ | ❌ | ✅ 3 層 |
| REST API | ❌ | ✅ + SSE |
| MCP ツール | 1 | **36** |
| SDK | 0 | **9 言語** |
| マルチテナント / RBAC / 監査 | ❌ | ✅ |

## クイックスタート

```bash
cargo run --bin agentspan -- serve
curl "localhost:8080/api/v1/read?url=https://example.com"
curl "localhost:8080/api/v1/channels/hackernews/search?q=rust"
```

詳細は [English README](../README.md) と [API リファレンス](api-reference.md) を参照。

## ライセンス

[MIT](../LICENSE)
