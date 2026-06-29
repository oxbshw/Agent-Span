#!/usr/bin/env sh
#
# check-links.sh — verify that external http(s) links in Markdown files are reachable.
#
# Usage:
#   check-links.sh [FILE.md ...]
#
# For each Markdown file given as an argument, this script extracts http/https URLs
# and checks each one with `curl`. Any URL that cannot be reached is reported, and
# the script exits non-zero if at least one link is broken.
#
# Dependencies: POSIX sh, curl, grep, sed (all commonly available). No jq, no node.
#
# Behaviour notes:
#   - Localhost / loopback and documentation example domains are skipped (they are
#     not expected to be reachable from CI).
#   - Each unique URL is checked only once.
#   - Exit code 0 = all checked links OK (or nothing to check); 1 = one or more broken.

set -eu

# ----------------------------------------------------------------------------
# Config (overridable via environment).
# ----------------------------------------------------------------------------
# Max seconds to wait per request.
: "${LINK_CHECK_TIMEOUT:=15}"
# Number of retries per URL before declaring it broken.
: "${LINK_CHECK_RETRIES:=1}"
# User-Agent: some hosts reject the default curl UA.
: "${LINK_CHECK_UA:=agentspan-check-links/1.0 (+https://github.com/)}"

# ----------------------------------------------------------------------------
# No-files case: nothing passed in -> nothing to do, succeed quietly.
# ----------------------------------------------------------------------------
if [ "$#" -eq 0 ]; then
  echo "check-links: no files provided; nothing to check."
  exit 0
fi

# Ensure curl is available; without it we cannot do our job.
if ! command -v curl >/dev/null 2>&1; then
  echo "check-links: error: 'curl' is required but was not found in PATH." >&2
  exit 2
fi

# ----------------------------------------------------------------------------
# Decide whether a URL should be skipped (not expected to be reachable).
# Returns 0 (true) when the URL should be skipped.
# ----------------------------------------------------------------------------
should_skip() {
  url="$1"
  case "$url" in
    *://localhost|*://localhost/*|*://localhost:*) return 0 ;;
    *://127.0.0.1|*://127.0.0.1/*|*://127.0.0.1:*) return 0 ;;
    *://0.0.0.0|*://0.0.0.0/*|*://0.0.0.0:*) return 0 ;;
    *://\[::1\]|*://\[::1\]/*|*://\[::1\]:*) return 0 ;;
    *://example.com|*://example.com/*) return 0 ;;
    *://*.example.com|*://*.example.com/*) return 0 ;;
    *://example.org|*://example.org/*) return 0 ;;
    *://*.example.org|*://*.example.org/*) return 0 ;;
    *://example.net|*://example.net/*) return 0 ;;
    *://*.example.net|*://*.example.net/*) return 0 ;;
  esac
  return 1
}

# ----------------------------------------------------------------------------
# Check a single URL. Returns 0 if reachable, 1 otherwise.
# Tries a lightweight HEAD first, then falls back to a GET (range-limited),
# since some servers do not support or mishandle HEAD.
# ----------------------------------------------------------------------------
check_url() {
  url="$1"

  # HEAD attempt.
  if curl --fail --silent --show-error --location --head \
      --retry "$LINK_CHECK_RETRIES" \
      --max-time "$LINK_CHECK_TIMEOUT" \
      --user-agent "$LINK_CHECK_UA" \
      --output /dev/null "$url" >/dev/null 2>&1; then
    return 0
  fi

  # GET fallback (request only the first byte where supported).
  if curl --fail --silent --show-error --location \
      --retry "$LINK_CHECK_RETRIES" \
      --max-time "$LINK_CHECK_TIMEOUT" \
      --user-agent "$LINK_CHECK_UA" \
      --range 0-0 \
      --output /dev/null "$url" >/dev/null 2>&1; then
    return 0
  fi

  return 1
}

# ----------------------------------------------------------------------------
# Extract candidate URLs from a file.
#   - grep -oE pulls http(s) tokens.
#   - sed trims common trailing punctuation that is usually Markdown syntax,
#     not part of the URL: ) ] } > " ' , . ; and trailing whitespace.
# ----------------------------------------------------------------------------
extract_urls() {
  file="$1"
  # The character class for the URL body intentionally excludes whitespace,
  # closing brackets/parens/quotes/angle-brackets and backticks.
  grep -oE 'https?://[^[:space:]<>")'"'"'`\]]+' "$file" 2>/dev/null \
    | sed -E 's/[.,;:!?)"'"'"'>\]\}]+$//' \
    || true
}

# ----------------------------------------------------------------------------
# Collect all unique URLs across all input files.
# Using a temp file keeps us POSIX (no associative arrays).
# ----------------------------------------------------------------------------
tmp_urls="$(mktemp 2>/dev/null || echo "${TMPDIR:-/tmp}/agentspan_links.$$")"
# shellcheck disable=SC2064
trap "rm -f \"$tmp_urls\"" EXIT INT TERM

for file in "$@"; do
  if [ ! -f "$file" ]; then
    echo "check-links: warning: skipping non-existent file '$file'." >&2
    continue
  fi
  extract_urls "$file" >> "$tmp_urls"
done

# Deduplicate while preserving stability via sort -u.
if [ ! -s "$tmp_urls" ]; then
  echo "check-links: no http(s) links found; nothing to check."
  exit 0
fi
sort -u "$tmp_urls" -o "$tmp_urls"

# ----------------------------------------------------------------------------
# Check each URL.
# ----------------------------------------------------------------------------
broken=0
checked=0
skipped=0

while IFS= read -r url; do
  [ -n "$url" ] || continue

  if should_skip "$url"; then
    echo "  skip   $url"
    skipped=$((skipped + 1))
    continue
  fi

  checked=$((checked + 1))
  if check_url "$url"; then
    echo "  ok     $url"
  else
    echo "  BROKEN $url" >&2
    broken=$((broken + 1))
  fi
done < "$tmp_urls"

echo ""
echo "check-links: checked=$checked skipped=$skipped broken=$broken"

if [ "$broken" -gt 0 ]; then
  echo "check-links: $broken broken link(s) found." >&2
  exit 1
fi

exit 0
