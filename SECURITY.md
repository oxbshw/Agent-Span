# Security Policy

## Supported versions

AgentSpan is pre-1.0. Security fixes land on the latest `0.x` release line.

| Version | Supported |
| ------- | --------- |
| 0.5.x   | ✅        |
| < 0.5   | ❌        |

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Instead, report privately via GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)
on this repository, or email the maintainers.

Include:

- A description of the issue and its impact
- Steps to reproduce (proof-of-concept if possible)
- Affected version(s) and configuration

We aim to acknowledge reports within 72 hours and to ship a fix or mitigation
as quickly as severity warrants.

## Handling of secrets

AgentSpan treats API keys, bot tokens, and browser cookies as secrets:

- They are read from environment variables or `~/.agentspan/config.yaml`.
- API keys are stored hashed (SHA-256), never in plaintext.
- The `/api/v1/config` endpoint returns a **non-secret** view only.
- Never paste secrets into issues, PRs, or logs.
