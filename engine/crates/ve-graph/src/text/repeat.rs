//! Bounded loop expansion with a source line and iteration for each emitted line.

mod header;
mod macros;

use super::expr::{self, Env, Texts};
use super::lex::{Fail, Tok, syntax};
use crate::diag::{Diagnostic, Span};
pub(in crate::text) use header::{Item, bindings};
use header::{condition, else_at, end, header, keyword};
use macros::{Macro, Macros};

const LIMIT: usize = 2_000;

#[derive(Clone, Debug)]
pub(super) struct Origin {
    pub(super) line: u32,
    pub(super) iteration: String,
}

pub(super) struct Expanded {
    pub(super) text: String,
    pub(super) constants: Vec<String>,
    origins: Vec<Origin>,
}

impl Expanded {
    pub(super) fn origin(&self, line: u32) -> Option<&Origin> {
        self.origins.get(line.saturating_sub(1) as usize)
    }

    pub(super) fn locate(&self, mut diagnostic: Diagnostic) -> Diagnostic {
        if let Some(span) = diagnostic.span
            && let Some(origin) = self.origin(span.line)
        {
            diagnostic.span = Some(Span {
                line: origin.line,
                ..span
            });
            annotate(&mut diagnostic, &origin.iteration);
        }
        diagnostic
    }
}

pub(super) fn annotate(diagnostic: &mut Diagnostic, iteration: &str) {
    if !iteration.is_empty() {
        diagnostic.message.push_str(" (in iteration ");
        diagnostic.message.push_str(iteration);
        diagnostic.message.push(')');
    }
}

fn span(line: usize) -> Span {
    Span {
        line: u32::try_from(line + 1).unwrap_or(u32::MAX),
        col: 1,
        len: 1,
    }
}

pub(super) fn too_big(at: Span) -> Fail {
    Box::new(
        Diagnostic::error("parse.too_big", "text expands past 2,000 statements")
            .at_span(at)
            .suggest("use inst.array or inst.radial to repeat without extra nodes"),
    )
}

struct Expander {
    out: Expanded,
    statements: usize,
    nesting: usize,
    /// The macros defined so far, and the names of the ones being read right now (a macro cannot be inside itself).
    defs: Macros,
    calls: Vec<String>,
}

impl Expander {
    fn emit(
        &mut self,
        text: &str,
        line: usize,
        iteration: &[(String, String)],
    ) -> Result<(), Fail> {
        // Count statement starts, including stages; multiline argument lines are not statements.
        let tokens = super::lex::lex(text)?;
        let starts = self.nesting == 0
            && (matches!(tokens.as_slice(), [a, b, ..]
            if matches!(a.tok, Tok::Word(_)) && b.tok == Tok::Equals)
                || keyword(text, "output").is_some());
        let existing = matches!(tokens.as_slice(), [_, b, c, d, ..]
            if b.tok == Tok::Equals && matches!(c.tok, Tok::Word(_)) && d.tok == Tok::Pipe);
        self.statements += usize::from(starts && !existing);
        for token in &tokens {
            match token.tok {
                Tok::Open | Tok::LBracket => self.nesting += 1,
                Tok::Close | Tok::RBracket => self.nesting = self.nesting.saturating_sub(1),
                Tok::Pipe if self.nesting == 0 => self.statements += 1,
                _ => {}
            }
        }
        if self.statements > LIMIT {
            return Err(too_big(span(line)));
        }
        self.out.text.push_str(text);
        self.out.text.push('\n');
        self.out.origins.push(Origin {
            line: span(line).line,
            iteration: iteration
                .iter()
                .map(|(n, v)| format!("{n}={v}"))
                .collect::<Vec<_>>()
                .join(", "),
        });
        Ok(())
    }

