//! Tracks URLs that no channel can handle, so popular gaps surface themselves.
//!
//! When an agent asks for a URL and `ChannelRegistry::by_url` comes back empty,
//! we tally it here by host. The host with the most misses this week is the
//! channel most worth building next — instead of guessing what to add, we let
//! real demand rank the backlog.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;
use tracing::info;

/// Per-host tally of unsupported requests.
#[derive(Debug, Clone)]
struct DomainStat {
    count: u64,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    sample_url: String,
}

/// A serializable view of demand for an unsupported platform.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnsupportedPlatform {
    pub domain: String,
    pub suggested_channel: String,
    pub count: u64,
    pub sample_url: String,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// Counts requests for platforms AgentSpan doesn't yet support.
#[derive(Debug, Clone, Default)]
pub struct MissingChannelDetector {
    domains: Arc<DashMap<String, DomainStat>>,
    // A simple counter of total misses, handy for the dashboard headline.
    total: Arc<RwLock<u64>>,
}

impl MissingChannelDetector {
    /// Create an empty detector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one request for a URL that no channel could handle.
    pub fn record_unsupported(&self, url: &str) {
        let Some(domain) = host_of(url) else {
            return;
        };
        let now = Utc::now();
        let mut entry = self
            .domains
            .entry(domain.clone())
            .or_insert_with(|| DomainStat {
                count: 0,
                first_seen: now,
                last_seen: now,
                sample_url: url.to_string(),
            });
        entry.count += 1;
        entry.last_seen = now;

        *self.total.write().expect("detector lock poisoned") += 1;
        info!(
            domain = %domain,
            count = entry.count,
            "Unsupported URL: {url} — consider adding a channel for {domain}"
        );
    }

    /// Total unsupported requests seen.
    pub fn total(&self) -> u64 {
        *self.total.read().expect("detector lock poisoned")
    }

    /// The `n` most-requested unsupported platforms, most popular first.
    pub fn top(&self, n: usize) -> Vec<UnsupportedPlatform> {
        let mut all: Vec<UnsupportedPlatform> = self
            .domains
            .iter()
            .map(|e| view(e.key(), e.value()))
            .collect();
        all.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.domain.cmp(&b.domain)));
        all.truncate(n);
        all
    }

    /// The top 10 platforms requested within the last 7 days.
    pub fn weekly_report(&self) -> Vec<UnsupportedPlatform> {
        let cutoff = Utc::now() - chrono::Duration::days(7);
        let mut recent: Vec<UnsupportedPlatform> = self
            .domains
            .iter()
            .filter(|e| e.value().last_seen >= cutoff)
            .map(|e| view(e.key(), e.value()))
            .collect();
        recent.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.domain.cmp(&b.domain)));
        recent.truncate(10);
        recent
    }
}

fn view(domain: &str, stat: &DomainStat) -> UnsupportedPlatform {
    UnsupportedPlatform {
        domain: domain.to_string(),
        suggested_channel: suggest_channel_name(domain),
        count: stat.count,
        sample_url: stat.sample_url.clone(),
        first_seen: stat.first_seen,
        last_seen: stat.last_seen,
    }
}

/// Extract the host from a URL, lower-cased and without a leading `www.` or port.
///
/// Deliberately dependency-free: we only need the registrable host, not a full
/// URL parse.
pub fn host_of(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back() // drop any userinfo
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host.is_empty() || !host.contains('.') {
        None
    } else {
        Some(host.to_string())
    }
}

/// Guess a channel name from a host: the most significant label, e.g.
/// `news.substack.com` -> `substack`.
pub fn suggest_channel_name(domain: &str) -> String {
    let labels: Vec<&str> = domain.split('.').filter(|s| !s.is_empty()).collect();
    // For a normal `name.tld` take the second-to-last label; for deeper hosts the
    // registrable name is still usually second-to-last.
    if labels.len() >= 2 {
        labels[labels.len() - 2].to_string()
    } else {
        domain.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_extraction_handles_common_shapes() {
        assert_eq!(
            host_of("https://www.substack.com/p/article"),
            Some("substack.com".to_string())
        );
        assert_eq!(
            host_of("http://news.ycombinator.com:8080/item?id=1"),
            Some("news.ycombinator.com".to_string())
        );
        assert_eq!(
            host_of("ftp://files.example.org/x"),
            Some("files.example.org".to_string())
        );
        // No host / not a real domain.
        assert_eq!(host_of("not a url"), None);
        assert_eq!(host_of("https://localhost/x"), None);
    }

    #[test]
    fn channel_name_suggestion() {
        assert_eq!(suggest_channel_name("substack.com"), "substack");
        assert_eq!(suggest_channel_name("news.substack.com"), "substack");
        assert_eq!(suggest_channel_name("example.co.uk"), "co");
    }

    #[test]
    fn counts_accumulate_per_domain() {
        let d = MissingChannelDetector::new();
        d.record_unsupported("https://substack.com/a");
        d.record_unsupported("https://www.substack.com/b");
        d.record_unsupported("https://medium.com/c");
        assert_eq!(d.total(), 3);
        let top = d.top(10);
        assert_eq!(top[0].domain, "substack.com");
        assert_eq!(top[0].count, 2);
        assert_eq!(top[0].suggested_channel, "substack");
    }

    #[test]
    fn unparseable_urls_are_ignored() {
        let d = MissingChannelDetector::new();
        d.record_unsupported("garbage");
        assert_eq!(d.total(), 0);
        assert!(d.top(5).is_empty());
    }

    #[test]
    fn top_is_ranked_and_truncated() {
        let d = MissingChannelDetector::new();
        for _ in 0..5 {
            d.record_unsupported("https://a.com/x");
        }
        for _ in 0..2 {
            d.record_unsupported("https://b.com/x");
        }
        d.record_unsupported("https://c.com/x");
        let top = d.top(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].domain, "a.com");
        assert_eq!(top[1].domain, "b.com");
    }

    #[test]
    fn weekly_report_returns_recent_demand() {
        let d = MissingChannelDetector::new();
        d.record_unsupported("https://substack.com/a");
        let weekly = d.weekly_report();
        assert_eq!(weekly.len(), 1);
        assert_eq!(weekly[0].domain, "substack.com");
    }
}
