//! `agentspan format` — explain how each channel reduces tokens in `format_for_llm`.
//!
//! Useful for debugging token usage: it tells you what AgentSpan strips from each
//! platform's raw API response before handing text to an LLM.

use agentspan_channels::ChannelRegistry;
use clap::Args;

#[derive(Args)]
pub struct FormatArgs {
    /// Channel/platform name (e.g. `reddit`). Omit to list all.
    pub platform: Option<String>,
}

/// The token-reduction rule for a channel, or a generic pass-through note.
pub fn rule_for(channel: &str) -> &'static str {
    match channel {
        "reddit" => "Keeps title/selftext/body; drops awards, vote counts, and media variants.",
        "hackernews" => {
            "Keeps title/text/story_text/comment_text; drops ids, scores, and metadata."
        }
        "github" => "Keeps readable name/description/body fields; drops the large API envelope.",
        "wikipedia" => "Keeps the article extract/snippet/title; drops page metadata.",
        "discord" => "Keeps message content; drops embeds, reactions, and ids.",
        "web" | "quora" | "pinterest" | "linkedin" => {
            "Returns clean Markdown (Jina Reader); truncated to a token budget."
        }
        "exa" | "scholar" => "Search results trimmed to title/url/snippet/author.",
        "arxiv" => "Keeps title/abstract/authors; drops the raw Atom envelope.",
        "tiktok" => "Keeps title/description/uploader; drops the raw yt-dlp JSON envelope.",
        "instagram" => "Keeps caption/shortcode/uploader; drops media blobs and tracking ids.",
        "spotify" | "twitch" | "telegram" | "podcasts" => {
            "Keeps the human-readable fields; drops tracking ids and media blobs."
        }
        "npm" | "crates" | "pypi" => {
            "Keeps name/version/description (and repo/homepage); drops the registry envelope."
        }
        "gitlab" | "dockerhub" => {
            "Keeps name/description and stats (stars); drops the rest of the API object."
        }
        "wayback" => "Keeps the snapshot timestamp, original URL, and snapshot link.",
        "maps" => "Keeps display_name and lat/lon; drops the rest of the place record.",
        "weather" => "Keeps the current temperature/wind and place; drops hourly arrays.",
        "coinbase" => "Keeps amount/base/currency from the spot price.",
        "duckduckgo" => "Keeps the abstract/heading and each result's text.",
        "gnews" => "Keeps each item's title/link/description; drops feed boilerplate.",
        "statuspage" => "Keeps the overall status and per-component statuses.",
        "huggingface" => "Keeps id/task/downloads/likes; drops the rest of the model card JSON.",
        "openai" | "anthropic" => "Keeps the model id and owner/display name; drops the envelope.",
        "brave" | "bing" | "google" => {
            "Trims web results to title/url/snippet; drops ranking and ad metadata."
        }
        "notion" => "Keeps the page title and url; drops block/property internals.",
        "slack" => "Keeps message text, channel, and author; drops blocks and ids.",
        "flight" => "Keeps route/airline/status; drops the rest of the flight record.",
        "devto" => "Keeps title/description/body_markdown; drops the article envelope.",
        "openlibrary" => "Keeps title/author/description; drops the catalogue record.",
        "gutenberg" => "Keeps title/authors/subjects; drops download/format metadata.",
        "lobsters" => "Keeps title/url/tags/description; drops vote and user internals.",
        "wikidata" => "Keeps the entity label and description; drops claims and sitelinks.",
        _ => "Passes through (the response is already compact).",
    }
}

/// Build the (channel, rule) table for every registered channel.
pub fn rules() -> Vec<(String, &'static str)> {
    ChannelRegistry::default_channels()
        .list()
        .iter()
        .map(|c| (c.name().to_string(), rule_for(c.name())))
        .collect()
}

pub async fn run(args: FormatArgs) -> anyhow::Result<()> {
    match args.platform {
        Some(name) => {
            let registry = ChannelRegistry::default_channels();
            match registry.by_name(&name) {
                Some(_) => println!("{name}: {}", rule_for(&name)),
                None => {
                    eprintln!("unknown channel: {name}");
                    println!("Run 'agentspan format' to list all channels.");
                }
            }
        }
        None => {
            println!("Token-reduction rules (format_for_llm) per channel:\n");
            for (name, rule) in rules() {
                println!("  {name:<12} {rule}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_cover_every_channel() {
        let rules = rules();
        // Matches the default registry size.
        assert_eq!(rules.len(), 52);
        assert!(rules.iter().all(|(_, r)| !r.is_empty()));
    }

    #[test]
    fn known_channels_have_specific_rules() {
        assert!(rule_for("reddit").contains("awards"));
        assert!(rule_for("wikipedia").contains("extract"));
        assert!(rule_for("discord").contains("content"));
    }

    #[test]
    fn unknown_channel_gets_passthrough() {
        assert!(rule_for("does-not-exist").contains("Passes through"));
    }
}