    fn block(
        &mut self,
        lines: &[&str],
        start: usize,
        stop: usize,
        env: &mut Env,
        texts: &mut Texts,
        iteration: &[(String, String)],
        depth: usize,
    ) -> Result<(), Fail> {
        let mut index = start;
        while index < stop {
            let result = self.line(lines, &mut index, env, texts, iteration, depth);
            result.map_err(|mut e| {
                if !e.message.contains("in iteration") {
                    let context = iteration
                        .iter()
                        .map(|(n, v)| format!("{n}={v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    annotate(&mut e, &context);
                }
                e
            })?;
        }
        Ok(())
    }

    fn line(
        &mut self,
        lines: &[&str],
        index: &mut usize,
        env: &mut Env,
        texts: &mut Texts,
        iteration: &[(String, String)],
        depth: usize,
    ) -> Result<(), Fail> {
        let line = *index;
        let text = lines[line];
        if depth > 0
            && ["expose", "result"]
                .iter()
                .any(|k| keyword(text, k).is_some())
        {
            return Err(syntax(
                span(line),
                "loop bodies contain node, let and output statements",
            ));
        }
        if let Some(rest) = condition(text) {
            // `if cond ... else ... end`: only the branch that holds is read; a `let` inside it stays inside it.
            let close = end(lines, line, depth)?;
            let middle = else_at(lines, line, close)?;
            let holds = expr::truth(rest, span(line), env, texts)?;
            self.emit("", line, iteration)?;
            let (from, to) = match (holds, middle) {
                (true, _) => (line + 1, middle.unwrap_or(close)),
                (false, Some(m)) => (m + 1, close),
                (false, None) => (close, close),
            };
            let (mut local, mut local_texts) = (env.clone(), texts.clone());
            self.block(
                lines,
                from,
                to,
                &mut local,
                &mut local_texts,
                iteration,
                depth,
            )?;
            if let Some(m) = middle {
                self.emit("", m, iteration)?;
            }
            self.emit("", close, iteration)?;
            *index = close + 1;
        } else if let Some(rest) = header(text) {
            if depth >= 3 {
                return Err(syntax(span(line), "loops may nest to depth 3"));
            }
            let close = end(lines, line, depth + 1)?;
            let (names, rows) = bindings(rest, span(line), env)?;
            self.emit("", line, iteration)?;
            for row in rows {
                let mut local = env.clone();
                let mut local_texts = texts.clone();
                let mut context = iteration.to_vec();
                for (name, val) in names.iter().zip(row) {
                    context.push((name.clone(), val.shown()));
                    // A loop variable shadows a constant of the same name, number or text.
                    local.remove(name);
                    local_texts.remove(name);
                    match val {
                        Item::Num(number) => {
                            local.insert(name.clone(), number);
                        }
                        Item::Text(text) => {
                            local_texts.insert(name.clone(), text);
                        }
                    }
                }
                self.block(
                    lines,
                    line + 1,
                    close,
                    &mut local,
                    &mut local_texts,
                    &context,
                    depth + 1,
                )?;
            }
            self.emit("", close, iteration)?;
            *index = close + 1;
        } else if let Some(rest) = macros::definition(text) {
            // `def name(a, b) ... end`: remembered, read again at each call.
            if depth > 0 {
                return Err(syntax(
                    span(line),
                    "a def goes at the top level, not inside a loop or a macro",
                ));
            }
            let close = end(lines, line, depth)?;
            let (name, params) = macros::parse_def(rest, span(line))?;
            if self.defs.contains_key(&name) {
                return Err(syntax(
                    span(line),
                    format!("the macro {name} is already defined"),
                ));
            }
            macros::check_body(lines, line + 1, close)?;
            self.defs.insert(
                name,
                Macro {
                    params,
                    from: line + 1,
                    to: close,
                },
            );
            self.emit("", line, iteration)?;
            self.emit("", close, iteration)?;
            *index = close + 1;
        } else if let Some((name, arguments)) = macros::call(text, &self.defs) {
            if self.calls.iter().any(|open| open == name) {
                return Err(syntax(
                    span(line),
                    format!("the macro {name} cannot call itself, even through another macro"),
                ));
            }
            if self.calls.len() >= macros::MAX_CALL_DEPTH {
                return Err(syntax(span(line), "macros call each other too deeply"));
            }
            let called = self.defs[name].clone();
            let (mut local, mut local_texts, added) =
                macros::bind(name, &called, arguments, span(line), env, texts)?;
            let mut context = iteration.to_vec();
            context.extend(added);
            self.emit("", line, iteration)?;
            self.calls.push(name.to_owned());
            let done = self.block(
                lines,
                called.from,
                called.to,
                &mut local,
                &mut local_texts,
                &context,
                depth,
            );
            self.calls.pop();
            done?;
            *index += 1;
        } else {
            if let Some(rest) = expr::definition(text) {
                expr::define(rest, span(line).line, env, texts)?;
                self.out
                    .constants
                    .extend(env.keys().chain(texts.keys()).cloned());
                self.emit("", line, iteration)?;
            } else {
                let expanded = expr::substitute(text, span(line).line, env, texts)?;
                // Lexer errors must refer to this source line, rather than the isolated line.
                self.emit(&expanded, line, iteration).map_err(|mut e| {
                    if let Some(s) = e.span {
                        e.span = Some(Span {
                            line: span(line).line,
                            ..s
                        });
                    }
                    e
                })?;
            }
            *index += 1;
        }
        Ok(())
    }
}

pub(super) fn expand(source: &str) -> Result<Expanded, Fail> {
    let mut expander = Expander {
        out: Expanded {
            text: String::new(),
            constants: Vec::new(),
            origins: Vec::new(),
        },
        statements: 0,
        nesting: 0,
        defs: Macros::new(),
        calls: Vec::new(),
    };
    let lines: Vec<_> = source.lines().collect();
    if !lines.iter().any(|line| {
        header(line).is_some() || condition(line).is_some() || macros::definition(line).is_some()
    }) {
        let (text, constants) = expr::expand(source)?;
        for (index, line) in text.lines().enumerate() {
            expander.emit(line, index, &[]).map_err(|mut e| {
                if let Some(s) = e.span {
                    e.span = Some(Span {
                        line: span(index).line,
                        ..s
                    });
                }
                e
            })?;
        }
        expander.out.constants = constants;
        return Ok(expander.out);
    }
    expander.block(
        &lines,
        0,
        lines.len(),
        &mut Env::new(),
        &mut Texts::new(),
        &[],
        0,
    )?;
    expander.out.constants.sort();
    expander.out.constants.dedup();
    Ok(expander.out)
}
