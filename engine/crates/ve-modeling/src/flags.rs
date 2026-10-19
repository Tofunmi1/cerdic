//! Named on/off flags for behaviours (ADR 0029): what a behaviour sets, and what it needs before it acts.
//!
//! Flags are runtime state shared by the whole scene. They start off. A behaviour may change flags when it fires
//! (`sets`), may refuse to fire unless flags are in a given state (`needs`), and a part may react when a flag
//! changes (`trigger="flag"`, `watch`). The text forms are parsed here so the node, the tests and the viewer agree.
use serde::{Deserialize, Serialize};

use crate::{Fail, fail};

/// Most flags one behaviour may set or need.
pub const MAX_PER_BEHAVIOR: usize = 8;
/// Longest flag name.
pub const MAX_NAME: usize = 24;
/// Most distinct flags a scene may use at once.
pub const MAX_FLAGS: usize = 32;

/// How a behaviour changes a flag when it fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// Turn it on.
    On,
    /// Turn it off.
    Off,
    /// Flip it.
    Toggle,
}

/// One flag change a behaviour makes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagSet {
    /// The flag.
    pub name: String,
    /// What happens to it.
    pub change: Change,
}

/// One condition a behaviour needs: the flag must be in this state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Need {
    /// The flag.
    pub name: String,
    /// The state it must be in.
    pub value: bool,
}

/// A flag name: lowercase letters, digits and underscores, starting with a letter.
///
/// # Errors
/// `behavior.flag_name` when the text is not a valid name.
pub fn name(text: &str) -> Result<String, Fail> {
    let text = text.trim();
    let mut chars = text.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && text.len() <= MAX_NAME;
    if valid {
        Ok(text.to_owned())
    } else {
        Err(fail(
            "behavior.flag_name",
            format!(
                "flag name \"{text}\" must be lowercase letters, digits and underscores, start with a letter, and be at most {MAX_NAME} long"
            ),
        ))
    }
}

fn entries(text: &str) -> impl Iterator<Item = &str> {
    text.split([';', ','])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
}

/// Parses `sets`: `name=on`, `name=off`, `name=toggle` (a bare `name` means on), separated by `;` or `,`.
///
/// # Errors
/// A bad name or change, a repeated flag, or more than [`MAX_PER_BEHAVIOR`] entries.
pub fn parse_sets(text: &str) -> Result<Vec<FlagSet>, Fail> {
    let mut out: Vec<FlagSet> = Vec::new();
    for entry in entries(text) {
        let (flag, change) = match entry.split_once('=') {
            None => (entry, Change::On),
            Some((flag, how)) => (
                flag,
                match how.trim() {
                    "on" | "true" => Change::On,
                    "off" | "false" => Change::Off,
                    "toggle" => Change::Toggle,
                    other => {
                        return Err(fail(
                            "behavior.flag_change",
                            format!("sets entry \"{entry}\": \"{other}\" is not on, off or toggle"),
                        ));
                    }
                },
            ),
        };
        let flag = name(flag)?;
        if out.iter().any(|set| set.name == flag) {
            return Err(fail(
                "behavior.flag_change",
                format!("sets names flag \"{flag}\" twice"),
            ));
        }
        out.push(FlagSet { name: flag, change });
    }
    limit(out.len(), "sets")?;
    Ok(out)
}

/// Parses `needs`: `name` (must be on), `!name` or `name=off` (must be off), `name=on`.
///
/// # Errors
/// A bad name or state, a repeated flag, or more than [`MAX_PER_BEHAVIOR`] entries.
pub fn parse_needs(text: &str) -> Result<Vec<Need>, Fail> {
    let mut out: Vec<Need> = Vec::new();
    for entry in entries(text) {
        let (flag, value) = if let Some(rest) = entry.strip_prefix('!') {
            (rest, false)
        } else if let Some((flag, how)) = entry.split_once('=') {
            match how.trim() {
                "on" | "true" => (flag, true),
                "off" | "false" => (flag, false),
                other => {
                    return Err(fail(
                        "behavior.flag_need",
                        format!("needs entry \"{entry}\": \"{other}\" is not on or off"),
                    ));
                }
            }
        } else {
            (entry, true)
        };
        let flag = name(flag)?;
        if out.iter().any(|need| need.name == flag) {
            return Err(fail(
                "behavior.flag_need",
                format!("needs names flag \"{flag}\" twice"),
            ));
        }
        out.push(Need { name: flag, value });
    }
    limit(out.len(), "needs")?;
    Ok(out)
}

fn limit(count: usize, what: &str) -> Result<(), Fail> {
    if count > MAX_PER_BEHAVIOR {
        return Err(fail(
            "behavior.flag_limit",
            format!("{what} names {count} flags; a behaviour may name at most {MAX_PER_BEHAVIOR}"),
        ));
    }
    Ok(())
}

/// The change that undoes `change` when a held part is let go: on and off swap, toggle stays toggle.
#[must_use]
pub fn reverse(change: Change) -> Change {
    match change {
        Change::On => Change::Off,
        Change::Off => Change::On,
        Change::Toggle => Change::Toggle,
    }
}

/// The new value of a flag after `change`.
#[must_use]
pub fn apply(change: Change, current: bool) -> bool {
    match change {
        Change::On => true,
        Change::Off => false,
        Change::Toggle => !current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_and_needs_parse_and_refuse_bad_text() {
        let sets = parse_sets("power=on; door_open=toggle, alarm=off, key").unwrap_or_default();
        assert_eq!(sets.len(), 4);
        assert_eq!(sets[1].change, Change::Toggle);
        assert_eq!(
            sets[3],
            FlagSet {
                name: "key".into(),
                change: Change::On
            },
            "a bare name means on"
        );
        let needs = parse_needs("key; !alarm, power=on, lid=off").unwrap_or_default();
        assert_eq!(
            needs.iter().map(|n| n.value).collect::<Vec<_>>(),
            [true, false, true, false]
        );
        for bad in [
            "Power=on",
            "1x",
            "a=maybe",
            "a=on; a=off",
            "x y",
            "has-dash",
        ] {
            assert!(parse_sets(bad).is_err(), "{bad:?} is refused as a set");
        }
        assert!(parse_needs("a=toggle").is_err() && parse_needs("a; a").is_err());
        let nine = (0..9)
            .map(|i| format!("f{i}"))
            .collect::<Vec<_>>()
            .join(";");
        assert!(parse_sets(&nine).is_err(), "too many flags");
        assert!(parse_sets("").unwrap_or_default().is_empty());
    }

    #[test]
    fn changes_apply() {
        assert!(apply(Change::On, false) && !apply(Change::Off, true));
        assert!(apply(Change::Toggle, false) && !apply(Change::Toggle, true));
    }
}
