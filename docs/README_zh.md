<h1 align="center">🛰️ AgentSpan</h1>

<p align="center"><strong>面向 AI Agent 的网络访问网关 —— 多平台、多租户、极速。</strong></p>

<p align="center"><a href="../README.md">English</a> · 中文 · <a href="README_ja.md">日本語</a> · <a href="README_ko.md">한국어</a></p>

AgentSpan 用一套异步 Rust 内核，为 AI Agent 提供对 **24 个互联网平台** 的持久、可扩展、
**带缓存** 的访问：统一 REST API、SSE 事件流、原生 **MCP 服务（36 个工具）**、
**9 种语言 SDK**、命令行工具，以及 React 仪表盘。

## 为什么选 AgentSpan？

[Agent Reach](https://github.com/Panniantong/Agent-Reach) 验证了「能力层」模型
（安装 → 体检 → 路由）。AgentSpan 把它升级成真正的 **网关** —— 由网关自己完成读取，
全部隐藏在一个 API 之后。

| 能力 | Agent Reach | AgentSpan |
|---|:---:|:---:|
| 架构 | Python 安装器 | **异步 Rust 网关** |
| 渠道 | 13 | **24** |
| 缓存 | ❌ | ✅ 三级缓存 |
| REST API | ❌ | ✅ + SSE |
| MCP 工具 | 1 | **36** |
| SDK | 0 | **9 种语言** |
| 多租户 / RBAC / 审计 | ❌ | ✅ |
| Web 仪表盘 | ❌ | ✅ |

## 快速上手

```bash
cargo run --bin agentspan -- serve
curl "localhost:8080/api/v1/read?url=https://example.com"
curl "localhost:8080/api/v1/channels/hackernews/search?q=rust"
```

完整文档见 [English README](../README.md) 与 [API 参考](api-reference.md)。

## 许可证

[MIT](../LICENSE)
