//! Token-reduction helpers for `Channel::format_for_llm`.
//!
//! Platform APIs return large JSON envelopes full of fields an LLM doesn't need
//! (awards, tracking ids, media variants…). These helpers pull out just the
//! human-readable text fields, drastically cutting tokens.

use agentspan_core::types::SearchResult;
use serde_json::Value;

/// Wrap unparseable CLI/API output as a single search result instead of failing.
///
/// Shell-out backends (OpenCLI, mcporter, twitter-cli) target tools whose exact
/// output shape isn't verified here; if JSON parsing fails we surface the raw
/// output so the caller still gets something actionable rather than an error.
pub fn raw_search_fallback(raw: &str) -> Vec<SearchResult> {
    vec![SearchResult {
        title: "⚠️ unparsed backend output".to_string(),
        url: String::new(),
        snippet: raw.trim().chars().take(500).collect(),
        author: None,
        timestamp: None,
        metadata: Value::String(raw.to_string()),
    }]
}

/// Extract the values of the named string fields (recursively) and join them.
///
/// Falls back to a truncated copy of the raw input when it isn't JSON or has no
/// matching fields.
pub fn extract_text_fields(raw: &str, keys: &[&str], max_chars: usize) -> String {
    let value: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return truncate(raw, max_chars),
    };
    let mut out = Vec::new();
    collect(&value, keys, &mut out);
    if out.is_empty() {
        return truncate(raw, max_chars);
    }
    truncate(&out.join("\n"), max_chars)
}

fn collect(value: &Value, keys: &[&str], out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if keys.contains(&k.as_str()) {
                    if let Some(s) = v.as_str() {
                        let s = s.trim();
                        if !s.is_empty() {
                            out.push(s.to_string());
                        }
                    }
                }
                collect(v, keys, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect(v, keys, out);
            }
        }
        _ => {}
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_text_fields() {
        let raw = r#"{"data":{"children":[
            {"data":{"title":"Hello","selftext":"world","ups":99,"awards":[1,2,3]}},
            {"data":{"body":"a comment","score":5}}
        ]}}"#;
        let out = extract_text_fields(raw, &["title", "selftext", "body"], 1000);
        assert!(out.contains("Hello"));
        assert!(out.contains("world"));
        assert!(out.contains("a comment"));
        assert!(!out.contains("awards"));
        assert!(!out.contains("99"));
    }

    #[test]
    fn raw_search_fallback_wraps_unparsed_output() {
        let r = raw_search_fallback("not json at all");
        assert_eq!(r.len(), 1);
        assert!(r[0].snippet.contains("not json"));
        assert!(r[0].title.contains("unparsed"));
    }

    #[test]
    fn falls_back_on_non_json() {
        assert_eq!(
            extract_text_fields("plain text", &["title"], 1000),
            "plain text"
        );
    }

    #[test]
    fn falls_back_when_no_fields_match() {
        let raw = r#"{"unrelated":1}"#;
        assert_eq!(extract_text_fields(raw, &["title"], 1000), raw);
    }

    #[test]
    fn truncates_long_output() {
        let raw = format!(r#"{{"title":"{}"}}"#, "x".repeat(5000));
        let out = extract_text_fields(&raw, &["title"], 100);
        assert!(out.chars().count() <= 101); // 100 + ellipsis
        assert!(out.ends_with('…'));
    }
}
