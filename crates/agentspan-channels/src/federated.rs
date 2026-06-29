//! Federated search: query many channels at once and merge the results.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

use agentspan_core::channel::Channel;
use agentspan_core::types::SearchOptions;

use crate::registry::ChannelRegistry;

/// A single merged result and the channel(s) that surfaced it.
#[derive(Debug, Clone, Serialize)]
pub struct SourcedResult {
    pub channels: Vec<String>,
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// A per-channel failure during a federated search (one bad channel doesn't
/// sink the whole query).
#[derive(Debug, Clone, Serialize)]
pub struct FederatedError {
    pub channel: String,
    pub error: String,
}

/// The combined outcome of a federated search.
#[derive(Debug, Clone, Serialize)]
pub struct FederatedResults {
    pub query: String,
    pub searched: Vec<String>,
    pub errors: Vec<FederatedError>,
    pub results: Vec<SourcedResult>,
}

impl FederatedResults {
    /// Re-order results by lexical relevance to `query` (title + snippet),
    /// breaking ties by how many channels surfaced each result. Opt-in: the
    /// default federated ranking is purely by source count.
    pub fn rerank(&mut self, query: &str) {
        let mut scored: Vec<(f64, SourcedResult)> = self
            .results
            .drain(..)
            .map(|r| {
                let score = crate::rank::relevance_score(query, &[&r.title, &r.snippet]);
                (score, r)
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.channels.len().cmp(&a.1.channels.len()))
        });
        self.results = scored.into_iter().map(|(_, r)| r).collect();
    }

    /// Collapse near-duplicate results — entries whose titles exceed `threshold`
    /// token-similarity (0.0..=1.0), e.g. the same article re-syndicated under
    /// different URLs. The first occurrence (current order) is kept and absorbs
    /// the others' source channels. Opt-in; runs after any URL de-duplication.
    pub fn collapse_near_duplicates(&mut self, threshold: f64) {
        let mut kept: Vec<SourcedResult> = Vec::with_capacity(self.results.len());
        for candidate in self.results.drain(..) {
            let mut merged = false;
            for existing in &mut kept {
                if crate::rank::token_similarity(&existing.title, &candidate.title) >= threshold {
                    for ch in candidate.channels.iter() {
                        if !existing.channels.contains(ch) {
                            existing.channels.push(ch.clone());
                        }
                    }
                    merged = true;
                    break;
                }
            }
            if !merged {
                kept.push(candidate);
            }
        }
        self.results = kept;
    }
}

/// Normalise a URL for dedup: drop the fragment and a trailing slash, lowercase.
fn normalize_url(url: &str) -> String {
    url.split('#')
        .next()
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_lowercase()
}

