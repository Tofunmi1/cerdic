//! Source locations retained by a parsed command batch.

use crate::command::Command;
use crate::diag::{Diagnostic, Span};

/// Commands read from text, with the source span each one came from.
#[derive(Clone, Debug, Default)]
pub struct TextBatch {
    /// The commands, in an order that is valid to apply.
    pub commands: Vec<Command>,
    /// For each command, where in the text it was written.
    pub spans: Vec<Span>,
    pub(super) iterations: Vec<String>,
}

impl TextBatch {
    pub(super) fn extend(&mut self, items: Vec<(Command, Span)>) {
        for (command, span) in items {
            self.commands.push(command);
            self.spans.push(span);
        }
    }

    pub(super) fn remap(&mut self, expanded: &super::repeat::Expanded) {
        for span in &mut self.spans {
            let origin = expanded.origin(span.line);
            self.iterations
                .push(origin.map_or(String::new(), |o| o.iteration.clone()));
            if let Some(origin) = origin {
                span.line = origin.line;
            }
        }
    }

    /// Adds the source span to a diagnostic that names a command of this batch.
    #[must_use]
    pub fn locate(&self, diagnostic: Diagnostic) -> Diagnostic {
        match diagnostic.command.and_then(|i| self.spans.get(i)) {
            Some(span) => {
                let iteration = diagnostic.command.and_then(|i| self.iterations.get(i));
                let mut diagnostic = diagnostic.at_span(*span);
                if let Some(iteration) = iteration {
                    super::repeat::annotate(&mut diagnostic, iteration);
                }
                diagnostic
            }
            None => diagnostic,
        }
    }
}
