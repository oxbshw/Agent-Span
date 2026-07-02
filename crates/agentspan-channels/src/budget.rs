//! Token-budget compiler — fit content to a hard token ceiling.
//!
//! Agents pay per token and have finite context windows, so "give me this page in
//! at most 2 000 tokens" is one of the most common real requests. [`fit_to_budget`]
//! takes a blob of text plus a token ceiling and returns text guaranteed to fit,
//! degrading gracefully: first an extractive summary (dropping the least salient
//! sentences), and only if that still overshoots, a boundary-aware truncation.
//!
//! Everything here is deterministic and dependency-free — the same estimator the
//! rest of the stack can budget against, with no tokenizer crate and no model call.
//! It reuses the [`summarize`] and [`smart_truncate`] primitives.

use serde::Serialize;

use crate::intelligence::{smart_truncate, summarize};

/// How the text was made to fit the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetStrategy {
    /// Already within budget — returned unchanged.
    Untouched,
    /// Reduced to its most salient sentences (extractive summary).
    Summarized,
    /// Summarizing still overshot; hard-trimmed on a text boundary.
    Truncated,
}

/// Result of fitting text to a token budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetResult {
    /// The budget-compliant text.
    pub text: String,
    /// Estimated tokens of the original input.
    pub tokens_before: usize,
    /// Estimated tokens of [`BudgetResult::text`] — always `<= budget`.
    pub tokens_after: usize,
    /// The ceiling that was requested.
    pub budget: usize,
    /// Which strategy produced the result.
    pub strategy: BudgetStrategy,
}

/// Estimate the token count of `text` without a real tokenizer.
///
/// BPE tokenizers land near `chars / 4` for English prose but closer to one token
/// per short word for code/punctuation-heavy text. We take the **larger** of a
/// character-based and a word-based estimate, so the result is a safe upper bound:
/// budgeting against it never silently under-counts and blows the context window.
pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    let by_chars = chars.div_ceil(4);
    let by_words = words.saturating_mul(4).div_ceil(3); // ~1.33 tokens/word
    by_chars.max(by_words)
}

/// Number of non-empty sentences (cheap split on terminal punctuation).
fn sentence_count(text: &str) -> usize {
    text.split(['.', '!', '?'])
        .filter(|s| !s.trim().is_empty())
        .count()
}

/// Largest character count whose char-based estimate fits `max_tokens`.
fn char_budget_for(max_tokens: usize) -> usize {
    max_tokens.saturating_mul(4)
}

/// Hard-trim `text` so its estimate fits `max_tokens`. Boundary-aware where it can
/// be, shrinking until the (upper-bound) estimate is within budget. This is the
/// final guarantee path — it always returns something that fits (possibly empty).
fn hard_fit(text: &str, max_tokens: usize) -> String {
    if max_tokens == 0 {
        return String::new();
    }
    if estimate_tokens(text) <= max_tokens {
        return text.to_string();
    }
    let mut budget_chars = char_budget_for(max_tokens).max(1);
    // 3/4 geometric shrink converges to 0 well within 64 steps from any input.
    for _ in 0..64 {
        let candidate = smart_truncate(text, budget_chars);
        if estimate_tokens(&candidate) <= max_tokens {
            return candidate;
        }
        budget_chars = budget_chars * 3 / 4;
        if budget_chars == 0 {
            break;
        }
    }
    String::new()
}

/// Fit `text` within `max_tokens`, summarizing then truncating as needed.
///
/// The returned [`BudgetResult::tokens_after`] is always `<= max_tokens`.
pub fn fit_to_budget(text: &str, max_tokens: usize) -> BudgetResult {
    let tokens_before = estimate_tokens(text);

    if max_tokens == 0 {
        return BudgetResult {
            text: String::new(),
            tokens_before,
            tokens_after: 0,
            budget: 0,
            strategy: BudgetStrategy::Truncated,
        };
    }

    if tokens_before <= max_tokens {
        return BudgetResult {
            text: text.to_string(),
            tokens_before,
            tokens_after: tokens_before,
            budget: max_tokens,
            strategy: BudgetStrategy::Untouched,
        };
    }

    // 1) Extractive summary: scan from the most sentences down to one, returning
    //    the *largest* summary that fits (token count is monotone in sentences).
    let sentences = sentence_count(text);
    if sentences > 1 {
        let start = sentences.saturating_sub(1).clamp(1, 256);
        for n in (1..=start).rev() {
            let candidate = summarize(text, n);
            let after = estimate_tokens(&candidate);
            if after <= max_tokens {
                return BudgetResult {
                    text: candidate,
                    tokens_before,
                    tokens_after: after,
                    budget: max_tokens,
                    strategy: BudgetStrategy::Summarized,
                };
            }
        }
    }

    // 2) Hard fallback: boundary-aware truncation sized to the budget.
    let fitted = hard_fit(text, max_tokens);
    let after = estimate_tokens(&fitted);
    BudgetResult {
        text: fitted,
        tokens_before,
        tokens_after: after,
        budget: max_tokens,
        strategy: BudgetStrategy::Truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_is_nonzero_for_text_and_zero_for_empty() {
        assert_eq!(estimate_tokens(""), 0);
        assert!(estimate_tokens("hello world") >= 2);
        // Upper-bound property: more text never estimates fewer tokens.
        let short = estimate_tokens("alpha beta");
        let long = estimate_tokens("alpha beta gamma delta epsilon zeta");
        assert!(long >= short);
    }

    #[test]
    fn short_text_is_untouched() {
        let r = fit_to_budget("A short sentence.", 1000);
        assert_eq!(r.strategy, BudgetStrategy::Untouched);
        assert_eq!(r.text, "A short sentence.");
        assert!(r.tokens_after <= r.budget);
    }

    #[test]
    fn zero_budget_yields_empty() {
        let r = fit_to_budget("anything at all here", 0);
        assert_eq!(r.tokens_after, 0);
        assert!(r.text.is_empty());
        assert_eq!(r.strategy, BudgetStrategy::Truncated);
    }

    #[test]
    fn over_budget_text_is_reduced_and_never_exceeds_ceiling() {
        // Many distinct sentences so the summary path engages.
        let text = "Rust guarantees memory safety without a garbage collector. \
            Ownership and borrowing power that guarantee at compile time. \
            The async runtime Tokio drives high concurrency. \
            Cargo is the build tool and package manager. \
            Traits provide zero-cost abstraction over behavior. \
            The borrow checker rejects data races before they run. \
            Pattern matching makes state handling exhaustive. \
            Crates publish to a shared registry for reuse."
            .repeat(3);
        let budget = 40;
        let r = fit_to_budget(&text, budget);
        assert!(r.tokens_before > budget, "test text must exceed the budget");
        assert!(
            r.tokens_after <= budget,
            "fitted {} tokens must be <= budget {}",
            r.tokens_after,
            budget
        );
        assert!(!r.text.is_empty());
        assert!(matches!(
            r.strategy,
            BudgetStrategy::Summarized | BudgetStrategy::Truncated
        ));
    }

    #[test]
    fn punctuationless_blob_falls_back_to_truncation_and_fits() {
        // No sentence boundaries -> the summary path can't help -> hard truncate.
        let blob = "alpha ".repeat(2000);
        let budget = 50;
        let r = fit_to_budget(&blob, budget);
        assert!(
            r.tokens_after <= budget,
            "got {} > {}",
            r.tokens_after,
            budget
        );
        assert_eq!(r.strategy, BudgetStrategy::Truncated);
    }
}
