//! The condition of an `if` line: comparisons of numbers (`i < 3`, `w * 2 >= 1.5`), comparisons of text (`side == "l"`), joined by `&&` and `||`, negated
//! with `!`, grouped with parentheses. A lone number is true when it is not zero. Everything is decided while the text is read, from constants and loop
//! variables, so a condition never depends on the model.
use super::{Env, Texts, evaluate, problem};
use crate::diag::Span;
use crate::text::lex::Fail;

/// The most nested parentheses a condition may have.
const MAX_DEPTH: usize = 8;

/// Splits `text` at `separator` where it stands outside quotes and brackets.
fn split_top<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    let (mut depth, mut start, mut quoted) = (0_i32, 0, false);
    let mut parts = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        let c = rest.chars().next().unwrap_or(' ');
        match c {
            '"' => quoted = !quoted,
            _ if quoted => {}
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 && rest.starts_with(separator) => {
                parts.push(text[start..at].trim());
                at += separator.len();
                start = at;
                continue;
            }
            _ => {}
        }
        at += c.len_utf8();
    }
    parts.push(text[start..].trim());
    parts
}

/// Where the comparison operator of `atom` is, and which one (two-character operators first).
fn comparison(atom: &str) -> Option<(usize, &'static str)> {
    let (mut depth, mut quoted) = (0_i32, false);
    let bytes = atom.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => quoted = !quoted,
            _ if quoted => {}
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ if depth == 0 && atom.is_char_boundary(at) => {
                for op in ["==", "!=", "<=", ">=", "<", ">"] {
                    if atom[at..].starts_with(op) {
                        return Some((at, op));
                    }
                }
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// The text a side of a comparison stands for: a quoted literal or a text constant; `None` for a number.
fn text_of(side: &str, texts: &Texts) -> Option<String> {
    let side = side.trim();
    if let Some(inner) = side.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Some(inner.to_owned());
    }
    texts.get(side).cloned()
}

fn encloses(atom: &str) -> bool {
    if !(atom.starts_with('(') && atom.ends_with(')')) {
        return false;
    }
    let mut depth = 0_i32;
    for (index, c) in atom.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index != atom.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn atom(text: &str, span: Span, env: &Env, texts: &Texts, depth: usize) -> Result<bool, Fail> {
    let text = text.trim();
    if depth > MAX_DEPTH {
        return Err(problem(
            span,
            "the condition nests too deeply",
            "simplify it with a constant: let ok = ...",
        ));
    }
    if text.is_empty() {
        return Err(problem(
            span,
            "the condition is empty",
            "write a comparison, for example: if i < 3",
        ));
    }
    if let Some(rest) = text.strip_prefix('!').filter(|r| !r.starts_with('=')) {
        return Ok(!atom(rest, span, env, texts, depth + 1)?);
    }
    if encloses(text) {
        return condition(&text[1..text.len() - 1], span, env, texts, depth + 1);
    }
    let Some((at, op)) = comparison(text) else {
        return Ok(evaluate(text, span, env)? != 0.0);
    };
    let (left, right) = (&text[..at], &text[at + op.len()..]);
    let (left_text, right_text) = (text_of(left, texts), text_of(right, texts));
    if left_text.is_some() || right_text.is_some() {
        let (Some(a), Some(b)) = (left_text, right_text) else {
            return Err(problem(
                span,
                "a text can only be compared with another text",
                "compare text with text: side == \"l\"",
            ));
        };
        return match op {
            "==" => Ok(a == b),
            "!=" => Ok(a != b),
            _ => Err(problem(
                span,
                format!("text cannot be compared with {op}"),
                "use == or != for text",
            )),
        };
    }
    let (a, b) = (evaluate(left, span, env)?, evaluate(right, span, env)?);
    let close = (a - b).abs() < 1e-9;
    Ok(match op {
        "==" => close,
        "!=" => !close,
        "<=" => a < b || close,
        ">=" => a > b || close,
        "<" => a < b && !close,
        _ => a > b && !close,
    })
}

fn condition(text: &str, span: Span, env: &Env, texts: &Texts, depth: usize) -> Result<bool, Fail> {
    for either in split_top(text, "||") {
        let mut all = true;
        for each in split_top(either, "&&") {
            if !atom(each, span, env, texts, depth)? {
                all = false;
                break;
            }
        }
        if all {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether the condition `text` holds, for the constants and loop variables in `env` and `texts`.
pub(in crate::text) fn truth(
    text: &str,
    span: Span,
    env: &Env,
    texts: &Texts,
) -> Result<bool, Fail> {
    condition(text.split('#').next().unwrap_or(""), span, env, texts, 0)
}
