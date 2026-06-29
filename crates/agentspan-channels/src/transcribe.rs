//! Whisper transcription: yt-dlp download → ffmpeg compress → Whisper API.
//!
//! Defaults to Groq's free `whisper-large-v3`, falling back to OpenAI `whisper-1`.
//! API keys are read from config `api_keys.groq` / `api_keys.openai`.

use std::path::{Path, PathBuf};

use agentspan_core::Config;

/// Whisper size limit is 25MB; leave headroom for multipart overhead.
const SIZE_LIMIT_BYTES: u64 = 24 * 1024 * 1024;

/// Errors raised by the transcription pipeline.
#[derive(Debug)]
pub enum TranscribeError {
    MissingDependency(String),
    NoProvider,
    Command(String),
    Http(String),
}

impl std::fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranscribeError::MissingDependency(b) => write!(f, "missing dependency: {b}"),
            TranscribeError::NoProvider => {
                write!(
                    f,
                    "no Whisper API key (set api_keys.groq or api_keys.openai)"
                )
            }
            TranscribeError::Command(m) => write!(f, "command failed: {m}"),
            TranscribeError::Http(m) => write!(f, "transcription request failed: {m}"),
        }
    }
}

impl std::error::Error for TranscribeError {}

/// A Whisper-compatible provider.
#[derive(Debug, Clone)]
pub struct Provider {
    pub name: &'static str,
    pub endpoint: String,
    pub model: &'static str,
    pub key: String,
}

/// Resolve configured providers in preference order (Groq → OpenAI).
pub fn providers_from_config(config: &Config) -> Vec<Provider> {
    let mut out = Vec::new();
    if let Some(key) = config.api_keys.get("groq") {
        out.push(Provider {
            name: "groq",
            endpoint: "https://api.groq.com/openai/v1/audio/transcriptions".to_string(),
            model: "whisper-large-v3",
            key: key.clone(),
        });
    }
    if let Some(key) = config.api_keys.get("openai") {
        out.push(Provider {
            name: "openai",
            endpoint: "https://api.openai.com/v1/audio/transcriptions".to_string(),
            model: "whisper-1",
            key: key.clone(),
        });
    }
    out
}

fn which(bin: &str) -> bool {
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let exts: &[&str] = if cfg!(windows) { &["", ".exe"] } else { &[""] };
    std::env::split_paths(&path).any(|d| exts.iter().any(|e| d.join(format!("{bin}{e}")).is_file()))
}

/// Transcribe a single audio file at a Whisper endpoint (the testable unit).
pub async fn transcribe_chunk_at(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    bytes: Vec<u8>,
    filename: &str,
) -> Result<String, TranscribeError> {
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str("audio/m4a")
        .map_err(|e| TranscribeError::Http(e.to_string()))?;
    let form = reqwest::multipart::Form::new()
        .text("model", model.to_string())
        .text("response_format", "text")
        .part("file", part);

    let resp = client
        .post(endpoint)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| TranscribeError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(TranscribeError::Http(format!("HTTP {}", resp.status())));
    }
    resp.text()
        .await
        .map_err(|e| TranscribeError::Http(e.to_string()))
}

async fn run(cmd: &str, args: &[&str]) -> Result<(), TranscribeError> {
    let status = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TranscribeError::MissingDependency(cmd.to_string())
            } else {
                TranscribeError::Command(e.to_string())
            }
        })?;
    if !status.status.success() {
        return Err(TranscribeError::Command(format!(
            "{cmd}: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        )));
    }
    Ok(())
}

/// Download audio for a URL with yt-dlp, returning the output file path.
async fn download_audio(url: &str, dir: &Path) -> Result<PathBuf, TranscribeError> {
    if !which("yt-dlp") {
        return Err(TranscribeError::MissingDependency("yt-dlp".to_string()));
    }
    let template = dir.join("source.%(ext)s");
    run(
        "yt-dlp",
        &[
            "-x",
            "--audio-format",
            "m4a",
            "-o",
            template.to_str().unwrap_or("source.m4a"),
            url,
        ],
    )
    .await?;
    std::fs::read_dir(dir)
        .ok()
        .and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some("source"))
        })
        .ok_or_else(|| TranscribeError::Command("yt-dlp produced no output".to_string()))
}

