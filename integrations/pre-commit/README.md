# AgentSpan pre-commit hooks

This directory provides a [pre-commit](https://pre-commit.com/) hook that verifies the
external `http(s)` links in your Markdown documentation are reachable, so you catch broken
links before they land in `main`.

## Hooks

| Hook id                 | What it does                                                  | Files     |
| ----------------------- | ------------------------------------------------------------ | --------- |
| `agentspan-check-links` | Extracts http(s) URLs from Markdown and checks each via curl | `\.md$`   |

The hook runs [`check-links.sh`](./check-links.sh), a dependency-light POSIX shell script
that needs only `curl`, `grep`, and `sed`.

## Wiring it into a consumer repo

Add the following to the consumer repository's `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/your-org/agentspan
    rev: v0.4.0            # pin to a tag/SHA of the AgentSpan repo
    hooks:
      - id: agentspan-check-links
```

Then install and run:

```sh
pip install pre-commit          # or: pipx install pre-commit / brew install pre-commit
pre-commit install              # install the git hook
pre-commit run agentspan-check-links --all-files   # run once over everything
```

After `pre-commit install`, the hook runs automatically on every `git commit`, but only
against the staged Markdown files (pre-commit passes the changed filenames to the script).

## Behaviour

- Only `*.md` files are inspected.
- Each unique URL is checked once: a `HEAD` request first, falling back to a ranged `GET`
  for servers that mishandle `HEAD`.
- The following are **skipped** (not expected to be reachable from CI):
  - `localhost`, `127.0.0.1`, `0.0.0.0`, `[::1]`
  - `example.com`, `example.org`, `example.net` (and their subdomains)
- The hook **fails the commit** (exit code 1) if any checked link is unreachable, printing
  each broken URL.

## Configuration

The script honours these environment variables (all optional):

| Variable             | Default                | Meaning                          |
| -------------------- | ---------------------- | -------------------------------- |
| `LINK_CHECK_TIMEOUT` | `15`                   | Per-request timeout in seconds.  |
| `LINK_CHECK_RETRIES` | `1`                    | Retries per URL before failing.  |
| `LINK_CHECK_UA`      | `agentspan-check-links/1.0 ...` | User-Agent sent with requests. |

You can set these in `.pre-commit-config.yaml` indirectly by exporting them in your shell,
or run the script directly:

```sh
LINK_CHECK_TIMEOUT=30 integrations/pre-commit/check-links.sh docs/*.md
```

## Running the script standalone (without pre-commit)

```sh
# Check specific files
integrations/pre-commit/check-links.sh README.md docs/guide.md

# Check every tracked Markdown file
git ls-files '*.md' | xargs integrations/pre-commit/check-links.sh
```
