//! Offline lexical feature hashing for memory retrieval.

/// Maps text to a fixed dimensional vector.
pub trait Embedder {
    /// Returns the output vector dimension.
    fn dim(&self) -> usize;
    /// Embeds text deterministically.
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Deterministic word and adjacent-word-pair hashing embedder.
///
/// Tokens are lower-cased and split on non-alphanumeric characters. The fixed English stop-word
/// list is: `the a an of and to in on at is was me my i you it he she they we who what this that
/// for with from as are be have has do does`. Words longer than three characters lose one trailing
/// `s`, so `owes` and `owe` share a feature. Word features have weight 1 and adjacent pairs weight
/// 0.5; signed FNV-1a buckets are then L2-normalised.
#[derive(Clone, Debug)]
pub struct LexicalEmbedder {
    dim: usize,
}

impl Default for LexicalEmbedder {
    fn default() -> Self {
        Self::new(256)
    }
}

impl LexicalEmbedder {
    /// Creates an embedder with `dim` buckets. A zero dimension produces an empty vector.
    #[must_use]
    pub const fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl Embedder for LexicalEmbedder {
    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut out = vec![0.0; self.dim];
        if self.dim == 0 {
            return out;
        }
        let words: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty() && !is_stop(w))
            .map(stem)
            .collect();
        for word in &words {
            add_feature(&mut out, word.as_bytes(), 1.0);
        }
        for pair in words.windows(2) {
            let mut feature = pair[0].as_bytes().to_vec();
            feature.push(0xff);
            feature.extend_from_slice(pair[1].as_bytes());
            add_feature(&mut out, &feature, 0.5);
        }
        let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut out {
                *v /= norm;
            }
        }
        out
    }
}

fn is_stop(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "a"
            | "an"
            | "of"
            | "and"
            | "to"
            | "in"
            | "on"
            | "at"
            | "is"
            | "was"
            | "me"
            | "my"
            | "i"
            | "you"
            | "it"
            | "he"
            | "she"
            | "they"
            | "we"
            | "who"
            | "what"
            | "this"
            | "that"
            | "for"
            | "with"
            | "from"
            | "as"
            | "are"
            | "be"
            | "have"
            | "has"
            | "do"
            | "does"
    )
}

fn stem(word: &str) -> String {
    if word.len() > 3 && word.ends_with('s') {
        word[..word.len() - 1].to_owned()
    } else {
        word.to_owned()
    }
}

fn add_feature(out: &mut [f32], feature: &[u8], weight: f32) {
    let hash = fnv(feature, 0xcbf2_9ce4_8422_2325);
    let bucket = usize::try_from(hash % u64::try_from(out.len()).unwrap_or(u64::MAX)).unwrap_or(0);
    let sign = if fnv(feature, 0x8422_2325_cbf2_9ce4) & (1 << 63) == 0 {
        1.0
    } else {
        -1.0
    };
    out[bucket] += sign * weight;
}

fn fnv(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
