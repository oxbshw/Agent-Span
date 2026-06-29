//! Content fingerprinting and change detection.
//!
//! Two cheap, deterministic fingerprints over a blob of text:
//!   * an exact 64-bit content hash (FNV-1a) — equal iff the bytes are equal;
//!   * a 64-bit **SimHash** over word tokens — *close* (small Hamming distance)
//!     when content is merely edited, far when it genuinely changed.
//!
//! Together they let an agent (or the cache layer) answer "did this page actually
//! change since I last read it, and by *how much*?" without diffing full bodies or
//! calling a model. This pairs with the HTTP conditional-revalidation path: an ETag
//! tells you the server *thinks* the bytes differ; the SimHash tells you whether the
//! content *meaningfully* did — so an agent can skip a re-read of a page that only
//! bumped a timestamp or rotated an ad.
//!
//! FNV-1a is used instead of [`std::collections::hash_map::DefaultHasher`] because
//! the latter is explicitly not guaranteed stable across releases; a fingerprint
//! that an agent persists must mean the same thing tomorrow.

use serde::Serialize;

/// A pair of fingerprints for a piece of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Fingerprint {
    /// Exact content hash (FNV-1a, 64-bit).
    pub hash: u64,
    /// Locality-sensitive hash; a small Hamming distance means similar content.
    pub simhash: u64,
}

/// How two fingerprints relate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Change {
    /// Byte-for-byte identical.
    pub identical: bool,
    /// Hamming distance between the two SimHashes (`0..=64`).
    pub hamming: u32,
    /// Similarity in `0.0..=1.0` (`1.0` == identical SimHash).
    pub similarity: f64,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit hash — stable across runs and platforms.
pub fn content_hash(text: &str) -> u64 {
    let mut h = FNV_OFFSET;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Lowercased alphanumeric tokens of length >= 2 (matches the ranking tokenizer).
fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_lowercase())
        .collect()
}

/// 64-bit SimHash over the token multiset (Charikar). Each token votes per bit by
/// its hash; the sign of the summed votes forms the fingerprint. Empty input
/// hashes to `0`.
pub fn simhash(text: &str) -> u64 {
    let toks = tokens(text);
    if toks.is_empty() {
        return 0;
    }
    let mut counts = [0i64; 64];
    for t in &toks {
        let h = content_hash(t);
        for (i, c) in counts.iter_mut().enumerate() {
            if (h >> i) & 1 == 1 {
                *c += 1;
            } else {
                *c -= 1;
            }
        }
    }
    let mut out = 0u64;
    for (i, c) in counts.iter().enumerate() {
        if *c > 0 {
            out |= 1u64 << i;
        }
    }
    out
}

/// Compute both fingerprints for `text`.
pub fn fingerprint(text: &str) -> Fingerprint {
    Fingerprint {
        hash: content_hash(text),
        simhash: simhash(text),
    }
}

/// Compare two fingerprints into a [`Change`].
pub fn diff(old: Fingerprint, new: Fingerprint) -> Change {
    let hamming = (old.simhash ^ new.simhash).count_ones();
    Change {
        identical: old.hash == new.hash,
        hamming,
        similarity: 1.0 - (hamming as f64 / 64.0),
    }
}

/// Did the content change past a similarity floor? `threshold` is in `0.0..=1.0`;
/// below it (and not byte-identical) we report a meaningful change.
pub fn changed_beyond(old: Fingerprint, new: Fingerprint, threshold: f64) -> bool {
    let c = diff(old, new);
    !c.identical && c.similarity < threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_deterministic_and_distinguishes() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
        assert_ne!(content_hash("hello"), content_hash("world"));
        assert_eq!(simhash(""), 0);
    }

    #[test]
    fn identical_text_has_identical_fingerprint() {
        let a =
            fingerprint("Rust is a systems language with memory safety and zero cost abstractions");
        let b =
            fingerprint("Rust is a systems language with memory safety and zero cost abstractions");
        let c = diff(a, b);
        assert!(c.identical);
        assert_eq!(c.hamming, 0);
        assert_eq!(c.similarity, 1.0);
    }

    #[test]
    fn small_edit_is_more_similar_than_unrelated_content() {
        let base =
            fingerprint("Rust is a systems language with memory safety and zero cost abstractions");
        let edited = fingerprint(
            "Rust is a systems language with memory safety and zero cost abstractions and great tooling",
        );
        let unrelated = fingerprint(
            "Bananas are a tropical fruit rich in potassium enjoyed worldwide by people",
        );

        let sim_edit = diff(base, edited);
        let sim_unrelated = diff(base, unrelated);

        assert!(!sim_edit.identical);
        assert!(
            sim_edit.similarity > sim_unrelated.similarity,
            "edit {} should be closer than unrelated {}",
            sim_edit.similarity,
            sim_unrelated.similarity
        );
        assert!(sim_edit.similarity >= 0.6);
    }

    #[test]
    fn changed_beyond_respects_threshold() {
        let a =
            fingerprint("the quick brown fox jumps over the lazy dog near the river bank today");
        let same =
            fingerprint("the quick brown fox jumps over the lazy dog near the river bank today");
        let very_different = fingerprint(
            "quantum chromodynamics describes the strong interaction between quarks gluons",
        );

        // Identical content is never "changed", regardless of threshold.
        assert!(!changed_beyond(a, same, 0.99));
        // Wildly different content trips a reasonable floor.
        assert!(changed_beyond(a, very_different, 0.85));
    }
}
