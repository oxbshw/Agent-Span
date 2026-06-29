//! Registry of all built-in channels.

use std::sync::Arc;

use agentspan_core::channel::Channel;

use crate::{
    AnthropicChannel, ArxivChannel, BilibiliChannel, BingChannel, BraveChannel, CoinbaseChannel,
    CratesChannel, DevToChannel, DiscordChannel, DockerHubChannel, DuckDuckGoChannel,
    ExaSearchChannel, FlightChannel, GitLabChannel, GithubChannel, GoogleChannel,
    GoogleNewsChannel, GutenbergChannel, HackerNewsChannel, HuggingFaceChannel, InstagramChannel,
    LinkedInChannel, LobstersChannel, MapsChannel, NotionChannel, NpmChannel, OpenAiChannel,
    OpenLibraryChannel, PinterestChannel, PodcastIndexChannel, PypiChannel, QuoraChannel,
    RedditChannel, RssChannel, ScholarChannel, SlackChannel, SpotifyChannel, StatusPageChannel,
    TelegramChannel, TiktokChannel, TwitchChannel, TwitterChannel, V2exChannel, WaybackChannel,
    WeatherChannel, WebChannel, WikidataChannel, WikipediaChannel, XiaohongshuChannel,
    XiaoyuzhouChannel, XueqiuChannel, YoutubeChannel,
};

/// Holds all available channels and selects by name or URL.
#[derive(Debug, Clone)]
pub struct ChannelRegistry {
    channels: Vec<Arc<dyn Channel>>,
}

impl ChannelRegistry {
    /// Create a registry with the default set of channels.
    pub fn default_channels() -> Self {
        // Order matters: domain-specific channels first, then RSS, then the
        // generic web channel last (it can_handle any http(s) URL).
        let channels: Vec<Arc<dyn Channel>> = vec![
            Arc::new(GithubChannel::new()),
            Arc::new(HackerNewsChannel::new()),
            Arc::new(V2exChannel::new()),
            Arc::new(YoutubeChannel::new()),
            Arc::new(TiktokChannel::new()),
            Arc::new(TwitterChannel::new()),
            Arc::new(RedditChannel::new()),
            Arc::new(BilibiliChannel::new()),
            Arc::new(XiaohongshuChannel::new()),
            Arc::new(InstagramChannel::new()),
            Arc::new(LinkedInChannel::new()),
            Arc::new(XueqiuChannel::new()),
            Arc::new(XiaoyuzhouChannel::new()),
            Arc::new(ExaSearchChannel::new()),
            Arc::new(WikipediaChannel::new()),
            Arc::new(ArxivChannel::new()),
            Arc::new(DiscordChannel::new()),
            Arc::new(TelegramChannel::new()),
            Arc::new(SpotifyChannel::new()),
            Arc::new(TwitchChannel::new()),
            Arc::new(ScholarChannel::new()),
            Arc::new(PodcastIndexChannel::new()),
            Arc::new(QuoraChannel::new()),
            Arc::new(PinterestChannel::new()),
            Arc::new(NpmChannel::new()),
            Arc::new(CratesChannel::new()),
            Arc::new(PypiChannel::new()),
            Arc::new(GitLabChannel::new()),
            Arc::new(DockerHubChannel::new()),
            Arc::new(WaybackChannel::new()),
            Arc::new(MapsChannel::new()),
            Arc::new(WeatherChannel::new()),
            Arc::new(CoinbaseChannel::new()),
            Arc::new(DuckDuckGoChannel::new()),
            Arc::new(GoogleNewsChannel::new()),
            Arc::new(StatusPageChannel::new()),
            Arc::new(HuggingFaceChannel::new()),
            Arc::new(OpenAiChannel::new()),
            Arc::new(AnthropicChannel::new()),
            Arc::new(BraveChannel::new()),
            Arc::new(BingChannel::new()),
            Arc::new(GoogleChannel::new()),
            Arc::new(NotionChannel::new()),
            Arc::new(SlackChannel::new()),
            Arc::new(FlightChannel::new()),
            Arc::new(DevToChannel::new()),
            Arc::new(OpenLibraryChannel::new()),
            Arc::new(GutenbergChannel::new()),
            Arc::new(LobstersChannel::new()),
            Arc::new(WikidataChannel::new()),
            Arc::new(RssChannel::new()),
            Arc::new(WebChannel::new()),
        ];
        Self { channels }
    }

    /// Create a registry from an explicit list of channels.
    pub fn new(channels: Vec<Arc<dyn Channel>>) -> Self {
        Self { channels }
    }

    /// List all registered channels.
    pub fn list(&self) -> &[Arc<dyn Channel>] {
        &self.channels
    }

    /// Find a channel by exact name.
    pub fn by_name(&self, name: &str) -> Option<Arc<dyn Channel>> {
        self.channels.iter().find(|c| c.name() == name).cloned()
    }

