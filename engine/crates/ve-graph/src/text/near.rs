//! Close names for text diagnostics, ranked by edit distance and then alphabetically.

/// Levenshtein distance in characters (a transposition takes two edits).
fn distance(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = i;
        row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row.last().copied().unwrap_or(0)
}

/// At most three distinct candidates, within two edits or containing/contained by the name.
pub(super) fn closest<'a>(
    wanted: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<&'a str> {
    let mut scored: Vec<_> = candidates
        .into_iter()
        .map(|name| (distance(wanted, name), name))
        .filter(|(edits, name)| {
            *edits <= 2
                || (!wanted.is_empty()
                    && !name.is_empty()
                    && (wanted.contains(name) || name.contains(wanted)))
        })
        .collect();
    scored.sort_unstable();
    scored.dedup();
    scored.into_iter().take(3).map(|(_, name)| name).collect()
}

/// Prepend close names to an existing hint, retaining it unchanged when nothing is close.
pub(super) fn suggestion<'a>(
    wanted: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    fallback: Option<String>,
) -> Option<String> {
    let names = closest(wanted, candidates);
    if names.is_empty() {
        return fallback;
    }
    let quoted: Vec<_> = names.iter().map(|name| format!("`{name}`")).collect();
    let mut hint = format!("Did you mean {}?", quoted.join(" or "));
    if let Some(fallback) = fallback {
        hint.push(' ');
        hint.push_str(&fallback);
    }
    Some(hint)
}

#[cfg(test)]
mod tests {
    use super::{closest, distance, suggestion};

    #[test]
    fn counts_insertions_deletions_substitutions_and_transpositions() {
        for (a, b, edits) in [
            ("", "", 0),
            ("", "box", 3),
            ("size", "size", 0),
            ("sizes", "size", 1),
            ("bx", "box", 1),
            ("size", "side", 1),
            ("psot", "post", 2),
            ("café", "cafe", 1),
        ] {
            assert_eq!(distance(a, b), edits);
            assert_eq!(distance(b, a), edits);
        }
    }

    #[test]
    fn ranks_deduplicates_and_limits_candidates() {
        assert_eq!(
            closest("cat", ["hat", "cut", "cot", "bat", "cat", "cat"]),
            ["cat", "bat", "cot"]
        );
        assert_eq!(closest("psot", ["post", "zzz"]), ["post"]);
        assert_eq!(closest("cat", ["dog"]), Vec::<&str>::new());
    }

    #[test]
    fn accepts_prefixes_and_substrings_beyond_two_edits() {
        assert_eq!(closest("solid", ["solid.box"]), ["solid.box"]);
        assert_eq!(closest("box", ["solid.box"]), ["solid.box"]);
        assert_eq!(closest("sizes_extra", ["size"]), ["size"]);
        assert!(closest("", ["solid.box"]).is_empty());
    }

    #[test]
    fn preserves_fallbacks_and_starts_close_hints_with_did_you_mean() {
        assert_eq!(
            suggestion("sizes", ["size"], Some("takes: size".to_owned())).as_deref(),
            Some("Did you mean `size`? takes: size")
        );
        assert_eq!(suggestion("zzzzzz", ["size"], None), None);
        assert_eq!(
            suggestion("zzzzzz", ["size"], Some("takes: size".to_owned())).as_deref(),
            Some("takes: size")
        );
    }
}
