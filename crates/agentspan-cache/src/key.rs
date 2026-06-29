//! Cache key generation.

use std::hash::{DefaultHasher, Hash, Hasher};

/// A deterministic cache key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey(pub String);

impl CacheKey {
    /// Build a key from a channel name, operation, and argument.
    pub fn new(channel: &str, op: &str, arg: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        channel.hash(&mut hasher);
        op.hash(&mut hasher);
        arg.hash(&mut hasher);
        Self(format!("{}:{}:{:x}", channel, op, hasher.finish()))
    }

    /// Build a key from a raw string.
    pub fn raw(key: impl Into<String>) -> Self {
        Self(key.into())
    }
}

impl From<String> for CacheKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_produce_same_key() {
        let a = CacheKey::new("web", "read", "https://example.com");
        let b = CacheKey::new("web", "read", "https://example.com");
        assert_eq!(a, b);
    }

    #[test]
    fn different_inputs_produce_different_keys() {
        let a = CacheKey::new("web", "read", "https://example.com");
        let b = CacheKey::new("web", "read", "https://example.org");
        assert_ne!(a, b);
    }
}
