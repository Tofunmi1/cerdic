//! Constants and arithmetic for the text language, folded away before the parser runs.
//!
//! ```text
//! let w = 1.5
//! let half = w / 2
//! top = solid.box@1(size=[{w}, {half}, {w * 0.4}])
//! ```
//! A line `let name = expression` defines a constant (it becomes an empty line, so line numbers in later errors
//! do not move). `{expression}` anywhere outside a string or comment is replaced by its value, so `{w * 2}mm`
//! reads as `3mm`. Numbers are plain: a length is in metres and an angle in degrees, as the parameter it fills
//! reads them; write a unit suffix after the closing brace. Operators `+ - * /` and parentheses; functions
//! `min max abs sqrt floor ceil round clamp sin cos radians` (trigonometry in radians); constants `pi` and `tau`.
//! Columns after a substitution on the same line shift by the change in length.

mod cond;
mod eval;
mod join;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use super::lex::{Fail, error};
use crate::diag::{Diagnostic, Span};
pub(super) use cond::truth;
pub(super) use eval::evaluate;
pub(super) use join::split_top as split_args;

pub(super) type Env = BTreeMap<String, f64>;
/// Text constants (`let mirror = "u = ...;"`), spliced into strings as `${mirror}`.
pub(super) type Texts = BTreeMap<String, String>;

/// Names a constant may not take: they mean something inside an expression.
pub(super) const RESERVED: [&str; 13] = [
    "pi", "tau", "min", "max", "abs", "sqrt", "floor", "ceil", "round", "clamp", "sin", "cos",
    "radians",
];

/// The source with every `let` line blanked and every `{expression}` replaced by its number, and the names of the
/// constants it defined (so a later "no node is named w" can say that `w` is a constant).
///
/// # Errors
/// `parse.expression` with the span of the expression and a suggestion.
pub(super) fn expand(source: &str) -> Result<(String, Vec<String>), Fail> {
    if !source.contains('{') && !source.lines().any(|line| definition(line).is_some()) {
        return Ok((source.to_owned(), Vec::new()));
    }
    let mut env = Env::new();
    let mut texts = Texts::new();
    let mut out = String::with_capacity(source.len());
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let text = line.trim_end_matches(['\n', '\r']);
        let ending = &line[text.len()..];
        if let Some(rest) = definition(text) {
            define(rest, number, &mut env, &mut texts)?;
        } else {
            out.push_str(&substitute(text, number, &env, &texts)?);
        }
        out.push_str(ending);
    }
    let mut names: Vec<String> = env.into_keys().chain(texts.into_keys()).collect();
    names.sort();
    Ok((out, names))
}

/// What follows `let` when the line defines a constant.
pub(super) fn definition(line: &str) -> Option<&str> {
    let line = line.trim_start();
    line.strip_prefix("let ")
        .or_else(|| (line == "let").then_some(""))
}

fn problem(span: Span, message: impl Into<String>, suggestion: &str) -> Fail {
    Box::new(Diagnostic {
        suggestion: Some(suggestion.to_owned()),
        ..error("parse.expression", span, message)
    })
}

fn span_of(line: u32, col: usize, len: usize) -> Span {
    Span {
        line,
        col: u32::try_from(col + 1).unwrap_or(u32::MAX),
        len: u32::try_from(len.max(1)).unwrap_or(u32::MAX),
    }
}

/// `name = expression` after `let`.
pub(super) fn define(rest: &str, line: u32, env: &mut Env, texts: &mut Texts) -> Result<(), Fail> {
    // A text constant may hold `#` (a colour in a texture program), so only a number's line loses its comment.
    let is_text = rest.split_once('=').is_some_and(|(_, e)| {
        let e = e.trim_start();
        e.starts_with('"') || e.starts_with("join(")
    });
    let rest = if is_text {
        rest
    } else {
        rest.split('#').next().unwrap_or("")
    };
    let span = span_of(line, 0, rest.len() + 4);
    let Some((name, expression)) = rest.split_once('=') else {
        return Err(problem(
            span,
            "a constant is written `let name = expression`",
            "for example: let width = 1.5",
        ));
    };
    let name = name.trim();
    let valid = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid || RESERVED.contains(&name) {
        return Err(problem(
            span,
            format!("{name:?} cannot name a constant"),
            "use lowercase letters, digits and underscores, starting with a letter, and not pi, tau or a function name",
        ));
    }
    if env.contains_key(name) || texts.contains_key(name) {
        return Err(problem(
            span,
            format!("the constant {name} is already defined"),
            "give the second one another name",
        ));
    }
    // `let keys = join(i in 0..5, "{i}:0,0,0", "; ")`: a text made by a loop (see `join.rs`).
    if let Some(args) = expression
        .trim()
        .strip_prefix("join(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        let joined = join::run(args, line, env, texts)?;
        texts.insert(name.to_owned(), joined);
        return Ok(());
    }
    if is_text {
        // `let prog = "n = expr(code=\"...\")"`: the characters between the quotes, kept as written (escapes included) so
        // that splicing them into another string gives the same string; earlier constants inside it are spliced first.
        let body = expression.trim();
        let inner = body
            .strip_prefix('"')
            .and_then(|b| b.strip_suffix('"'))
            .filter(|b| !b.ends_with('\\') || b.ends_with("\\\\"));
        let Some(inner) = inner else {
            return Err(problem(
                span,
                "a text constant is written `let name = \"text\"`",
                "close the quote, and escape quotes inside it as \\\"",
            ));
        };
        let spliced = splice(inner, line, 0, env, texts)?;
        texts.insert(name.to_owned(), spliced);
        return Ok(());
    }
    // `let r = {w * 2}` is accepted too: braces mark an expression everywhere else, so a model may write them here.
    let expression = expression.trim();
    let expression = expression
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(expression);
    let value = evaluate(expression, span, env)?;
    env.insert(name.to_owned(), value);
    Ok(())
}

