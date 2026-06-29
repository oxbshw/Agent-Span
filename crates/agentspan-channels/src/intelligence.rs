//! Lightweight content intelligence.
//!
//! Classifies and condenses extracted page text before it reaches an agent,
//! complementing each channel's `format_for_llm`. Everything here is deliberately
//! cheap — plain string scans, no regex or ML dependency. The aim is "good enough
//! to add structure and save tokens", not perfect NLP. Keeping it dependency-free
//! also matters on this toolchain, where every extra crate is build-time we feel.

use serde::Serialize;

/// A coarse classification of a piece of extracted content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Article,
    ForumThread,
    CodeSnippet,
    Documentation,
    Unknown,
}

/// Notable facts pulled out of content for quick scanning and metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KeyFacts {
    /// Distinct http(s) URLs found, in order of first appearance.
    pub urls: Vec<String>,
    /// ISO-8601 dates (`YYYY-MM-DD`) found, deduplicated.
    pub iso_dates: Vec<String>,
    /// Number of fenced ``` code blocks.
    pub code_block_count: usize,
}

/// Word/sentence counts and an estimated reading time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReadingStats {
    pub word_count: usize,
    pub sentence_count: usize,
    /// Estimated reading time in minutes (200 wpm, rounded up).
    pub reading_minutes: usize,
}

/// The combined analysis attached to extracted content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContentAnalysis {
    pub content_type: ContentType,
    pub key_facts: KeyFacts,
    pub reading: ReadingStats,
    /// An extractive summary (the most salient few sentences).
    pub summary: String,
}

/// Default number of sentences in the auto-attached summary.
const DEFAULT_SUMMARY_SENTENCES: usize = 3;

/// Analyze a blob of text (and optionally its source URL).
pub fn analyze(text: &str, url: Option<&str>) -> ContentAnalysis {
    ContentAnalysis {
        content_type: detect_content_type(text, url),
        key_facts: extract_key_facts(text),
        reading: reading_stats(text),
        summary: summarize(text, DEFAULT_SUMMARY_SENTENCES),
    }
}

/// Best-effort guess at what kind of content this is.
///
/// URL hints win when present (a reddit URL is a forum thread regardless of how
/// the prose reads); otherwise we fall back to text heuristics. Order matters:
/// code is checked first because a code dump can otherwise look like an article.
pub fn detect_content_type(text: &str, url: Option<&str>) -> ContentType {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return ContentType::Unknown;
    }

    if let Some(hint) = url.and_then(content_type_from_url) {
        return hint;
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let total_lines = lines.len().max(1);
    let fenced = trimmed.matches("```").count() / 2;
    // Count fence markers as code too — they're part of the block, otherwise a
    // short snippet that's mostly fences reads as less "code" than it is.
    let code_like = lines
        .iter()
        .filter(|l| l.trim_start().starts_with("```") || looks_like_code(l))
        .count();
    let code_ratio = code_like as f64 / total_lines as f64;

    let lower = trimmed.to_ascii_lowercase();
    let forum_hits: usize = ["points", "comments", "reply", "posted by", "upvote", "/r/"]
        .iter()
        .map(|m| lower.matches(m).count())
        .sum();
    let heading_lines = lines
        .iter()
        .filter(|l| l.trim_start().starts_with('#'))
        .count();
    let doc_keywords = [
        "parameters",
        "returns",
        "arguments",
        "usage",
        "installation",
    ]
    .iter()
    .filter(|k| lower.contains(*k))
    .count();

    if (fenced >= 1 && code_ratio > 0.25) || code_ratio > 0.5 {
        ContentType::CodeSnippet
    } else if forum_hits >= 3 {
        ContentType::ForumThread
    } else if heading_lines >= 2 && doc_keywords >= 1 {
        ContentType::Documentation
    } else if trimmed.len() >= 200 {
        ContentType::Article
    } else {
        ContentType::Unknown
    }
}

fn content_type_from_url(url: &str) -> Option<ContentType> {
    let u = url.to_ascii_lowercase();
    const FORUMS: [&str; 6] = [
        "reddit.com",
        "news.ycombinator.com",
        "v2ex.com",
        "quora.com",
        "stackoverflow.com",
        "stackexchange.com",
    ];
    if FORUMS.iter().any(|h| u.contains(h)) {
        return Some(ContentType::ForumThread);
    }
    if u.contains("gist.github")
        || u.contains("/raw/")
        || (u.contains("github.com") && u.contains("/blob/"))
    {
        return Some(ContentType::CodeSnippet);
    }
    if u.contains("docs.")
        || u.contains("/docs/")
        || u.contains("readthedocs")
        || u.contains("developer.")
    {
        return Some(ContentType::Documentation);
    }
    None
}

