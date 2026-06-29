//! Error types.

use thiserror::Error;

/// The top-level AgentSpan error type.
#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("rate limited")]
    RateLimited,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors that can occur within a channel implementation.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ChannelError {
    #[error("channel not found: {0}")]
    NotFound(String),

    #[error("unsupported URL: {0}")]
    UnsupportedUrl(String),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("auth required: {0}")]
    AuthRequired(String),

    #[error("rate limited")]
    RateLimited,

    #[error("timeout after {0}s")]
    Timeout(u64),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("other channel error: {0}")]
    Other(String),
}

/// Errors that can occur within a backend adapter.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum BackendError {
    #[error("backend {0}: command not found")]
    CommandNotFound(String),

    #[error("backend {0}: command failed with {1}")]
    CommandFailed(String, String),

    #[error("backend {0}: request failed with {1}")]
    RequestFailed(String, String),

    #[error("backend {0}: parse error: {1}")]
    Parse(String, String),

    #[error("backend {0}: timeout")]
    Timeout(String),

    #[error("backend {0}: auth required")]
    AuthRequired(String),

    #[error("backend {0}: not found")]
    NotFound(String),

    #[error("backend {0}: {1}")]
    Other(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_error_display() {
        let e = ChannelError::UnsupportedUrl("ftp://example.com".to_string());
        assert_eq!(e.to_string(), "unsupported URL: ftp://example.com");
    }

    #[test]
    fn backend_error_display() {
        let e = BackendError::CommandNotFound("gh".to_string());
        assert!(e.to_string().contains("command not found"));
    }
}
