//! Pipe stages lower to ordinary node statements, in dependency order.

use super::{Arg, Parser, Stmt};
use crate::diag::Span;
use crate::text::lex::{Fail, Tok, syntax};

impl Parser {
    pub(super) fn pipe(&mut self, name: String, span: Span) -> Result<Vec<Stmt>, Fail> {
        let (first, first_span) = self.word("an operator or a node name")?;
        let mut previous = None;
        let mut stages = Vec::new();
        if self.peek().is_some_and(|t| t.tok == Tok::Open) {
            self.at += 1;
            stages.push((first, first_span, self.args()?));
        } else {
            // `name = other` alone on its line is another name for the node `other`
            if self.peek().is_none_or(|t| t.tok == Tok::Newline) {
                return Ok(vec![Stmt::Alias { name, name_span: span, target: first, target_span: first_span }]);
            }
            previous = Some((first, first_span));
            if !self.peek().is_some_and(|t| t.tok == Tok::Pipe) {
                return Err(syntax(
                    self.here(),
                    "expected ( after the operator or | after a node",
                ));
            }
        }
        while self.peek().is_some_and(|t| t.tok == Tok::Pipe) {
            self.at += 1;
            self.skip_newlines();
            let (op, op_span) = self.word("an operator after |")?;
            self.expect(&Tok::Open, "( after the pipe operator")?;
            stages.push((op, op_span, self.args()?));
        }
        let count = stages.len();
        let piped = count > 1 || previous.is_some();
        let mut out = Vec::new();
        for (index, (op, op_span, mut args)) in stages.into_iter().enumerate() {
            let stage = if index + 1 == count {
                name.clone()
            } else {
                format!("{name}_{}", index + 1)
            };
            if let Some((source, source_span)) = previous {
                args.insert(
                    0,
                    Arg::Input {
                        port: None,
                        name: source,
                        span: source_span,
                    },
                );
            }
            previous = Some((stage.clone(), span));
            out.push(Stmt::Node {
                name: stage,
                name_span: span,
                op,
                op_span,
                args,
                piped,
            });
        }
        Ok(out)
    }
}
