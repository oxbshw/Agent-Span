<h1 align="center">🛰️ AgentSpan</h1>

<p align="center"><strong>AI 에이전트를 위한 웹 액세스 게이트웨이 — 멀티플랫폼, 멀티테넌트, 초고속.</strong></p>

<p align="center"><a href="../README.md">English</a> · <a href="README_zh.md">中文</a> · <a href="README_ja.md">日本語</a> · 한국어</p>

AgentSpan 은 비동기 Rust 코어 위에서 AI 에이전트에게 **24 개 인터넷 플랫폼** 에 대한
지속적·확장 가능·**캐시 적용** 액세스를 제공합니다: 통합 REST API, SSE 이벤트 스트림,
네이티브 **MCP 서버(36 개 도구)**, **9 개 언어 SDK**, CLI, React 대시보드.

## 왜 AgentSpan 인가

[Agent Reach](https://github.com/Panniantong/Agent-Reach) 는 "능력 계층"(설치 → 진단 →
라우팅) 모델을 입증했습니다. AgentSpan 은 이를 진짜 **게이트웨이** 로 발전시켜, 읽기
작업 자체를 하나의 API 뒤에서 직접 수행합니다.

| 기능 | Agent Reach | AgentSpan |
|---|:---:|:---:|
| 아키텍처 | Python 설치 도구 | **비동기 Rust 게이트웨이** |
| 채널 | 13 | **24** |
| 캐시 | ❌ | ✅ 3단계 |
| REST API | ❌ | ✅ + SSE |
| MCP 도구 | 1 | **36** |
| SDK | 0 | **9 개 언어** |
| 멀티테넌트 / RBAC / 감사 | ❌ | ✅ |

## 빠른 시작

```bash
cargo run --bin agentspan -- serve
curl "localhost:8080/api/v1/read?url=https://example.com"
curl "localhost:8080/api/v1/channels/hackernews/search?q=rust"
```

자세한 내용은 [English README](../README.md) 및 [API 레퍼런스](api-reference.md) 참조.

## 라이선스

[MIT](../LICENSE)
