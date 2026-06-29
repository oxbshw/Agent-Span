//! Lexical relevance scoring for search results.
//!
//! A small, dependency-free TF-style scorer: it rewards query terms appearing in
//! a result's fields (the first field — usually the title — weighted higher),
//! with diminishing returns for repetition and a bonus for a full-phrase match.
//! No model calls, fully deterministic, so it is cheap to apply to every result.

/// Split text into lowercased alphanumeric tokens of length >= 2.
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_lowercase())
        .collect()
}

/// Jaccard similarity (0.0..=1.0) of the token sets of two strings. Used to spot
/// near-duplicate titles whose URLs differ (the same article re-syndicated).
pub fn token_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let sa: HashSet<String> = tokenize(a).into_iter().collect();
    let sb: HashSet<String> = tokenize(b).into_iter().collect();
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Score how relevant `fields` are to `query`. Higher is better; `0.0` means no
/// query term matched. The first field is weighted twice the rest (title bias).
pub fn relevance_score(query: &str, fields: &[&str]) -> f64 {
    let q_terms = tokenize(query);
    if q_terms.is_empty() {
        return 0.0;
    }
    let phrase = query.trim().to_lowercase();
    let mut score = 0.0;

    for (i, field) in fields.iter().enumerate() {
        let weight = if i == 0 { 2.0 } else { 1.0 };
        let tokens = tokenize(field);
        if tokens.is_empty() {
            continue;
        }
        for term in &q_terms {
            let tf = tokens.iter().filter(|t| *t == term).count() as f64;
            if tf > 0.0 {
                // ln(1 + tf): first occurrence matters most, repeats taper off.
                score += weight * (1.0 + tf).ln();
            }
        }
        // Reward an exact phrase hit (multi-word queries especially).
        if phrase.len() > 2 && field.to_lowercase().contains(&phrase) {
            score += weight * 0.5;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_drops_short_and_punctuation() {
        assert_eq!(tokenize("Rust, a-language!"), vec!["rust", "language"]);
        assert!(tokenize("  ").is_empty());
    }

    #[test]
    fn relevant_text_scores_higher_than_irrelevant() {
        let q = "async rust";
        let hit = relevance_score(q, &["Async Rust runtime", "tokio internals"]);
        let miss = relevance_score(q, &["Sourdough bread", "baking at home"]);
        assert!(hit > miss);
        assert_eq!(miss, 0.0);
    }

    #[test]
    fn title_field_outweighs_later_fields() {
        let in_title = relevance_score("rust", &["rust guide", "general"]);
        let in_snippet = relevance_score("rust", &["general guide", "rust"]);
        assert!(in_title > in_snippet);
    }

    #[test]
    fn exact_phrase_gets_a_bonus() {
        let phrase = relevance_score("web gateway", &["the web gateway pattern", ""]);
        let scattered = relevance_score("web gateway", &["gateway for the web", ""]);
        assert!(phrase > scattered);
    }

    #[test]
    fn empty_query_scores_zero() {
        assert_eq!(relevance_score("", &["anything here"]), 0.0);
        assert_eq!(relevance_score("   ", &["anything here"]), 0.0);
    }

    #[test]
    fn token_similarity_detects_near_duplicates() {
        assert_eq!(
            token_similarity("Rust async runtime", "Rust async runtime"),
            1.0
        );
        assert!(token_similarity("Rust async runtime guide", "Rust async runtime") > 0.7);
        assert!(token_similarity("Rust async runtime", "Sourdough baking tips") < 0.1);
        assert_eq!(token_similarity("", ""), 1.0);
    }
}
