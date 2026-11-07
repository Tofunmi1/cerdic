//! `def name(a, b) ... end` and the calls `name(1, "x")`: a block of lines written once and read again for each call, with its parameters bound to the
//! arguments. The body may hold nodes, `let`, `for`, `if` and calls of other macros (never of itself); it sees its parameters and the constants that
//! stand above the call. Nothing here can run for ever: loops and statements are capped as everywhere else, and recursion is refused.
use std::collections::BTreeMap;

use super::header::{Item, clean, keyword};
use super::span;
use crate::diag::Span;
use crate::text::expr::{self, Env, Texts, evaluate, split_args};
use crate::text::lex::{Fail, syntax};

/// The most parameters a macro may take.
const MAX_PARAMS: usize = 12;
/// The most macros that may be calling each other at once.
pub(super) const MAX_CALL_DEPTH: usize = 12;

/// A macro: its parameter names and where its body stands in the source (`from..to`, the line after `def` up to the line of its `end`).
#[derive(Clone, Debug)]
pub(super) struct Macro {
    pub(super) params: Vec<String>,
    pub(super) from: usize,
    pub(super) to: usize,
}

pub(super) type Macros = BTreeMap<String, Macro>;

/// The rest of a `def` line, when `line` is one.
pub(super) fn definition(line: &str) -> Option<&str> {
    keyword(line, "def")
}

fn name_ok(name: &str) -> bool {
    crate::text::build::valid_name(name) && !expr::RESERVED.contains(&name)
}

/// The name and parameters in `name(a, b)`.
pub(super) fn parse_def(rest: &str, at: Span) -> Result<(String, Vec<String>), Fail> {
    let rest = clean(rest);
    let (Some(open), true) = (rest.find('('), rest.ends_with(')')) else {
        return Err(syntax(
            at,
            "write def name(a, b) on one line, then the body, then end",
        ));
    };
    let name = rest[..open].trim();
    if !name_ok(name) {
        return Err(syntax(at, format!("{name:?} cannot name a macro")));
    }
    let inner = rest[open + 1..rest.len() - 1].trim();
    let params: Vec<String> = if inner.is_empty() {
        Vec::new()
    } else {
        inner.split(',').map(|p| p.trim().to_owned()).collect()
    };
    if params.len() > MAX_PARAMS {
        return Err(syntax(
            at,
            format!("a macro takes at most {MAX_PARAMS} parameters"),
        ));
    }
    for (index, param) in params.iter().enumerate() {
        if !name_ok(param) || params[..index].contains(param) {
            return Err(syntax(at, format!("{param:?} cannot name a parameter")));
        }
    }
    Ok((name.to_owned(), params))
}

/// Refuses what a macro body may not hold: a group, `expose`, `result` or another `def`.
pub(super) fn check_body(lines: &[&str], from: usize, to: usize) -> Result<(), Fail> {
    for (offset, line) in lines[from..to].iter().enumerate() {
        let text = clean(line);
        for banned in ["group", "expose", "result", "def"] {
            if keyword(text, banned).is_some() {
                return Err(syntax(
                    span(from + offset),
                    format!("a macro body holds nodes, let, for, if and calls, not {banned}"),
                ));
            }
        }
    }
    Ok(())
}

/// `(name, arguments text)` when `line` calls a defined macro.
pub(super) fn call<'a>(line: &'a str, defs: &Macros) -> Option<(&'a str, &'a str)> {
    let text = clean(line);
    let open = text.find('(')?;
    let name = text[..open].trim();
    if !defs.contains_key(name) || !text.ends_with(')') {
        return None;
    }
    Some((name, &text[open + 1..text.len() - 1]))
}

fn argument(text: &str, at: Span, env: &Env, texts: &Texts) -> Result<Item, Fail> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        if inner.contains(['"', '\\']) {
            return Err(syntax(
                at,
                "a macro text argument has no quotes or backslashes inside",
            ));
        }
        // `"n${i}"` builds a text from constants and loop variables.
        return Ok(Item::Text(expr::splice(inner, at.line, 0, env, texts)?));
    }
    if let Some(known) = texts.get(text) {
        return Ok(Item::Text(known.clone()));
    }
    let body = text
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(text);
    evaluate(body, at, env).map(Item::Num)
}

/// The constants a call's body starts with (the caller's, then the parameters bound to the arguments) and the note errors carry.
pub(super) fn bind(
    name: &str,
    called: &Macro,
    arguments: &str,
    at: Span,
    env: &Env,
    texts: &Texts,
) -> Result<(Env, Texts, Vec<(String, String)>), Fail> {
    let given = if arguments.trim().is_empty() {
        Vec::new()
    } else {
        split_args(arguments)
    };
    if given.len() != called.params.len() {
        return Err(syntax(
            at,
            format!(
                "{name} takes {} argument(s) ({}), got {}",
                called.params.len(),
                called.params.join(", "),
                given.len()
            ),
        ));
    }
    let (mut local, mut local_texts) = (env.clone(), texts.clone());
    let mut context = vec![("macro".to_owned(), name.to_owned())];
    for (param, text) in called.params.iter().zip(given) {
        let item = argument(text, at, env, texts)?;
        context.push((param.clone(), item.shown()));
        local.remove(param);
        local_texts.remove(param);
        match item {
            Item::Num(number) => {
                local.insert(param.clone(), number);
            }
            Item::Text(value) => {
                local_texts.insert(param.clone(), value);
            }
        }
    }
    Ok((local, local_texts, context))
}