/// Replaces each `${name}` in the body of a string by the text or number constant of that name.
pub(in crate::text) fn splice(
    body: &str,
    line: u32,
    col: usize,
    env: &Env,
    texts: &Texts,
) -> Result<String, Fail> {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find("${") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let Some(close) = after.find('}') else {
            return Err(problem(
                span_of(line, col, body.len()),
                "this ${ has no closing }",
                "name a constant: ${mirror}",
            ));
        };
        let name = after[..close].trim();
        if let Some(text) = texts.get(name) {
            out.push_str(text);
        } else if let Some(value) = env.get(name) {
            out.push_str(&format_number(*value));
        } else {
            return Err(problem(
                span_of(line, col, body.len()),
                format!("no constant is named {name}"),
                "define it above with `let name = \"text\"` or `let name = 12`",
            ));
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Replaces each `{expression}` on one line, leaving strings and comments alone; inside strings `${name}` is spliced.
pub(super) fn substitute(text: &str, line: u32, env: &Env, texts: &Texts) -> Result<String, Fail> {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let (mut at, mut in_string) = (0, false);
    while at < chars.len() {
        let c = chars[at];
        // An escaped character (`\"`, `\\`) inside a string neither ends it nor starts an expression.
        if in_string && c == '\\' {
            out.push(c);
            out.extend(chars.get(at + 1));
            at += 2;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
        }
        if !in_string && c == '#' {
            out.extend(&chars[at..]);
            break;
        }
        // `${name}` is spliced as written, inside a string (`keys="${keys}"`) or outside one (`points=[${pts}]`).
        if c == '$' && chars.get(at + 1) == Some(&'{') {
            let Some(close) = chars[at..].iter().position(|&ch| ch == '}') else {
                return Err(problem(
                    span_of(line, at, chars.len() - at),
                    "this ${ has no closing }",
                    "name a constant: ${mirror}",
                ));
            };
            let piece: String = chars[at..=at + close].iter().collect();
            out.push_str(&splice(&piece, line, at, env, texts)?);
            at += close + 1;
            continue;
        }
        if in_string || c != '{' {
            out.push(c);
            at += 1;
            continue;
        }
        let Some(close) = chars[at..].iter().position(|&ch| ch == '}') else {
            return Err(problem(
                span_of(line, at, chars.len() - at),
                "this { has no closing }",
                "close the expression: {width * 2}",
            ));
        };
        let body: String = chars[at + 1..at + close].iter().collect();
        let span = span_of(line, at, close + 1);
        // Expressions embedded in identifiers must leave a usable identifier.
        let in_name = out
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            || (out.trim().is_empty() && chars[at + close + 1..].contains(&'='))
            // `output {name} = ...` names a node too.
            || out.trim() == "output";
        // `{name}` of a text loop variable: raw inside a node name, a quoted string anywhere else (`bone={name}`).
        if let Some(text) = texts.get(body.trim()) {
            if in_name {
                let usable =
                    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if !usable {
                    return Err(problem(
                        span,
                        format!("the text {text:?} cannot go inside a node name"),
                        "use letters, digits and underscores only",
                    ));
                }
                out.push_str(text);
            } else {
                out.push('"');
                out.push_str(text);
                out.push('"');
            }
            at += close + 1;
            continue;
        }
        let value = evaluate(&body, span, env)?;
        if in_name && value.fract() != 0.0 {
            return Err(problem(
                span,
                "a number inside a name must be whole",
                "use round, floor or ceil",
            ));
        }
        let number = format_number(value);
        if in_name && value < 0.0 {
            out.push('m');
            out.push_str(number.trim_start_matches('-'));
        } else {
            out.push_str(&number);
        }
        at += close + 1;
    }
    Ok(out)
}

/// A number the lexer reads back exactly: whole values without a point, no exponent.
pub(super) fn format_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

/// Suggest braces when a constant was used in place of a node reference.
pub(super) fn hint_constants(errors: &mut [Diagnostic], constants: &[String]) {
    // A constant used where a node is expected (`radius=w` instead of `radius={w}`) gets the fix spelled out.
    for error in errors {
        let name = error.message.rsplit(' ').next().unwrap_or("").to_owned();
        if error.code == "parse.unknown_name"
            && error.suggestion.is_none()
            && constants.contains(&name)
        {
            error.suggestion = Some(format!(
                "{name} is a constant, not a node: write {{{name}}} where its value goes"
            ));
        }
    }
}
