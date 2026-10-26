//! Tokens with source spans. Newlines separate statements except inside brackets.

use crate::diag::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    /// A name or operator reference such as `outer` or `solid.revolve@1`.
    Word(String),
    /// A number with an optional unit suffix (`mm`, `deg`, ...).
    Number {
        value: f64,
        whole: bool,
        unit: String,
    },
    Text(String),
    Equals,
    Open,
    Close,
    LBracket,
    RBracket,
    Comma,
    Pipe,
    Newline,
}

#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) tok: Tok,
    pub(super) span: Span,
}

pub(super) type Fail = Box<Diagnostic>;

pub(super) fn syntax(span: Span, message: impl Into<String>) -> Fail {
    fail("parse.syntax", span, message)
}

/// An error with a code, placed in the source.
pub(super) fn error(code: &str, span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message).at_span(span)
}

/// [`error`], boxed for `Result`s.
pub(super) fn fail(code: &str, span: Span, message: impl Into<String>) -> Fail {
    Box::new(error(code, span, message))
}

struct Cursor<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: u32,
    col: u32,
}

impl Cursor<'_> {
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn eat_while(&mut self, keep: impl Fn(char) -> bool) -> String {
        let mut out = String::new();
        while let Some(&c) = self.chars.peek() {
            if !keep(c) {
                break;
            }
            out.push(c);
            self.bump();
        }
        out
    }
}

fn length(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

fn number(cur: &mut Cursor<'_>, first: char) -> String {
    let mut text = String::from(first);
    text.push_str(&cur.eat_while(|c| c.is_ascii_digit() || c == '.'));
    if matches!(cur.chars.peek(), Some('e' | 'E')) {
        let mut ahead = cur.chars.clone();
        ahead.next();
        let sign = matches!(ahead.peek(), Some('+' | '-'));
        if sign {
            ahead.next();
        }
        if ahead.peek().is_some_and(char::is_ascii_digit) {
            text.extend(cur.bump());
            if sign {
                text.extend(cur.bump());
            }
            text.push_str(&cur.eat_while(|c| c.is_ascii_digit()));
        }
    }
    text
}

fn string(cur: &mut Cursor<'_>, start: Span) -> Result<String, Fail> {
    let mut out = String::new();
    loop {
        match cur.bump() {
            Some('"') => return Ok(out),
            Some('\\') => match cur.bump() {
                Some('n') => out.push('\n'),
                Some(c @ ('"' | '\\')) => out.push(c),
                _ => {
                    return Err(syntax(
                        start,
                        "unknown escape in text (use \\\" \\\\ or \\n)",
                    ));
                }
            },
            Some('\n') | None => return Err(syntax(start, "text is missing its closing quote")),
            Some(c) => out.push(c),
        }
    }
}

/// Splits `source` into tokens. Comments (`#` to the end of the line) are dropped.
///
/// # Errors
/// `parse.syntax` for a character the language does not use or a malformed number or text.
pub(super) fn lex(source: &str) -> Result<Vec<Token>, Fail> {
    let mut cur = Cursor {
        chars: source.chars().peekable(),
        line: 1,
        col: 1,
    };
    let mut out = Vec::new();
    while let Some(&c) = cur.chars.peek() {
        let start = Span {
            line: cur.line,
            col: cur.col,
            len: 1,
        };
        let (tok, len) = match c {
            ' ' | '\t' | '\r' => {
                cur.bump();
                continue;
            }
            '#' => {
                cur.eat_while(|c| c != '\n');
                continue;
            }
            '\n' => {
                cur.bump();
                (Tok::Newline, 1)
            }
            '=' | '(' | ')' | '[' | ']' | ',' | '|' => {
                cur.bump();
                let tok = match c {
                    '=' => Tok::Equals,
                    '|' => Tok::Pipe,
                    '(' => Tok::Open,
                    ')' => Tok::Close,
                    '[' => Tok::LBracket,
                    ']' => Tok::RBracket,
                    _ => Tok::Comma,
                };
                (tok, 1)
            }
            '"' => {
                cur.bump();
                let text = string(&mut cur, start)?;
                let len = length(&text) + 2;
                (Tok::Text(text), len)
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut word = cur.eat_while(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
                if cur.chars.peek() == Some(&'@') {
                    word.extend(cur.bump());
                    word.push_str(&cur.eat_while(|c| c.is_ascii_digit()));
                }
                let len = length(&word);
                (Tok::Word(word), len)
            }
            c if c.is_ascii_digit() || c == '-' || c == '+' => {
                cur.bump();
                let text = number(&mut cur, c);
                let unit = cur.eat_while(|c| c.is_ascii_alphabetic());
                let value = text
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| syntax(start, format!("{text:?} is not a number")))?;
                let whole = !text.contains(['.', 'e', 'E']);
                let len = length(&text) + length(&unit);
                (Tok::Number { value, whole, unit }, len)
            }
            other => {
                return Err(syntax(start, format!("unexpected character {other:?}")));
            }
        };
        out.push(Token {
            tok,
            span: Span { len, ..start },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_keep_units_exponents_and_signs() {
        let tokens = lex("a = f(x=-1.5e-3, y=90deg, z=7)").unwrap();
        let numbers: Vec<_> = tokens
            .iter()
            .filter_map(|t| match &t.tok {
                Tok::Number { value, whole, unit } => Some((*value, *whole, unit.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            numbers,
            vec![
                (-1.5e-3, false, String::new()),
                (90.0, true, "deg".into()),
                (7.0, true, String::new())
            ]
        );
    }

    #[test]
    fn spans_point_at_the_token() {
        let tokens = lex("ab\n  cd").unwrap();
        assert_eq!(
            tokens[2].span,
            Span {
                line: 2,
                col: 3,
                len: 2
            }
        );
    }

    #[test]
    fn stray_characters_and_open_text_are_syntax_errors() {
        assert_eq!(lex("a = $b").unwrap_err().code, "parse.syntax");
        assert_eq!(lex("a = \"open").unwrap_err().code, "parse.syntax");
    }
}
