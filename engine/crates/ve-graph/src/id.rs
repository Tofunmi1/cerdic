//! Stable identifiers for documents, nodes, materials and transactions.
//!
//! Document, node and material IDs are a prefix plus eight base32 characters (40 bits). They are
//! generated from a per-document counter through a bijection, so they are unique, deterministic
//! (replaying the same commands gives the same IDs) and never reused after deletion, because the
//! counter only moves forward.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Crockford-style lowercase base32 (no `i`, `l`, `o`, `u`).
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Characters after the prefix.
const BODY_LEN: usize = 8;

/// IDs carry 40 bits.
const MASK: u64 = (1 << 40) - 1;

/// An odd multiplier, so multiplying modulo 2^40 is a bijection.
const ODD: u64 = 0x9E37_79B9_7F4B_7C15;

/// The longest transaction key accepted, in bytes.
const MAX_TRANSACTION_LEN: usize = 64;

/// Why a string is not a valid identifier.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{value:?} is not a valid {kind} id")]
pub struct IdError {
    /// What the string was meant to be (`node`, `document`, ...).
    pub kind: &'static str,
    /// The rejected input, truncated to a readable length.
    pub value: String,
}

fn truncated(s: &str) -> String {
    s.chars().take(80).collect()
}

/// Encodes `counter` (offset by `salt`) as a prefixed ID body.
pub(crate) fn encode(prefix: &str, counter: u64, salt: u64) -> String {
    let mixed = counter.wrapping_add(salt).wrapping_mul(ODD) & MASK;
    let mut out = String::with_capacity(prefix.len() + BODY_LEN);
    out.push_str(prefix);
    for i in (0..BODY_LEN).rev() {
        let digit = usize::try_from((mixed >> (5 * i)) & 31).unwrap_or(0);
        out.push(char::from(ALPHABET.get(digit).copied().unwrap_or(b'0')));
    }
    out
}

fn valid_generated(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix)
        .is_some_and(|body| body.len() == BODY_LEN && body.bytes().all(|b| ALPHABET.contains(&b)))
}

macro_rules! generated_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal, $kind:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        #[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
        pub struct $name(String);

        impl $name {
            /// Validates and wraps `s`.
            ///
            /// # Errors
            /// [`IdError`] if `s` does not have the prefix and eight base32 characters.
            pub fn parse(s: &str) -> Result<Self, IdError> {
                if valid_generated(s, $prefix) {
                    Ok(Self(s.to_owned()))
                } else {
                    Err(IdError { kind: $kind, value: truncated(s) })
                }
            }

            /// The ID as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn generated(counter: u64, salt: u64) -> Self {
                Self(encode($prefix, counter, salt))
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;
            fn try_from(s: String) -> Result<Self, IdError> {
                Self::parse(&s)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

generated_id!(
    /// Identifies a document.
    DocId, "d_", "document"
);
generated_id!(
    /// Identifies a node; stable across edits, undo and reload.
    NodeId, "n_", "node"
);
generated_id!(
    /// Identifies a material definition.
    MaterialId, "m_", "material"
);

/// The idempotency key of a command batch: `t_` plus 1 to 64 characters from `[A-Za-z0-9_-]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
pub struct TransactionId(String);

impl TransactionId {
    /// Validates and wraps `s`.
    ///
    /// # Errors
    /// [`IdError`] if `s` is not `t_` followed by 1 to 64 allowed characters.
    pub fn parse(s: &str) -> Result<Self, IdError> {
        let ok = s.strip_prefix("t_").is_some_and(|body| {
            (1..=MAX_TRANSACTION_LEN).contains(&body.len())
                && body
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        });
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdError {
                kind: "transaction",
                value: truncated(s),
            })
        }
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TransactionId {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        Self::parse(&s)
    }
}

impl From<TransactionId> for String {
    fn from(id: TransactionId) -> String {
        id.0
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn generated_ids_are_unique_and_valid() {
        let ids: HashSet<String> = (0..50_000).map(|n| encode("n_", n, 12345)).collect();
        assert_eq!(ids.len(), 50_000);
        assert!(ids.iter().all(|s| NodeId::parse(s).is_ok()));
    }

    #[test]
    fn parsing_rejects_other_shapes() {
        for bad in [
            "",
            "n_",
            "n_1234567",
            "n_123456789",
            "x_12345678",
            "n_1234567u",
            "N_12345678",
        ] {
            assert!(NodeId::parse(bad).is_err(), "{bad}");
        }
        assert!(NodeId::parse("n_0123abcd").is_ok());
        assert!(DocId::parse("n_0123abcd").is_err());
    }

    #[test]
    fn transaction_keys_are_bounded() {
        assert!(TransactionId::parse("t_01HZ-abc_9").is_ok());
        assert!(TransactionId::parse("t_").is_err());
        assert!(TransactionId::parse("t_has space").is_err());
        assert!(TransactionId::parse(&format!("t_{}", "a".repeat(65))).is_err());
    }

    #[test]
    fn ids_survive_json() {
        let id = NodeId::generated(7, 1);
        let json = serde_json::to_string(&id).unwrap_or_default();
        assert_eq!(serde_json::from_str::<NodeId>(&json).ok(), Some(id));
        assert!(serde_json::from_str::<NodeId>("\"bogus\"").is_err());
    }
}
