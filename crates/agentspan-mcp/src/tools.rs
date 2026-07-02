//! MCP tool catalogue and the channel/operation each maps to.

use serde_json::{json, Value};

/// The operation a tool performs against its channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Read,
    Search,
    Doctor,
}

/// One MCP tool, bound to a channel + operation.
#[derive(Debug, Clone, Copy)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub channel: &'static str,
    pub op: Op,
}

/// All tools exposed over MCP (91).
pub const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "web_read",
        description: "Read any web page as clean text",
        channel: "web",
        op: Op::Read,
    },
    ToolDef {
        name: "web_search",
        description: "Semantic web search across the whole internet",
        channel: "exa",
        op: Op::Search,
    },
    ToolDef {
        name: "exa_search",
        description: "Exa semantic search (alias of web_search)",
        channel: "exa",
        op: Op::Search,
    },
    ToolDef {
        name: "github_read",
        description: "Read a GitHub repo, issue, or PR",
        channel: "github",
        op: Op::Read,
    },
    ToolDef {
        name: "github_search",
        description: "Search GitHub repositories",
        channel: "github",
        op: Op::Search,
    },
    ToolDef {
        name: "youtube_subtitles",
        description: "Fetch a YouTube video's subtitles/metadata",
        channel: "youtube",
        op: Op::Read,
    },
    ToolDef {
        name: "youtube_search",
        description: "Search YouTube videos",
        channel: "youtube",
        op: Op::Search,
    },
    ToolDef {
        name: "tiktok_read",
        description: "Fetch a TikTok video's metadata and description",
        channel: "tiktok",
        op: Op::Read,
    },
    ToolDef {
        name: "instagram_read",
        description: "Read an Instagram post or profile",
        channel: "instagram",
        op: Op::Read,
    },
    ToolDef {
        name: "instagram_search",
        description: "Search Instagram profiles by username",
        channel: "instagram",
        op: Op::Search,
    },
    ToolDef {
        name: "twitter_read",
        description: "Read a tweet or timeline",
        channel: "twitter",
        op: Op::Read,
    },
    ToolDef {
        name: "twitter_search",
        description: "Search tweets",
        channel: "twitter",
        op: Op::Search,
    },
    ToolDef {
        name: "reddit_read",
        description: "Read a Reddit post and its comments",
        channel: "reddit",
        op: Op::Read,
    },
    ToolDef {
        name: "reddit_search",
        description: "Search Reddit posts",
        channel: "reddit",
        op: Op::Search,
    },
    ToolDef {
        name: "bilibili_read",
        description: "Read Bilibili video info",
        channel: "bilibili",
        op: Op::Read,
    },
    ToolDef {
        name: "bilibili_search",
        description: "Search Bilibili videos",
        channel: "bilibili",
        op: Op::Search,
    },
    ToolDef {
        name: "hn_read",
        description: "Read a Hacker News story and comments",
        channel: "hackernews",
        op: Op::Read,
    },
    ToolDef {
        name: "hn_search",
        description: "Search Hacker News",
        channel: "hackernews",
        op: Op::Search,
    },
    ToolDef {
        name: "rss_read",
        description: "Read an RSS/Atom feed",
        channel: "rss",
        op: Op::Read,
    },
    ToolDef {
        name: "wikipedia_read",
        description: "Read a Wikipedia article's plain-text extract",
        channel: "wikipedia",
        op: Op::Read,
    },
    ToolDef {
        name: "wikipedia_search",
        description: "Search Wikipedia articles",
        channel: "wikipedia",
        op: Op::Search,
    },
    ToolDef {
        name: "arxiv_read",
        description: "Read an arXiv paper's abstract and metadata",
        channel: "arxiv",
        op: Op::Read,
    },
    ToolDef {
        name: "arxiv_search",
        description: "Search arXiv papers",
        channel: "arxiv",
        op: Op::Search,
    },
    ToolDef {
        name: "discord_read",
        description: "Read recent messages from a Discord channel",
        channel: "discord",
        op: Op::Read,
    },
    ToolDef {
        name: "discord_search",
        description: "Search messages in a Discord guild",
        channel: "discord",
        op: Op::Search,
    },
    ToolDef {
        name: "telegram_read",
        description: "Read a Telegram channel/chat's info",
        channel: "telegram",
        op: Op::Read,
    },
    ToolDef {
        name: "telegram_search",
        description: "Search recent Telegram updates by keyword",
        channel: "telegram",
        op: Op::Search,
    },
    ToolDef {
        name: "spotify_read",
        description: "Read a Spotify track's details",
        channel: "spotify",
        op: Op::Read,
    },
    ToolDef {
        name: "spotify_search",
        description: "Search Spotify tracks",
        channel: "spotify",
        op: Op::Search,
    },
    ToolDef {
        name: "twitch_read",
        description: "Read a Twitch channel's live stream info",
        channel: "twitch",
        op: Op::Read,
    },
    ToolDef {
        name: "twitch_search",
        description: "Search Twitch channels",
        channel: "twitch",
        op: Op::Search,
    },
    ToolDef {
        name: "scholar_search",
        description: "Search academic papers on Google Scholar",
        channel: "scholar",
        op: Op::Search,
    },
    ToolDef {
        name: "podcasts_search",
        description: "Search podcasts by term via Podcast Index",
        channel: "podcasts",
        op: Op::Search,
    },
    ToolDef {
        name: "podcasts_read",
        description: "Read a podcast feed's recent episodes",
        channel: "podcasts",
        op: Op::Read,
    },
    ToolDef {
        name: "quora_read",
        description: "Read a Quora question/answer page",
        channel: "quora",
        op: Op::Read,
    },
    ToolDef {
        name: "quora_search",
        description: "Search Quora",
        channel: "quora",
        op: Op::Search,
    },
    ToolDef {
        name: "pinterest_read",
        description: "Read a Pinterest pin or board",
        channel: "pinterest",
        op: Op::Read,
    },
    ToolDef {
        name: "pinterest_search",
        description: "Search Pinterest pins",
        channel: "pinterest",
        op: Op::Search,
    },
    ToolDef {
        name: "npm_read",
        description: "Read an npm package's metadata",
        channel: "npm",
        op: Op::Read,
    },
    ToolDef {
        name: "npm_search",
        description: "Search the npm registry",
        channel: "npm",
        op: Op::Search,
    },
    ToolDef {
        name: "crates_read",
        description: "Read a Rust crate's metadata from crates.io",
        channel: "crates",
        op: Op::Read,
    },
    ToolDef {
        name: "crates_search",
        description: "Search crates.io",
        channel: "crates",
        op: Op::Search,
    },
    ToolDef {
        name: "pypi_read",
        description: "Read a Python package's metadata from PyPI",
        channel: "pypi",
        op: Op::Read,
    },
    ToolDef {
        name: "pypi_search",
        description: "Resolve a Python package by name on PyPI",
        channel: "pypi",
        op: Op::Search,
    },
    ToolDef {
        name: "gitlab_read",
        description: "Read a GitLab project's metadata",
        channel: "gitlab",
        op: Op::Read,
    },
    ToolDef {
        name: "gitlab_search",
        description: "Search public GitLab projects",
        channel: "gitlab",
        op: Op::Search,
    },
    ToolDef {
        name: "dockerhub_read",
        description: "Read a Docker image's metadata",
        channel: "dockerhub",
        op: Op::Read,
    },
    ToolDef {
        name: "dockerhub_search",
        description: "Search Docker Hub images",
        channel: "dockerhub",
        op: Op::Search,
    },
    ToolDef {
        name: "wayback_read",
        description: "Get the latest Wayback Machine snapshot of a URL",
        channel: "wayback",
        op: Op::Read,
    },
    ToolDef {
        name: "wayback_search",
        description: "List Wayback Machine snapshots of a URL",
        channel: "wayback",
        op: Op::Search,
    },
    ToolDef {
        name: "maps_read",
        description: "Geocode a place to its best match",
        channel: "maps",
        op: Op::Read,
    },
    ToolDef {
        name: "maps_search",
        description: "Geocode a place to candidate locations",
        channel: "maps",
        op: Op::Search,
    },
    ToolDef {
        name: "weather_read",
        description: "Get current weather for a place",
        channel: "weather",
        op: Op::Read,
    },
    ToolDef {
        name: "weather_search",
        description: "Find locations for a weather query",
        channel: "weather",
        op: Op::Search,
    },
    ToolDef {
        name: "coinbase_read",
        description: "Get a crypto spot price (e.g. BTC-USD)",
        channel: "coinbase",
        op: Op::Read,
    },
    ToolDef {
        name: "coinbase_search",
        description: "Look up a crypto spot price by ticker",
        channel: "coinbase",
        op: Op::Search,
    },
    ToolDef {
        name: "duckduckgo_search",
        description: "DuckDuckGo instant-answer web search",
        channel: "duckduckgo",
        op: Op::Search,
    },
    ToolDef {
        name: "duckduckgo_read",
        description: "Get the DuckDuckGo instant-answer abstract for a query",
        channel: "duckduckgo",
        op: Op::Read,
    },
    ToolDef {
        name: "gnews_search",
        description: "Search Google News headlines",
        channel: "gnews",
        op: Op::Search,
    },
    ToolDef {
        name: "gnews_read",
        description: "Read Google News headlines for a query or feed",
        channel: "gnews",
        op: Op::Read,
    },
    ToolDef {
        name: "statuspage_read",
        description: "Read a service's status from its Statuspage",
        channel: "statuspage",
        op: Op::Read,
    },
    ToolDef {
        name: "statuspage_search",
        description: "Get a service's current status line",
        channel: "statuspage",
        op: Op::Search,
    },
    ToolDef {
        name: "huggingface_read",
        description: "Read a Hugging Face model's metadata",
        channel: "huggingface",
        op: Op::Read,
    },
    ToolDef {
        name: "huggingface_search",
        description: "Search models on the Hugging Face Hub",
        channel: "huggingface",
        op: Op::Search,
    },
    ToolDef {
        name: "openai_read",
        description: "Look up an OpenAI model by id (needs OPENAI_API_KEY)",
        channel: "openai",
        op: Op::Read,
    },
    ToolDef {
        name: "openai_search",
        description: "List/filter OpenAI models (needs OPENAI_API_KEY)",
        channel: "openai",
        op: Op::Search,
    },
    ToolDef {
        name: "anthropic_read",
        description: "Look up an Anthropic model by id (needs ANTHROPIC_API_KEY)",
        channel: "anthropic",
        op: Op::Read,
    },
    ToolDef {
        name: "anthropic_search",
        description: "List/filter Anthropic models (needs ANTHROPIC_API_KEY)",
        channel: "anthropic",
        op: Op::Search,
    },
    ToolDef {
        name: "brave_search",
        description: "Web search via Brave (needs BRAVE_API_KEY)",
        channel: "brave",
        op: Op::Search,
    },
    ToolDef {
        name: "brave_read",
        description: "Top Brave web result for a query",
        channel: "brave",
        op: Op::Read,
    },
    ToolDef {
        name: "bing_search",
        description: "Web search via Bing (needs BING_API_KEY)",
        channel: "bing",
        op: Op::Search,
    },
    ToolDef {
        name: "bing_read",
        description: "Top Bing web result for a query",
        channel: "bing",
        op: Op::Read,
    },
    ToolDef {
        name: "google_search",
        description: "Web search via Google Custom Search (needs GOOGLE_API_KEY + GOOGLE_CSE_ID)",
        channel: "google",
        op: Op::Search,
    },
    ToolDef {
        name: "google_read",
        description: "Top Google web result for a query",
        channel: "google",
        op: Op::Read,
    },
    ToolDef {
        name: "notion_search",
        description: "Search a Notion workspace (needs NOTION_API_KEY)",
        channel: "notion",
        op: Op::Search,
    },
    ToolDef {
        name: "notion_read",
        description: "Top Notion page matching a query",
        channel: "notion",
        op: Op::Read,
    },
    ToolDef {
        name: "slack_search",
        description: "Search Slack messages (needs SLACK_TOKEN)",
        channel: "slack",
        op: Op::Search,
    },
    ToolDef {
        name: "slack_read",
        description: "Top Slack message matching a query",
        channel: "slack",
        op: Op::Read,
    },
    ToolDef {
        name: "flight_search",
        description: "Flight status by IATA code (needs AVIATIONSTACK_KEY)",
        channel: "flight",
        op: Op::Search,
    },
    ToolDef {
        name: "flight_read",
        description: "Top flight leg for an IATA code",
        channel: "flight",
        op: Op::Read,
    },
    ToolDef {
        name: "devto_search",
        description: "List DEV Community articles by tag",
        channel: "devto",
        op: Op::Search,
    },
    ToolDef {
        name: "devto_read",
        description: "Read a dev.to article by URL",
        channel: "devto",
        op: Op::Read,
    },
    ToolDef {
        name: "openlibrary_search",
        description: "Search books via Open Library",
        channel: "openlibrary",
        op: Op::Search,
    },
    ToolDef {
        name: "openlibrary_read",
        description: "Read an Open Library work by URL",
        channel: "openlibrary",
        op: Op::Read,
    },
    ToolDef {
        name: "gutenberg_search",
        description: "Search public-domain books (Project Gutenberg)",
        channel: "gutenberg",
        op: Op::Search,
    },
    ToolDef {
        name: "gutenberg_read",
        description: "Read a Project Gutenberg book by URL",
        channel: "gutenberg",
        op: Op::Read,
    },
    ToolDef {
        name: "lobsters_search",
        description: "Search Lobsters (lobste.rs) stories",
        channel: "lobsters",
        op: Op::Search,
    },
    ToolDef {
        name: "lobsters_read",
        description: "Read a Lobsters story by URL",
        channel: "lobsters",
        op: Op::Read,
    },
    ToolDef {
        name: "wikidata_search",
        description: "Search Wikidata entities",
        channel: "wikidata",
        op: Op::Search,
    },
    ToolDef {
        name: "wikidata_read",
        description: "Read a Wikidata entity by URL",
        channel: "wikidata",
        op: Op::Read,
    },
    ToolDef {
        name: "doctor",
        description: "Report channel health and active backends",
        channel: "",
        op: Op::Doctor,
    },
];

