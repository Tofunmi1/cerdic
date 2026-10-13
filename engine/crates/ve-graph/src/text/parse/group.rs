//! `group name`, its node statements, `expose name = node.param`, `result node`, and `end`.

use super::{Expose, Parser, Stmt};
use crate::text::lex::{Fail, Tok, Token, syntax};

impl Parser {
    /// Reads the rest of a group block; the word `group` has been read.
    pub(super) fn group(&mut self) -> Result<Stmt, Fail> {
        let (name, name_span) = self.word("the group's name")?;
        let mut body = Vec::new();
        let mut exposes = Vec::new();
        let mut result = None;
        loop {
            self.skip_newlines();
            let Some(Token {
                tok: Tok::Word(keyword),
                span,
            }) = self.peek().cloned()
            else {
                return Err(syntax(
                    self.here(),
                    "expected a statement or `end` in the group",
                ));
            };
            match keyword.as_str() {
                "end" => {
                    self.at += 1;
                    return Ok(Stmt::Group {
                        name,
                        name_span,
                        body,
                        exposes,
                        result,
                    });
                }
                "group" | "output" => {
                    return Err(syntax(
                        span,
                        format!("`{keyword}` is not allowed inside a group; end the group first"),
                    ));
                }
                "expose" => {
                    self.at += 1;
                    exposes.push(self.expose(span)?);
                }
                "result" => {
                    self.at += 1;
                    if result.is_some() {
                        return Err(syntax(span, "a group has one result"));
                    }
                    result = Some(self.word("the member that is the group's result")?);
                }
                _ => body.push(self.statement()?),
            }
            match self.peek() {
                None
                | Some(Token {
                    tok: Tok::Newline, ..
                }) => {}
                Some(t) => return Err(syntax(t.span, "one statement per line")),
            }
        }
    }

    fn expose(&mut self, span: crate::diag::Span) -> Result<Expose, Fail> {
        let (name, name_span) = self.word("the name instances use")?;
        self.expect(
            &Tok::Equals,
            "= after the exposed name (form: expose name = node.param)",
        )?;
        let (target, target_span) = self.word("the member and parameter, as node.param")?;
        let Some((node, param)) = target
            .rsplit_once('.')
            .filter(|(n, p)| !n.is_empty() && !p.is_empty())
        else {
            return Err(syntax(
                target_span,
                "write the member and its parameter as node.param",
            ));
        };
        Ok(Expose {
            name,
            name_span,
            node: node.to_owned(),
            param: param.to_owned(),
            span,
        })
    }
}
