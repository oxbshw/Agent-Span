//! AgentSpan channel implementations.

pub mod anthropic;
pub mod arxiv;
pub mod bilibili;
pub mod bing;
pub mod brave;
pub mod budget;
pub mod coinbase;
pub mod crates_io;
pub mod devto;
pub mod discord;
pub mod dockerhub;
pub mod duckduckgo;
pub mod exa_search;
pub mod extract;
pub mod federated;
pub mod fingerprint;
pub mod flight;
pub mod format;
pub mod github;
pub mod gitlab;
pub mod gnews;
pub mod google;
pub mod gutenberg;
pub mod hackernews;
pub mod healer;
pub mod http;
pub mod huggingface;
pub mod instagram;
pub mod intelligence;
pub mod linkedin;
pub mod lobsters;
pub mod maps;
pub mod notion;
pub mod npm;
pub mod openai;
pub mod opencli;
pub mod openlibrary;
pub mod pinterest;
pub mod podcast_index;
pub mod pypi;
pub mod quora;
pub mod rank;
pub mod reddit;
pub mod registry;
pub mod rss;
pub mod scholar;
pub mod slack;
pub mod spotify;
pub mod statuspage;
pub mod telegram;
pub mod tiktok;
pub mod transcribe;
pub mod twitch;
pub mod twitter;
pub mod v2ex;
pub mod wayback;
pub mod weather;
pub mod web;
pub mod wikidata;
pub mod wikipedia;
pub mod xiaohongshu;
pub mod xiaoyuzhou;
pub mod xueqiu;
pub mod youtube;

pub use anthropic::AnthropicChannel;
pub use arxiv::ArxivChannel;
pub use bilibili::BilibiliChannel;
pub use bing::BingChannel;
pub use brave::BraveChannel;
pub use budget::{estimate_tokens, fit_to_budget, BudgetResult, BudgetStrategy};
pub use coinbase::CoinbaseChannel;
pub use crates_io::CratesChannel;
pub use devto::DevToChannel;
pub use discord::DiscordChannel;
pub use dockerhub::DockerHubChannel;
pub use duckduckgo::DuckDuckGoChannel;
pub use exa_search::ExaSearchChannel;
pub use extract::{extract as extract_fields, Extraction};
pub use federated::{FederatedError, FederatedResults, SourcedResult};
pub use fingerprint::{
    changed_beyond, content_hash, diff as fingerprint_diff, fingerprint as content_fingerprint,
    simhash, Change, Fingerprint,
};
pub use flight::FlightChannel;
pub use github::GithubChannel;
pub use gitlab::GitLabChannel;
pub use gnews::GoogleNewsChannel;
pub use google::GoogleChannel;
pub use gutenberg::GutenbergChannel;
pub use hackernews::HackerNewsChannel;
pub use healer::{
    Alert, AlertManager, AlertRecord, AlertSeverity, AutoSwitch, BackendSwitcher, Healer,
    HealingReport, HealthMonitor, HealthSnapshot, MissingChannelDetector, RepairAttempt,
    RepairKind, RepairManager, SnapshotView, UnsupportedPlatform,
};
pub use huggingface::HuggingFaceChannel;
pub use instagram::InstagramChannel;
pub use intelligence::{
    analyze, reading_stats, summarize, ContentAnalysis, ContentType, KeyFacts, ReadingStats,
};
pub use linkedin::LinkedInChannel;
pub use lobsters::LobstersChannel;
pub use maps::MapsChannel;
pub use notion::NotionChannel;
pub use npm::NpmChannel;
pub use openai::OpenAiChannel;
pub use opencli::OpenCliBackend;
pub use openlibrary::OpenLibraryChannel;
pub use pinterest::PinterestChannel;
pub use podcast_index::PodcastIndexChannel;
pub use pypi::PypiChannel;
pub use quora::QuoraChannel;
pub use reddit::RedditChannel;
pub use registry::ChannelRegistry;
pub use rss::RssChannel;
pub use scholar::ScholarChannel;
pub use slack::SlackChannel;
pub use spotify::SpotifyChannel;
pub use statuspage::StatusPageChannel;
pub use telegram::TelegramChannel;
pub use tiktok::TiktokChannel;
pub use twitch::TwitchChannel;
pub use twitter::TwitterChannel;
pub use v2ex::V2exChannel;
pub use wayback::WaybackChannel;
pub use weather::WeatherChannel;
pub use web::WebChannel;
pub use wikidata::WikidataChannel;
pub use wikipedia::WikipediaChannel;
pub use xiaohongshu::XiaohongshuChannel;
pub use xiaoyuzhou::XiaoyuzhouChannel;
pub use xueqiu::XueqiuChannel;
pub use youtube::YoutubeChannel;

/// Minimal percent-encoding for query-string values (RFC 3986 unreserved set).
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_escapes_spaces_and_reserved() {
        assert_eq!(percent_encode("rust lang"), "rust%20lang");
        assert_eq!(percent_encode("a&b"), "a%26b");
        assert_eq!(percent_encode("plain-Text_1.0~"), "plain-Text_1.0~");
    }
}