/// Heuristic: does this single line look like source code?
fn looks_like_code(line: &str) -> bool {
    // Indented blocks (4 spaces or a tab) are the classic Markdown code signal.
    if line.starts_with("    ") || line.starts_with('\t') {
        return true;
    }
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    if t.ends_with(['{', '}', ';']) {
        return true;
    }
    const TOKENS: [&str; 12] = [
        "fn ",
        "def ",
        "function ",
        "import ",
        "#include",
        "const ",
        "let ",
        "var ",
        "class ",
        "public ",
        "private ",
        "=>",
    ];
    TOKENS.iter().any(|tok| t.contains(tok))
}

/// Pull URLs, ISO dates, and a fenced-code-block count out of the text.
pub fn extract_key_facts(text: &str) -> KeyFacts {
    KeyFacts {
        urls: extract_urls(text),
        iso_dates: extract_iso_dates(text),
        code_block_count: text.matches("```").count() / 2,
    }
}

fn extract_urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (idx, _) in text.match_indices("http") {
        let rest = &text[idx..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            continue;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>' | ')' | ']' | '}'))
            .unwrap_or(rest.len());
        // Trim trailing sentence punctuation that's almost never part of a URL.
        let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if !url.is_empty() && !out.contains(&url.to_string()) {
            out.push(url.to_string());
        }
    }
    out
}

fn extract_iso_dates(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i + 10 <= n {
        let window: String = chars[i..i + 10].iter().collect();
        let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let next_digit = i + 10 < n && chars[i + 10].is_ascii_digit();
        if !prev_digit && !next_digit && is_iso_date(&window) {
            if !out.contains(&window) {
                out.push(window);
            }
            i += 10;
        } else {
            i += 1;
        }
    }
    out
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

/// Very common words that carry little topical signal for scoring.
const STOPWORDS: [&str; 30] = [
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "had", "her", "was",
    "one", "our", "out", "has", "him", "his", "how", "its", "may", "new", "now", "old", "see",
    "two", "who", "this", "that",
];

/// Split text into sentences on `.`/`!`/`?`, trimming whitespace.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if matches!(c, '.' | '!' | '?') {
            let s = cur.trim();
            if !s.is_empty() {
                out.push(s.to_string());
            }
            cur.clear();
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

/// Lowercased alphanumeric content words (length >= 3, not a stopword).
fn content_words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Word/sentence counts and an estimated reading time (200 wpm, rounded up).
pub fn reading_stats(text: &str) -> ReadingStats {
    let word_count = text.split_whitespace().count();
    ReadingStats {
        word_count,
        sentence_count: sentences(text).len(),
        reading_minutes: word_count.div_ceil(200),
    }
}

/// Extractive summary: return the `max_sentences` most salient sentences (by
/// summed content-word frequency, length-normalized), in their original order.
/// Dependency-free and deterministic — no model calls.
pub fn summarize(text: &str, max_sentences: usize) -> String {
    if max_sentences == 0 {
        return String::new();
    }
    let sents = sentences(text);
    if sents.len() <= max_sentences {
        return sents.join(" ");
    }

    let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for s in &sents {
        for w in content_words(s) {
            *freq.entry(w).or_default() += 1;
        }
    }

    let mut scored: Vec<(usize, f64)> = sents
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let words = content_words(s);
            let score = if words.is_empty() {
                0.0
            } else {
                let total: usize = words
                    .iter()
                    .map(|w| freq.get(w).copied().unwrap_or(0))
                    .sum();
                total as f64 / (words.len() as f64).sqrt()
            };
            (i, score)
        })
        .collect();

    // Pick the top-scoring sentences, then restore reading order.
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut chosen: Vec<usize> = scored
        .into_iter()
        .take(max_sentences)
        .map(|(i, _)| i)
        .collect();
    chosen.sort_unstable();
    chosen
        .into_iter()
        .map(|i| sents[i].clone())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Truncate to at most `max_chars`, but cut on a paragraph, sentence, or word
