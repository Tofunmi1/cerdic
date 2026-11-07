//! Loop headers, binding patterns and matching end delimiters.

use super::{LIMIT, span, too_big};
use crate::diag::Span;
use crate::text::expr::{self, Env, evaluate};
use crate::text::lex::{Fail, syntax};

/// A declaration keyword followed by whitespace, keeping names such as `for = op()` usable.
pub(super) fn keyword<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(word)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    (!rest.starts_with('=')).then_some(rest)
}

pub(super) fn header(line: &str) -> Option<&str> {
    keyword(line, "for")
}

pub(super) fn clean(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

/// The condition of an `if` line, when `line` is one.
pub(super) fn condition(line: &str) -> Option<&str> {
    keyword(line, "if")
}

/// Find the matching end, rejecting group declarations even in an empty loop or branch. Each nested block is a loop (true) or an `if` (false);
/// only loops count towards the depth limit.
pub(super) fn end(lines: &[&str], start: usize, depth: usize) -> Result<usize, Fail> {
    let mut open: Vec<bool> = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        let text = clean(line);
        if keyword(text, "group").is_some() {
            return Err(syntax(
                span(index),
                "a group is not allowed inside a loop, an if or a macro",
            ));
        }
        if keyword(text, "def").is_some() {
            return Err(syntax(
                span(index),
                "a def goes at the top level, not inside a loop, an if or another def",
            ));
        }
        if header(text).is_some() {
            open.push(true);
            if depth + open.iter().filter(|is_loop| **is_loop).count() > 3 {
                return Err(syntax(span(index), "loops may nest to depth 3"));
            }
        } else if condition(text).is_some() {
            open.push(false);
        } else if text == "end" {
            if open.pop().is_none() {
                return Ok(index);
            }
        }
    }
    Err(syntax(span(start), "a loop or an if is missing its end"))
}

/// The `else` that belongs to the `if` at `start` and closes before `close`, if there is one.
pub(super) fn else_at(lines: &[&str], start: usize, close: usize) -> Result<Option<usize>, Fail> {
    let (mut nested, mut found) = (0_usize, None);
    for (index, line) in lines.iter().enumerate().take(close).skip(start + 1) {
        let text = clean(line);
        if header(text).is_some() || condition(text).is_some() {
            nested += 1;
        } else if text == "end" {
            nested = nested.saturating_sub(1);
        } else if text == "else" && nested == 0 {
            if found.is_some() {
                return Err(syntax(span(index), "an if has only one else"));
            }
            found = Some(index);
        }
    }
    Ok(found)
}

/// Split a list or binding pattern only at commas outside nested lists and expressions.
fn items(text: &str, at: Span) -> Result<Vec<&str>, Fail> {
    let Some(inner) = text
        .trim()
        .strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
    else {
        return Err(syntax(at, "expected a [list]"));
    };
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut depth = 0_i32;
    let mut start = 0;
    let mut quoted = false;
    let mut parts = Vec::new();
    for (index, ch) in inner.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            _ if quoted => {}
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return Err(syntax(at, "unbalanced loop list"));
        }
    }
    if depth != 0 {
        return Err(syntax(at, "unbalanced loop list"));
    }
    parts.push(inner[start..].trim());
    Ok(parts)
}

fn value(text: &str, at: Span, env: &Env) -> Result<f64, Fail> {
    let text = text.trim();
    evaluate(
        text.strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(text),
        at,
        env,
    )
}

/// One value a loop variable takes: a number, or a piece of text written in quotes.
#[derive(Clone, Debug)]
pub(in crate::text) enum Item {
    Num(f64),
    Text(String),
}

impl Item {
    /// How the value is written in an error's "in iteration" note.
    pub(in crate::text) fn shown(&self) -> String {
        match self {
            Self::Num(n) => expr::format_number(*n),
            Self::Text(t) => format!("{t:?}"),
        }
    }
}

fn item(text: &str, at: Span, env: &Env) -> Result<Item, Fail> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        if inner.contains(['"', '\\']) {
            return Err(syntax(
                at,
                "a loop text value has no quotes or backslashes inside",
            ));
        }
        return Ok(Item::Text(inner.to_owned()));
    }
    value(text, at, env).map(Item::Num)
}

pub(in crate::text) fn bindings(
    rest: &str,
    at: Span,
    env: &Env,
) -> Result<(Vec<String>, Vec<Vec<Item>>), Fail> {
    let Some((index, _)) = rest.match_indices("in").find(|(index, _)| {
        rest[..*index].ends_with(char::is_whitespace)
            && rest[*index + 2..].starts_with(char::is_whitespace)
    }) else {
        return Err(syntax(at, "write for name in A..B or for name in [list]"));
    };
    let (pattern, sequence) = (&rest[..index], &rest[index + 2..]);
    let pattern = pattern.trim();
    let names = if pattern.starts_with('[') {
        items(pattern, at)?
    } else {
        vec![pattern]
    };
    if names.is_empty() || names.len() > 3 || (pattern.starts_with('[') && names.len() < 2) {
        return Err(syntax(at, "bind one name, or a pair or triple of names"));
    }
    for (index, name) in names.iter().enumerate() {
        if !crate::text::build::valid_name(name)
            || expr::RESERVED.contains(name)
            || names[..index].contains(name)
        {
            return Err(syntax(at, format!("{name:?} cannot name a loop variable")));
        }
    }
    let sequence = clean(sequence);
    let mut rows = Vec::new();
    if sequence.starts_with('[') {
        for item in items(sequence, at)? {
            let row = if names.len() == 1 {
                vec![self::item(item, at, env)?]
            } else {
                items(item, at)?
                    .into_iter()
                    .map(|s| self::item(s, at, env))
                    .collect::<Result<Vec<_>, _>>()?
            };
            if row.len() != names.len() {
                return Err(syntax(at, "loop item does not match the binding pattern"));
            }
            rows.push(row);
            if rows.len() > LIMIT {
                return Err(too_big(at));
            }
        }
    } else if let Some((a, b)) = sequence.split_once("..") {
        let (a, b) = (value(a, at, env)?, value(b, at, env)?);
        if names.len() != 1 || a.fract() != 0.0 || b.fract() != 0.0 {
            return Err(syntax(at, "a range binds one name to whole numbers"));
        }
        if (b - a).max(0.0) > 2_000.0 {
            return Err(too_big(at));
        }
        for offset in 0..2_000_u32 {
            let next = a + f64::from(offset);
            if next >= b {
                break;
            }
            rows.push(vec![Item::Num(next)]);
        }
    } else {
        return Err(syntax(at, "expected A..B or a [list] after in"));
    }
    Ok((names.into_iter().map(str::to_owned).collect(), rows))
}
