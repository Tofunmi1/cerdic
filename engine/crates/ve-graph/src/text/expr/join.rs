//! `let name = join(i in 0..8, "template", "separator")`: a text constant made by running a loop header and writing the template once for each
//! row, joined by the separator (`", "` when left out). In the template `{expression}` is a number and `${name}` is a text constant spliced
//! as written; a loop variable that is text is spliced the same way. This is how `points=[...]` lists and `keys="t:x,y,z; ..."` strings are built.
use super::{Env, Texts, evaluate, format_number, problem, span_of, splice};
use crate::text::lex::Fail;
use crate::text::repeat::{Item, bindings};

/// The most characters a joined text may have.
const MAX_LEN: usize = 65_536;

/// Splits `text` at the commas that are outside quotes, brackets, parentheses and braces.
pub(in crate::text) fn split_top(text: &str) -> Vec<&str> {
    let (mut depth, mut start, mut quoted, mut escaped) = (0_i32, 0, false, false);
    let mut parts = Vec::new();
    for (index, c) in text.char_indices() {
        if quoted {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
}

fn unquote(text: &str) -> Option<&str> {
    text.strip_prefix('"').and_then(|t| t.strip_suffix('"'))
}

/// Runs the `join(...)` whose arguments are `args` (the text between its parentheses).
pub(super) fn run(args: &str, line: u32, env: &Env, texts: &Texts) -> Result<String, Fail> {
    let span = span_of(line, 0, args.len() + 6);
    let parts = split_top(args);
    let (header, template, separator) = match parts.as_slice() {
        [header, template] => (*header, *template, ", "),
        [header, template, separator] => {
            let Some(separator) = unquote(separator) else {
                return Err(problem(
                    span,
                    "the separator of a join is a quoted text",
                    "for example: join(i in 0..4, \"{i}\", \"; \")",
                ));
            };
            (*header, *template, separator)
        }
        _ => {
            return Err(problem(
                span,
                "a join is written join(i in 0..8, \"template\", \"separator\")",
                "the separator may be left out",
            ));
        }
    };
    let Some(template) = unquote(template) else {
        return Err(problem(
            span,
            "the template of a join is a quoted text",
            "for example: join(i in 0..4, \"[{i}, 0, 0]\")",
        ));
    };
    let (names, rows) = bindings(header, span, env)?;
    let mut out = String::new();
    for (count, row) in rows.into_iter().enumerate() {
        let shown = names
            .iter()
            .zip(&row)
            .map(|(name, item)| format!("{name}={}", item.shown()))
            .collect::<Vec<_>>()
            .join(", ");
        let (mut local, mut local_texts) = (env.clone(), texts.clone());
        for (name, item) in names.iter().zip(row) {
            local.remove(name);
            local_texts.remove(name);
            match item {
                Item::Num(number) => {
                    local.insert(name.clone(), number);
                }
                Item::Text(text) => {
                    local_texts.insert(name.clone(), text);
                }
            }
        }
        if count > 0 {
            out.push_str(separator);
        }
        let row_text = render(template, line, &local, &local_texts).map_err(|mut e| {
            e.message.push_str(&format!(" (in iteration {shown})"));
            e
        })?;
        out.push_str(&row_text);
        if out.len() > MAX_LEN {
            return Err(problem(
                span,
                format!("a joined text is longer than {MAX_LEN} characters"),
                "join fewer rows, or split the text in two",
            ));
        }
    }
    Ok(out)
}

/// One row of the template: `{expression}` becomes its number, `${name}` the constant's text.
fn render(template: &str, line: u32, env: &Env, texts: &Texts) -> Result<String, Fail> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        let dollar = rest[..at].ends_with('$');
        out.push_str(&rest[..at - usize::from(dollar)]);
        let after = &rest[at + 1..];
        let Some(close) = after.find('}') else {
            return Err(problem(
                span_of(line, at, template.len()),
                "this { has no closing }",
                "close the expression: {i * 2}",
            ));
        };
        let body = after[..close].trim();
        if dollar {
            out.push_str(&splice(&format!("${{{body}}}"), line, at, env, texts)?);
        } else if let Some(text) = texts.get(body) {
            out.push_str(text);
        } else {
            let value = evaluate(body, span_of(line, at, close + 2), env)?;
            out.push_str(&format_number(value));
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