/// boundary rather than mid-token, and note how much was dropped. Returns the
/// text unchanged when it already fits.
pub fn smart_truncate(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }

    let prefix: String = text.chars().take(max_chars).collect();
    // Prefer a paragraph break, then a sentence end, then a word boundary.
    let cut = prefix
        .rfind("\n\n")
        .or_else(|| prefix.rfind(['.', '!', '?', '\n']).map(|i| i + 1))
        .or_else(|| prefix.rfind(' '))
        .unwrap_or(prefix.len());
    let kept = prefix[..cut].trim_end();
    let dropped = total - kept.chars().count();
    format!("{kept}\n\n[… truncated {dropped} chars …]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_unknown() {
        assert_eq!(detect_content_type("   ", None), ContentType::Unknown);
    }

    #[test]
    fn url_hint_overrides_text() {
        // Prose that reads like an article, but the URL says forum.
        let text = "A long thoughtful paragraph about distributed systems. ".repeat(10);
        assert_eq!(
            detect_content_type(&text, Some("https://www.reddit.com/r/rust/abc")),
            ContentType::ForumThread
        );
    }

    #[test]
    fn detects_code_from_text() {
        let code = "```\nfn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n```";
        assert_eq!(detect_content_type(code, None), ContentType::CodeSnippet);
    }

    #[test]
    fn detects_forum_thread() {
        let text = "alice 42 points 3 comments\nreply\nbob 10 points\nreply posted by carol";
        assert_eq!(detect_content_type(text, None), ContentType::ForumThread);
    }

    #[test]
    fn detects_documentation() {
        let text = "# API\n## usage\nParameters and returns are listed here.\n## installation";
        assert_eq!(detect_content_type(text, None), ContentType::Documentation);
    }

    #[test]
    fn long_prose_is_article() {
        let text = "Rust's ownership model eliminates data races at compile time. ".repeat(6);
        assert_eq!(detect_content_type(&text, None), ContentType::Article);
    }

    #[test]
    fn extracts_urls_dedup_and_trims_punctuation() {
        let text = "see https://example.com/a, and https://example.com/a again, plus http://b.io.";
        let facts = extract_key_facts(text);
        assert_eq!(
            facts.urls,
            vec![
                "https://example.com/a".to_string(),
                "http://b.io".to_string()
            ]
        );
    }

    #[test]
    fn extracts_iso_dates_only() {
        let facts = extract_key_facts("released 2024-01-15, not 12345-6 or 99-99-99");
        assert_eq!(facts.iso_dates, vec!["2024-01-15".to_string()]);
    }

    #[test]
    fn counts_code_blocks() {
        let facts = extract_key_facts("```rust\nx\n```\nprose\n```\ny\n```");
        assert_eq!(facts.code_block_count, 2);
    }

    #[test]
    fn smart_truncate_keeps_short_text() {
        assert_eq!(smart_truncate("short", 100), "short");
    }

    #[test]
    fn smart_truncate_cuts_on_boundary_not_midword() {
        let text = "First sentence here. Second sentence is much longer and goes on.";
        let out = smart_truncate(text, 25);
        assert!(out.starts_with("First sentence here."));
        assert!(out.contains("truncated"));
        // The kept portion must not end in the middle of a word.
        let kept = out.split("\n\n[").next().unwrap();
        assert!(!kept.ends_with("Secon"), "cut mid-word: {kept:?}");
    }

    #[test]
    fn analyze_combines_type_and_facts() {
        let a = analyze("```\nfn x() {}\n```\nhttps://r.io 2023-05-05", None);
        assert_eq!(a.content_type, ContentType::CodeSnippet);
        assert_eq!(a.key_facts.code_block_count, 1);
        assert_eq!(a.key_facts.urls, vec!["https://r.io".to_string()]);
        assert_eq!(a.key_facts.iso_dates, vec!["2023-05-05".to_string()]);
    }

    #[test]
    fn reading_stats_counts_words_sentences_and_time() {
        let stats = reading_stats("Hello world. This is a test! Is it?");
        assert_eq!(stats.word_count, 8);
        assert_eq!(stats.sentence_count, 3);
        assert_eq!(stats.reading_minutes, 1);
        assert_eq!(reading_stats("").reading_minutes, 0);
    }

    #[test]
    fn summarize_picks_salient_sentences_in_order() {
        let text = "Rust is a systems programming language. \
            Rust guarantees memory safety without a garbage collector. \
            The weather today is sunny and warm. \
            Rust's ownership and borrowing power its memory safety.";
        let summary = summarize(text, 2);
        // The two Rust-heavy sentences should win over the weather aside.
        assert!(summary.to_lowercase().contains("memory safety"));
        assert!(!summary.contains("weather"));
        // Original order is preserved (the earlier selected sentence comes first).
        let first = summary.find("guarantees memory safety");
        let later = summary.find("ownership and borrowing");
        assert!(first.is_some() && later.is_some() && first < later);
    }

    #[test]
    fn summarize_returns_all_when_short() {
        assert_eq!(
            summarize("Only one sentence here.", 3),
            "Only one sentence here."
        );
        assert_eq!(summarize("anything", 0), "");
    }
}
