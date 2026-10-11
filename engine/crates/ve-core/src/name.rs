//! Validated identifiers.

use std::fmt;
use std::str::FromStr;

/// Longest accepted name, in bytes. Keeps tool schemas and logs bounded.
pub const MAX_NAME_LEN: usize = 64;

/// A stable identifier such as `box`, `sd_union` or `anatomy.heart`.
///
/// Segments are `snake_case` ASCII, separated by single dots. Names are the keys that LLM tool
/// schemas, saved scenes and packs refer to, so they are validated once, at the boundary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(String);

/// Why a string is not a valid [`Name`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The input was empty.
    #[error("name is empty")]
    Empty,
    /// The input exceeded [`MAX_NAME_LEN`].
    #[error("name is {len} bytes, the limit is {MAX_NAME_LEN}")]
    TooLong {
        /// Length of the rejected input.
        len: usize,
    },
    /// A segment was empty, started with a digit, or contained a disallowed character.
    #[error(
        "invalid segment {segment:?}: use lowercase letters, digits and '_', not starting with a digit"
    )]
    BadSegment {
        /// The offending segment.
        segment: String,
    },
}

impl Name {
    /// Validates and wraps `s`.
    ///
    /// # Errors
    /// Returns a [`NameError`] describing the first problem found.
    pub fn new(s: &str) -> Result<Self, NameError> {
        if s.is_empty() {
            return Err(NameError::Empty);
        }
        if s.len() > MAX_NAME_LEN {
            return Err(NameError::TooLong { len: s.len() });
        }
        for segment in s.split('.') {
            let mut chars = segment.chars();
            let head_ok = chars
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
            let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !(head_ok && rest_ok) {
                return Err(NameError::BadSegment {
                    segment: segment.to_owned(),
                });
            }
        }
        Ok(Self(s.to_owned()))
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Name {
    type Err = NameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_and_dotted() {
        for ok in ["box", "sd_union", "anatomy.heart", "_internal", "a1.b2.c3"] {
            assert_eq!(Name::new(ok).map(|n| n.to_string()), Ok(ok.to_owned()));
        }
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(Name::new(""), Err(NameError::Empty));
        assert_eq!(
            Name::new(&"a".repeat(65)),
            Err(NameError::TooLong { len: 65 })
        );
        for bad in ["Box", "1box", "a..b", ".a", "a.", "a-b", "a b", "é"] {
            assert!(
                matches!(Name::new(bad), Err(NameError::BadSegment { .. })),
                "{bad}"
            );
        }
    }
}