impl ChannelRegistry {
    /// Search several channels concurrently and merge the results.
    ///
    /// `channels` selects channels by name; `None` queries them all. Identical
    /// URLs are de-duplicated (merging the source list), and results surfaced by
    /// more channels rank first. Per-channel errors are collected, not fatal.
    pub async fn federated_search(
        &self,
        query: &str,
        channels: Option<&[String]>,
        limit: usize,
    ) -> FederatedResults {
        let selected: Vec<Arc<dyn Channel>> = match channels {
            Some(names) => names.iter().filter_map(|n| self.by_name(n)).collect(),
            None => self.list().to_vec(),
        };
        let opts = SearchOptions {
            limit,
            ..Default::default()
        };

        let searches = selected.iter().map(|ch| {
            let ch = ch.clone();
            let query = query.to_string();
            let opts = opts.clone();
            async move {
                let name = ch.name().to_string();
                (name, ch.search(&query, opts).await)
            }
        });
        let outcomes = futures::future::join_all(searches).await;

        let mut searched = Vec::new();
        let mut errors = Vec::new();
        let mut results: Vec<SourcedResult> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();

        for (channel, outcome) in outcomes {
            searched.push(channel.clone());
            let hits = match outcome {
                Ok(hits) => hits,
                Err(e) => {
                    errors.push(FederatedError {
                        channel,
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            for hit in hits {
                let key = normalize_url(&hit.url);
                if key.is_empty() {
                    continue;
                }
                if let Some(&idx) = seen.get(&key) {
                    let entry: &mut SourcedResult = &mut results[idx];
                    if !entry.channels.contains(&channel) {
                        entry.channels.push(channel.clone());
                    }
                } else {
                    seen.insert(key, results.len());
                    results.push(SourcedResult {
                        channels: vec![channel.clone()],
                        title: hit.title,
                        url: hit.url,
                        snippet: hit.snippet,
                    });
                }
            }
        }

        // More sources => more likely relevant. Stable sort keeps discovery
        // order within an equal source count.
        results.sort_by_key(|r| std::cmp::Reverse(r.channels.len()));
        if limit > 0 {
            results.truncate(limit);
        }

        FederatedResults {
            query: query.to_string(),
            searched,
            errors,
            results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::backend::Backend;
    use agentspan_core::error::ChannelError;
    use agentspan_core::types::{Content, ReadOptions, SearchResult, Tier};
    use async_trait::async_trait;

    fn sr(title: &str, url: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: String::new(),
            author: None,
            timestamp: None,
            metadata: serde_json::Value::Null,
        }
    }

    #[derive(Debug)]
    struct MockChannel {
        name: String,
        results: Vec<SearchResult>,
        fail: bool,
    }

    impl MockChannel {
        fn ok(name: &str, results: Vec<SearchResult>) -> Arc<dyn Channel> {
            Arc::new(Self {
                name: name.to_string(),
                results,
                fail: false,
            })
        }
        fn failing(name: &str) -> Arc<dyn Channel> {
            Arc::new(Self {
                name: name.to_string(),
                results: vec![],
                fail: true,
            })
        }
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "mock channel"
        }
        fn can_handle(&self, _url: &str) -> bool {
            false
        }
        fn tier(&self) -> Tier {
            Tier::Zero
        }
        fn backends(&self) -> Vec<Box<dyn Backend>> {
            vec![]
        }
        async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, ChannelError> {
            Err(ChannelError::BackendUnavailable("mock".to_string()))
        }
        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, ChannelError> {
            if self.fail {
                Err(ChannelError::BackendUnavailable("boom".to_string()))
            } else {
                Ok(self.results.clone())
            }
        }
    }

    #[test]
    fn normalize_url_strips_slash_and_fragment() {
        assert_eq!(
            normalize_url("https://R.io/Page/#frag"),
            "https://r.io/page"
        );
        assert_eq!(normalize_url("https://r.io"), "https://r.io");
    }

    #[tokio::test]
    async fn dedups_across_channels_and_ranks_by_sources() {
        let registry = ChannelRegistry::new(vec![
            MockChannel::ok(
                "a",
                vec![sr("Rust", "https://r.io"), sr("Only A", "https://a.io")],
            ),
            MockChannel::ok("b", vec![sr("Rust dup", "https://r.io/")]),
        ]);

        let out = registry.federated_search("rust", None, 0).await;
        assert_eq!(out.searched.len(), 2);
        assert!(out.errors.is_empty());
        assert_eq!(out.results.len(), 2);
        // r.io was found by both channels -> ranked first, sources merged.
        assert_eq!(out.results[0].url, "https://r.io");
        assert_eq!(
            out.results[0].channels,
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(out.results[1].url, "https://a.io");
    }

    #[tokio::test]
    async fn rerank_orders_by_relevance_to_query() {
        // One channel, two results; the less-relevant one is discovered first.
        let registry = ChannelRegistry::new(vec![MockChannel::ok(
            "a",
            vec![
                sr("Sourdough baking", "https://b.io"),
                sr("Async Rust runtime", "https://r.io"),
            ],
        )]);

        let mut out = registry.federated_search("async rust", None, 0).await;
        // Default (by source count) keeps discovery order here.
        assert_eq!(out.results[0].url, "https://b.io");
        out.rerank("async rust");
        // After rerank, the query-relevant result comes first.
        assert_eq!(out.results[0].url, "https://r.io");
    }

    #[tokio::test]
    async fn collapse_merges_near_duplicate_titles() {
        // Same story title, different URLs, from two channels.
        let registry = ChannelRegistry::new(vec![
            MockChannel::ok("a", vec![sr("Rust 2.0 announced today", "https://a.io/x")]),
            MockChannel::ok("b", vec![sr("Rust 2.0 announced today", "https://b.io/y")]),
        ]);

        let mut out = registry.federated_search("rust", None, 0).await;
        assert_eq!(out.results.len(), 2); // distinct URLs -> not URL-deduped
        out.collapse_near_duplicates(0.85);
        assert_eq!(out.results.len(), 1);
        // The surviving result carries both source channels.
        assert_eq!(out.results[0].channels.len(), 2);
    }

    #[tokio::test]
    async fn collects_per_channel_errors() {
        let registry = ChannelRegistry::new(vec![
            MockChannel::ok("a", vec![sr("X", "https://x.io")]),
            MockChannel::failing("b"),
        ]);

        let out = registry.federated_search("q", None, 0).await;
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.errors.len(), 1);
        assert_eq!(out.errors[0].channel, "b");
    }

    #[tokio::test]
    async fn selects_named_channels_only() {
        let registry = ChannelRegistry::new(vec![
            MockChannel::ok("a", vec![sr("X", "https://x.io")]),
            MockChannel::ok("b", vec![sr("Y", "https://y.io")]),
        ]);

        let only_a = ["a".to_string()];
        let out = registry.federated_search("q", Some(&only_a), 0).await;
        assert_eq!(out.searched, vec!["a".to_string()]);
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.results[0].url, "https://x.io");
    }
}