/// Look up a tool by name.
pub fn find(name: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|t| t.name == name)
}

fn input_schema(op: Op) -> Value {
    match op {
        Op::Read => json!({
            "type": "object",
            "properties": { "url": { "type": "string", "description": "The URL to read" } },
            "required": ["url"]
        }),
        Op::Search => json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" },
                "limit": { "type": "integer", "description": "Max results (default 10)" }
            },
            "required": ["query"]
        }),
        Op::Doctor => json!({ "type": "object", "properties": {} }),
    }
}

/// The `tools/list` payload.
pub fn tool_schemas() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": input_schema(t.op),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_at_least_30_tools() {
        assert!(TOOLS.len() >= 30, "expected 30+ tools, got {}", TOOLS.len());
    }

    #[test]
    fn new_channel_tools_are_present() {
        for name in [
            "wikipedia_search",
            "arxiv_read",
            "discord_read",
            "telegram_read",
            "spotify_search",
            "twitch_search",
            "scholar_search",
            "podcasts_search",
            "quora_read",
            "pinterest_search",
            "npm_read",
            "crates_search",
            "pypi_search",
            "gitlab_search",
            "dockerhub_read",
            "wayback_read",
            "maps_search",
            "weather_read",
            "coinbase_read",
            "duckduckgo_search",
            "gnews_search",
            "statuspage_read",
            "huggingface_search",
            "openai_search",
            "anthropic_search",
        ] {
            assert!(find(name).is_some(), "missing tool: {name}");
        }
    }

    #[test]
    fn tool_names_are_unique() {
        let mut names: Vec<_> = TOOLS.iter().map(|t| t.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate tool names present");
    }

    #[test]
    fn schemas_have_names_and_input() {
        let schemas = tool_schemas();
        assert_eq!(schemas.len(), TOOLS.len());
        for s in &schemas {
            assert!(s["name"].is_string());
            assert!(s["inputSchema"]["type"] == "object");
        }
    }

    #[test]
    fn find_known_and_unknown() {
        assert_eq!(find("web_read").unwrap().channel, "web");
        assert_eq!(find("doctor").unwrap().op, Op::Doctor);
        assert!(find("nope").is_none());
    }

    #[test]
    fn read_schema_requires_url() {
        assert_eq!(input_schema(Op::Read)["required"][0], "url");
        assert_eq!(input_schema(Op::Search)["required"][0], "query");
    }
}