/// Compress to mono/16kHz/32kbps m4a so most content fits under 25MB.
async fn compress_audio(src: &Path, dir: &Path) -> Result<PathBuf, TranscribeError> {
    if !which("ffmpeg") {
        // ffmpeg is optional — fall back to the raw download.
        return Ok(src.to_path_buf());
    }
    let dst = dir.join("compressed.m4a");
    run(
        "ffmpeg",
        &[
            "-loglevel",
            "error",
            "-y",
            "-i",
            src.to_str().unwrap_or(""),
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-b:a",
            "32k",
            dst.to_str().unwrap_or(""),
        ],
    )
    .await?;
    Ok(dst)
}

/// Create an isolated, auto-cleaned working directory for one transcription.
fn work_dir() -> Result<tempfile::TempDir, TranscribeError> {
    tempfile::Builder::new()
        .prefix("agentspan-transcribe-")
        .tempdir()
        .map_err(|e| TranscribeError::Command(e.to_string()))
}

/// Transcribe a remote audio/video URL. Downloads, compresses, then posts to a
/// Whisper provider with Groq→OpenAI fallback.
pub async fn transcribe_url(url: &str, config: &Config) -> Result<String, TranscribeError> {
    let providers = providers_from_config(config);
    if providers.is_empty() {
        return Err(TranscribeError::NoProvider);
    }

    // Unique per call (no cross-call collisions); auto-deleted when `dir` drops.
    let dir = work_dir()?;
    let audio = download_audio(url, dir.path()).await?;
    let compressed = compress_audio(&audio, dir.path()).await?;

    let size = std::fs::metadata(&compressed).map(|m| m.len()).unwrap_or(0);
    if size > SIZE_LIMIT_BYTES {
        return Err(TranscribeError::Command(format!(
            "audio is {size} bytes (>25MB even after compression); chunking not yet supported"
        )));
    }
    let bytes = std::fs::read(&compressed).map_err(|e| TranscribeError::Command(e.to_string()))?;

    let client = crate::http::default_client();
    let mut last = TranscribeError::NoProvider;
    for p in &providers {
        match transcribe_chunk_at(
            &client,
            &p.endpoint,
            &p.key,
            p.model,
            bytes.clone(),
            "audio.m4a",
        )
        .await
        {
            Ok(text) => return Ok(text),
            Err(e) => last = e,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn providers_resolved_from_config() {
        let mut config = Config::default();
        assert!(providers_from_config(&config).is_empty());
        config.api_keys.insert("groq".into(), "gsk_x".into());
        config.api_keys.insert("openai".into(), "sk_y".into());
        let providers = providers_from_config(&config);
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].name, "groq"); // groq preferred
    }

    #[tokio::test]
    async fn transcribe_chunk_posts_and_returns_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello transcript"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let text = transcribe_chunk_at(
            &client,
            &format!("{}/audio/transcriptions", server.uri()),
            "key",
            "whisper-large-v3",
            b"fake audio".to_vec(),
            "audio.m4a",
        )
        .await
        .unwrap();
        assert_eq!(text, "hello transcript");
    }

    #[tokio::test]
    async fn transcribe_chunk_errors_on_http_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = transcribe_chunk_at(
            &client,
            &format!("{}/audio/transcriptions", server.uri()),
            "bad",
            "whisper-1",
            b"x".to_vec(),
            "a.m4a",
        )
        .await;
        assert!(result.is_err());
    }

    #[test]
    fn work_dirs_are_unique_and_auto_cleaned() {
        // M3 regression: each call gets its own dir (old code reused one per pid)
        // and the dir is removed on drop (old code leaked it).
        let a = work_dir().unwrap();
        let b = work_dir().unwrap();
        assert_ne!(a.path(), b.path());
        let path = a.path().to_path_buf();
        assert!(path.exists());
        drop(a);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn transcribe_url_without_provider_errors() {
        let config = Config::default();
        let result = transcribe_url("https://example.com/podcast", &config).await;
        assert!(matches!(result, Err(TranscribeError::NoProvider)));
    }
}
