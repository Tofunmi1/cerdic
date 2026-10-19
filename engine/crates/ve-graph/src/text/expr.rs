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

mod eval;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use super::lex::{Fail, error};
use crate::diag::{Diagnostic, Span};
use eval::evaluate;

type Env = BTreeMap<String, f64>;

/// Names a constant may not take: they mean something inside an expression.
const RESERVED: [&str; 13] = [
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
    let mut out = String::with_capacity(source.len());
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let text = line.trim_end_matches(['\n', '\r']);
        let ending = &line[text.len()..];
        if let Some(rest) = definition(text) {
            define(rest, number, &mut env)?;
        } else {
            out.push_str(&substitute(text, number, &env)?);
        }
        out.push_str(ending);
    }
    Ok((out, env.into_keys().collect()))
}

/// What follows `let` when the line defines a constant.
fn definition(line: &str) -> Option<&str> {
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
fn define(rest: &str, line: u32, env: &mut Env) -> Result<(), Fail> {
    let rest = rest.split('#').next().unwrap_or("");
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
    if env.contains_key(name) {
        return Err(problem(
            span,
            format!("the constant {name} is already defined"),
            "give the second one another name",
        ));
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

/// Replaces each `{expression}` on one line, leaving strings and comments alone.
fn substitute(text: &str, line: u32, env: &Env) -> Result<String, Fail> {
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
        out.push_str(&format_number(evaluate(&body, span, env)?));
        at += close + 1;
    }
    Ok(out)
}

/// A number the lexer reads back exactly: whole values without a point, no exponent.
fn format_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}
