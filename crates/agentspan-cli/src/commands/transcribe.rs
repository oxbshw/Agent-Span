//! `agentspan transcribe <url>` — download audio/video and transcribe via Whisper.
//!
//! Thin CLI wrapper over [`agentspan_channels::transcribe::transcribe_url`], which
//! downloads with yt-dlp, compresses with ffmpeg, and posts to Groq (free) with an
//! OpenAI fallback. Keys come from config `api_keys.groq` / `api_keys.openai`.

use std::path::PathBuf;

use agentspan_channels::transcribe::transcribe_url;
use agentspan_core::Config;
use clap::Args;

#[derive(Args)]
pub struct TranscribeArgs {
    /// Audio/video URL (YouTube, podcast, or any yt-dlp-supported source).
    pub url: String,
    /// Write the transcript to a file instead of stdout.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub async fn run(args: TranscribeArgs) -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();
    match transcribe_url(&args.url, &config).await {
        Ok(text) => {
            match &args.output {
                Some(path) => {
                    std::fs::write(path, format!("{text}\n"))?;
                    println!("✅ Transcript written to {}", path.display());
                }
                None => println!("{text}"),
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ {e}");
            std::process::exit(1);
        }
    }
}
