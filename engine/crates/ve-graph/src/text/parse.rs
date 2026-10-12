//! Statements: `name = family.op(inputs, key=value, ...)` and `output name [= node]`.

use super::lex::{Fail, Tok, Token, lex, syntax};
use crate::diag::Span;

/// A literal value as written, before it is coerced to a parameter's type.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Lit {
    Num {
        value: f64,
        whole: bool,
        unit: String,
        span: Span,
    },
    Text(String, Span),
    Bool(bool, Span),
    List(Vec<Lit>, Span),
}

impl Lit {
    pub(super) fn span(&self) -> Span {
        match self {
            Self::Num { span, .. }
            | Self::Text(_, span)
            | Self::Bool(_, span)
            | Self::List(_, span) => *span,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum Arg {
    /// A node read by an input port; `port` is set when written `port=node`.
    Input {
        port: Option<String>,
        name: String,
        span: Span,
    },
    Param {
        key: String,
        key_span: Span,
        value: Lit,
    },
}

#[derive(Clone, Debug)]
pub(super) enum Stmt {
    Node {
        name: String,
        name_span: Span,
        op: String,
        op_span: Span,
        args: Vec<Arg>,
    },
    Output {
        name: String,
        name_span: Span,
        node: Option<(String, Span)>,
    },
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    last: Span,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn here(&self) -> Span {
        self.peek().map_or(self.last, |t| t.span)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        if let Some(t) = &token {
            self.last = t.span;
            self.at += 1;
        }
        token
    }

    fn skip_newlines(&mut self) {
        while matches!(
            self.peek(),
            Some(Token {
                tok: Tok::Newline,
                ..
            })
        ) {
            self.at += 1;
        }
    }

    fn word(&mut self, what: &str) -> Result<(String, Span), Fail> {
        match self.next() {
            Some(Token {
                tok: Tok::Word(w),
                span,
            }) => Ok((w, span)),
            Some(t) => Err(syntax(t.span, format!("expected {what}"))),
            None => Err(syntax(self.last, format!("expected {what}"))),
        }
    }

    fn expect(&mut self, tok: &Tok, what: &str) -> Result<Span, Fail> {
        match self.next() {
            Some(t) if &t.tok == tok => Ok(t.span),
            Some(t) => Err(syntax(t.span, format!("expected {what}"))),
            None => Err(syntax(self.last, format!("expected {what}"))),
        }
    }

    fn literal(&mut self) -> Result<Lit, Fail> {
        let Some(token) = self.next() else {
            return Err(syntax(self.last, "expected a value"));
        };
        match token.tok {
            Tok::Number { value, whole, unit } => Ok(Lit::Num {
                value,
                whole,
                unit,
                span: token.span,
            }),
            Tok::Text(text) => Ok(Lit::Text(text, token.span)),
            Tok::Word(w) if w == "true" || w == "false" => Ok(Lit::Bool(w == "true", token.span)),
            Tok::LBracket => self.list(token.span),
            _ => Err(syntax(
                token.span,
                "expected a number, text, true, false or a [list]",
            )),
        }
    }

    fn list(&mut self, open: Span) -> Result<Lit, Fail> {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(
                self.peek(),
                Some(Token {
                    tok: Tok::RBracket,
                    ..
                })
            ) {
                let close = self.here();
                self.at += 1;
                let span = Span {
                    len: close.col.saturating_sub(open.col) + 1,
                    ..open
                };
                return Ok(Lit::List(
                    items,
                    if close.line == open.line { span } else { open },
                ));
            }
            items.push(self.literal()?);
            self.skip_newlines();
            match self.peek().map(|t| t.tok.clone()) {
                Some(Tok::Comma) => self.at += 1,
                Some(Tok::RBracket) => {}
                _ => return Err(syntax(self.here(), "expected , or ] in the list")),
            }
        }
    }

    fn args(&mut self) -> Result<Vec<Arg>, Fail> {
        let mut args = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(
                self.peek(),
                Some(Token {
                    tok: Tok::Close,
                    ..
                })
            ) {
                self.at += 1;
                return Ok(args);
            }
            let (word, span) = self.word("an input name, a key=value pair or )")?;
            if matches!(
                self.peek(),
                Some(Token {
                    tok: Tok::Equals,
                    ..
                })
            ) {
                self.at += 1;
                match self.peek().map(|t| (t.tok.clone(), t.span)) {
                    Some((Tok::Word(node), node_span)) if node != "true" && node != "false" => {
                        self.at += 1;
                        args.push(Arg::Input {
                            port: Some(word),
                            name: node,
                            span: node_span,
                        });
                    }
                    _ => args.push(Arg::Param {
                        key: word,
                        key_span: span,
                        value: self.literal()?,
                    }),
                }
            } else {
                args.push(Arg::Input {
                    port: None,
                    name: word,
                    span,
                });
            }
            self.skip_newlines();
            match self.peek().map(|t| t.tok.clone()) {
                Some(Tok::Comma) => self.at += 1,
                Some(Tok::Close) => {}
                _ => return Err(syntax(self.here(), "expected , or ) after the argument")),
            }
        }
    }

    fn statement(&mut self) -> Result<Stmt, Fail> {
        let (first, first_span) = self.word("a node name or `output`")?;
        if first == "output" {
            let (name, name_span) = self.word("the output's name")?;
            let node = if matches!(
                self.peek(),
                Some(Token {
                    tok: Tok::Equals,
                    ..
                })
            ) {
                self.at += 1;
                Some(self.word("the node the output reads")?)
            } else {
                None
            };
            return Ok(Stmt::Output {
                name,
                name_span,
                node,
            });
        }
        self.expect(
            &Tok::Equals,
            "= after the node name (form: name = family.op(...))",
        )?;
        let (op, op_span) = self.word("an operator such as solid.revolve")?;
        self.expect(&Tok::Open, "( after the operator")?;
        Ok(Stmt::Node {
            name: first,
            name_span: first_span,
            op,
            op_span,
            args: self.args()?,
        })
    }
}

/// Parses every statement; the first syntax error stops parsing.
///
/// # Errors
/// `parse.syntax` with the span of the offending token.
pub(super) fn parse(source: &str) -> Result<Vec<Stmt>, Fail> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        last: Span {
            line: 1,
            col: 1,
            len: 1,
        },
    };
    let mut out = Vec::new();
    loop {
        parser.skip_newlines();
        if parser.peek().is_none() {
            return Ok(out);
        }
        out.push(parser.statement()?);
        match parser.peek() {
            None
            | Some(Token {
                tok: Tok::Newline, ..
            }) => {}
            Some(t) => return Err(syntax(t.span, "one statement per line")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_statement_with_inputs_and_params_parses() {
        let stmts =
            parse("body = solid.revolve(outer, angle=360deg,\n  segments=64)\noutput vase = body")
                .unwrap();
        assert_eq!(stmts.len(), 2);
        assert!(
            matches!(&stmts[0], Stmt::Node { name, args, .. } if name == "body" && args.len() == 3)
        );
        assert!(matches!(&stmts[1], Stmt::Output { node: Some((n, _)), .. } if n == "body"));
    }

    #[test]
    fn errors_name_what_was_expected_and_where() {
        let error = parse("a = (x)").unwrap_err();
        assert_eq!(error.code, "parse.syntax");
        assert_eq!(error.span.map(|s| (s.line, s.col)), Some((1, 5)));
        assert!(parse("a = f(x=)").is_err());
        assert!(parse("a = f(x) b = g()").is_err());
    }
}
