## What

Briefly describe the change.

## Why

The motivation / issue this addresses. Link issues with `Closes #123`.

## How

Key implementation notes. For a new channel, confirm it follows the template
(backend with `with_base_url`, `Channel` impl, `check_health`, `format_for_llm`).

## Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] `cargo fmt --all --check` is clean
- [ ] `pytest` passes (if the Python SDK changed)
- [ ] Added/updated tests for the change
- [ ] Updated `CHANGELOG.md`
