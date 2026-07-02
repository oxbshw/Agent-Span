//! Deterministic structured extraction — turn prose into typed JSON fields.
//!
//! Agents increasingly want *structured* output, not paragraphs: "give me the
//! title, the dates, the prices and the links as JSON." This module pulls a fixed
//! set of well-understood fields out of free text with cheap, deterministic
//! heuristics (no model, no schema-inference magic) and lets the caller **project**
//! just the fields they asked for — so a channel can answer in a JSON shape the
//! agent can parse, every time, with zero token spend on an LLM extraction pass.
//!
//! It reuses [`extract_key_facts`] for URLs and ISO dates and [`summarize`]
//! for the summary.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::intelligence::{extract_key_facts, summarize};

/// The structured view extracted from a blob of text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Extraction {
    /// First meaningful line (heading markers stripped), capped to 200 chars.
    pub title: Option<String>,
    /// Distinct http(s) URLs in order of appearance.
    pub urls: Vec<String>,
    /// Distinct email addresses.
    pub emails: Vec<String>,
    /// ISO-8601 (`YYYY-MM-DD`) dates.
    pub iso_dates: Vec<String>,
    /// Currency-prefixed amounts, e.g. `"$19.99"`.
    pub prices: Vec<String>,
    /// A two-sentence extractive summary.
    pub summary: String,
}

/// The field names [`Extraction::project`] understands.
pub const FIELDS: [&str; 6] = ["title", "urls", "emails", "iso_dates", "prices", "summary"];

/// Extract all known fields from `text`.
pub fn extract(text: &str) -> Extraction {
    let facts = extract_key_facts(text);
    Extraction {
        title: extract_title(text),
        urls: facts.urls,
        emails: extract_emails(text),
        iso_dates: facts.iso_dates,
        prices: extract_prices(text),
        summary: summarize(text, 2),
    }
}

impl Extraction {
    /// The full extraction as a JSON object.
    pub fn to_json(&self) -> Value {
        json!({
            "title": self.title,
            "urls": self.urls,
            "emails": self.emails,
            "iso_dates": self.iso_dates,
            "prices": self.prices,
            "summary": self.summary,
        })
    }

    /// Project only the requested top-level fields into a JSON object. Unknown
    /// field names are ignored, so an agent can request exactly the schema it wants.
    pub fn project(&self, fields: &[&str]) -> Value {
        let full = self.to_json();
        let mut out = Map::new();
        if let Value::Object(map) = full {
            for f in fields {
                if let Some(v) = map.get(*f) {
                    out.insert((*f).to_string(), v.clone());
                }
            }
        }
        Value::Object(out)
    }
}

/// First non-empty line, with Markdown heading markers stripped, capped in length.
fn extract_title(text: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim().trim_start_matches('#').trim();
        if !t.is_empty() {
            return Some(t.chars().take(200).collect());
        }
    }
    None
}

/// Pull distinct, plausibly-valid email addresses.
fn extract_emails(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split(|c: char| {
        c.is_whitespace() || matches!(c, '<' | '>' | '"' | '(' | ')' | ',' | ';' | '[' | ']')
    }) {
        // Trim trailing/leading punctuation that isn't part of an address.
        let tok = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if is_email(tok) && !out.contains(&tok.to_string()) {
            out.push(tok.to_string());
        }
    }
    out
}

fn is_email(s: &str) -> bool {
    let at = match s.find('@') {
        Some(i) => i,
        None => return false,
    };
    let local = &s[..at];
    let domain = &s[at + 1..];
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
        && local
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
        && domain
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-'))
}

/// Pull distinct currency-prefixed amounts (`$`, `€`, `£`).
fn extract_prices(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < n {
        if matches!(chars[i], '$' | '€' | '£') {
            let start = i;
            let mut j = i + 1;
            let mut seen_digit = false;
            while j < n && (chars[j].is_ascii_digit() || matches!(chars[j], ',' | '.')) {
                if chars[j].is_ascii_digit() {
                    seen_digit = true;
                }
                j += 1;
            }
            if seen_digit {
                let price: String = chars[start..j].iter().collect();
                let price = price.trim_end_matches(['.', ',']).to_string();
                if !out.contains(&price) {
                    out.push(price);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Acme Pro Plan\n\
        Contact sales@acme.io or support@acme.io for details.\n\
        Launched 2024-03-01, see https://acme.io/pricing for the $19.99 monthly tier.\n\
        Enterprise plans start at $199 per seat. This sentence pads the summary.";

    #[test]
    fn extracts_all_fields() {
        let e = extract(SAMPLE);
        assert_eq!(e.title.as_deref(), Some("Acme Pro Plan"));
        assert_eq!(e.urls, vec!["https://acme.io/pricing".to_string()]);
        assert_eq!(
            e.emails,
            vec!["sales@acme.io".to_string(), "support@acme.io".to_string()]
        );
        assert_eq!(e.iso_dates, vec!["2024-03-01".to_string()]);
        assert_eq!(e.prices, vec!["$19.99".to_string(), "$199".to_string()]);
        assert!(!e.summary.is_empty());
    }

    #[test]
    fn project_returns_only_requested_keys() {
        let e = extract(SAMPLE);
        let v = e.project(&["title", "prices", "nonexistent"]);
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("title"));
        assert!(obj.contains_key("prices"));
        assert!(!obj.contains_key("urls"));
        assert!(!obj.contains_key("nonexistent"));
    }

    #[test]
    fn email_heuristic_rejects_non_emails() {
        assert!(is_email("a@b.com"));
        assert!(is_email("first.last+tag@sub.example.co"));
        assert!(!is_email("not-an-email"));
        assert!(!is_email("@nope.com"));
        assert!(!is_email("trailing@dot."));
        assert!(!is_email("two@@at.com"));
    }

    #[test]
    fn prices_dedup_and_trim_trailing_punctuation() {
        let out = extract_prices("It costs $5, or $5 again, and £10.50. Free items are $0.");
        assert_eq!(
            out,
            vec!["$5".to_string(), "£10.50".to_string(), "$0".to_string()]
        );
    }

    #[test]
    fn empty_input_is_all_empty() {
        let e = extract("");
        assert!(e.title.is_none());
        assert!(e.urls.is_empty());
        assert!(e.emails.is_empty());
        assert!(e.prices.is_empty());
        assert!(e.summary.is_empty());
    }
}
