# Setup: Groq (free Whisper transcription)

Powers `agentspan transcribe` and the `xiaoyuzhou` podcast channel. Groq offers a
free Whisper endpoint; OpenAI is used as a fallback if configured.

## 1. Get a free Groq key
1. https://console.groq.com → sign up → **API Keys** → create a key (`gsk_...`).

## 2. Configure AgentSpan
Stored in config (not an env var) so transcription picks it up:

```bash
agentspan config set api_keys.groq gsk_xxxxxxxx
# optional fallback:
agentspan config set api_keys.openai sk-xxxxxxxx
```

## 3. Transcribe
```bash
agentspan transcribe "https://www.youtube.com/watch?v=..." -o transcript.txt
```

Requires `yt-dlp` (download) and, ideally, `ffmpeg` (compresses audio under the 25 MB
Whisper limit). Install both with `agentspan install`.