    /// Find the first channel that can handle the given URL.
    pub fn by_url(&self, url: &str) -> Option<Arc<dyn Channel>> {
        self.channels.iter().find(|c| c.can_handle(url)).cloned()
    }

    /// Suggest up to `max` channel names closest to `name` — for "did you mean"
    /// hints when a lookup misses. Ranks by edit distance, also accepting
    /// substring matches; far-off names are filtered out.
    pub fn suggest(&self, name: &str, max: usize) -> Vec<String> {
        let q = name.to_lowercase();
        let tolerance = (q.len() / 2).max(2);
        let mut scored: Vec<(usize, String)> = self
            .channels
            .iter()
            .map(|c| {
                let n = c.name().to_string();
                let nl = n.to_lowercase();
                let close = nl.contains(&q) || q.contains(&nl);
                // Substring matches sort as distance 0 so they always rank first.
                let dist = if close { 0 } else { levenshtein(&q, &nl) };
                (dist, n)
            })
            .filter(|(dist, _)| *dist <= tolerance)
            .collect();
        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        scored.into_iter().take(max).map(|(_, n)| n).collect()
    }
}

/// Levenshtein edit distance between two strings (classic DP, O(m*n)).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_expected_channels() {
        let registry = ChannelRegistry::default_channels();
        assert_eq!(registry.list().len(), 52);
        for name in [
            "github",
            "hackernews",
            "v2ex",
            "youtube",
            "twitter",
            "reddit",
            "bilibili",
            "xiaohongshu",
            "linkedin",
            "xueqiu",
            "xiaoyuzhou",
            "exa",
            "wikipedia",
            "arxiv",
            "discord",
            "telegram",
            "spotify",
            "twitch",
            "scholar",
            "podcasts",
            "quora",
            "pinterest",
            "npm",
            "crates",
            "pypi",
            "gitlab",
            "dockerhub",
            "wayback",
            "maps",
            "weather",
            "coinbase",
            "duckduckgo",
            "gnews",
            "statuspage",
            "huggingface",
            "openai",
            "anthropic",
            "brave",
            "bing",
            "google",
            "notion",
            "slack",
            "flight",
            "devto",
            "openlibrary",
            "gutenberg",
            "lobsters",
            "wikidata",
            "rss",
            "web",
        ] {
            assert!(registry.by_name(name).is_some(), "missing channel: {name}");
        }
    }

    #[test]
    fn levenshtein_basic_distances() {
        assert_eq!(levenshtein("github", "github"), 0);
        assert_eq!(levenshtein("githubb", "github"), 1);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn suggest_finds_close_channel_names() {
        let registry = ChannelRegistry::default_channels();
        // A one-character typo should surface the intended channel.
        let s = registry.suggest("githubb", 3);
        assert!(s.contains(&"github".to_string()), "got {s:?}");
        // A substring query matches too.
        assert!(registry
            .suggest("wiki", 5)
            .contains(&"wikipedia".to_string()));
        // Pure nonsense yields nothing.
        assert!(registry.suggest("zzzzzzzzzz", 3).is_empty());
    }

    #[test]
    fn registry_selects_by_url() {
        let registry = ChannelRegistry::default_channels();
        assert_eq!(
            registry.by_url("https://example.com").unwrap().name(),
            "web"
        );
        assert_eq!(
            registry
                .by_url("https://github.com/agentspan/agentspan")
                .unwrap()
                .name(),
            "github"
        );
        assert_eq!(
            registry.by_url("https://example.com/feed").unwrap().name(),
            "rss"
        );
    }

    #[test]
    fn registry_selects_platform_channels_by_url() {
        let registry = ChannelRegistry::default_channels();
        assert_eq!(
            registry
                .by_url("https://news.ycombinator.com/item?id=1")
                .unwrap()
                .name(),
            "hackernews"
        );
        assert_eq!(
            registry
                .by_url("https://www.youtube.com/watch?v=x")
                .unwrap()
                .name(),
            "youtube"
        );
        assert_eq!(
            registry
                .by_url("https://www.reddit.com/r/rust")
                .unwrap()
                .name(),
            "reddit"
        );
        assert_eq!(
            registry
                .by_url("https://www.bilibili.com/video/BV1")
                .unwrap()
                .name(),
            "bilibili"
        );
        assert_eq!(
            registry
                .by_url("https://en.wikipedia.org/wiki/Rust")
                .unwrap()
                .name(),
            "wikipedia"
        );
        assert_eq!(
            registry
                .by_url("https://arxiv.org/abs/2401.12345")
                .unwrap()
                .name(),
            "arxiv"
        );
        assert_eq!(
            registry
                .by_url("https://discord.com/channels/1/2")
                .unwrap()
                .name(),
            "discord"
        );
    }
}
