use super::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    Name(String),
    Number(f32),
    String(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Dot,
    Comma,
    Left,
    Right,
    Semi,
    End,
}

#[derive(Clone, Debug)]
pub(super) struct Token {
    pub kind: Kind,
    pub span: Span,
}

pub(super) fn lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let (mut index, mut line, mut column) = (0, 1, 1);
    while index < bytes.len() {
        let span = Span { line, column };
        if bytes[index].is_ascii_whitespace() {
            if bytes[index] == b'\n' {
                tokens.push(Token {
                    kind: Kind::Semi,
                    span,
                });
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            index += 1;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
                column += 1;
            }
            continue;
        }
        match scan_token(source, index, span) {
            Ok((token, width)) => {
                tokens.push(token);
                index += width;
                column += width;
            }
            Err((diagnostic, width)) => {
                errors.push(diagnostic);
                index += width;
                column += width;
            }
        }
    }
    tokens.push(Token {
        kind: Kind::End,
        span: Span { line, column },
    });
    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

fn scan_token(
    source: &str,
    start: usize,
    span: Span,
) -> Result<(Token, usize), (Diagnostic, usize)> {
    let bytes = source.as_bytes();
    let first = bytes[start];
    if first.is_ascii_alphabetic() || first == b'_' {
        return Ok(scan_name(source, start, span));
    }
    if first.is_ascii_digit()
        || (first == b'.' && bytes.get(start + 1).is_some_and(u8::is_ascii_digit))
    {
        return scan_number(source, start, span);
    }
    if first == b'"' {
        return scan_string(source, start, span);
    }
    scan_punctuation(bytes, start, span)
}

fn scan_name(source: &str, start: usize, span: Span) -> (Token, usize) {
    let bytes = source.as_bytes();
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    (
        Token {
            kind: Kind::Name(source[start..end].to_owned()),
            span,
        },
        end - start,
    )
}

fn scan_number(
    source: &str,
    start: usize,
    span: Span,
) -> Result<(Token, usize), (Diagnostic, usize)> {
    let bytes = source.as_bytes();
    let mut end = start;
    while end < bytes.len() && number_byte(bytes, end) {
        end += 1;
    }
    match source[start..end].parse::<f32>() {
        Ok(value) => Ok((
            Token {
                kind: Kind::Number(value),
                span,
            },
            end - start,
        )),
        Err(_) => Err((
            Diagnostic::new(span, "invalid number literal", "write a decimal number"),
            end - start,
        )),
    }
}

fn number_byte(bytes: &[u8], index: usize) -> bool {
    bytes[index].is_ascii_digit()
        || matches!(bytes[index], b'.' | b'e' | b'E')
        || matches!(bytes[index], b'+' | b'-')
            && matches!(bytes.get(index.wrapping_sub(1)), Some(b'e' | b'E'))
}

fn scan_string(
    source: &str,
    start: usize,
    span: Span,
) -> Result<(Token, usize), (Diagnostic, usize)> {
    let bytes = source.as_bytes();
    let mut end = start + 1;
    while end < bytes.len() && bytes[end] != b'"' {
        end += 1;
    }
    if end == bytes.len() {
        return Err((
            Diagnostic::new(
                span,
                "unterminated string literal",
                "close the string with a quote",
            ),
            end - start,
        ));
    }
    Ok((
        Token {
            kind: Kind::String(source[start + 1..end].to_owned()),
            span,
        },
        end + 1 - start,
    ))
}

fn scan_punctuation(
    bytes: &[u8],
    start: usize,
    span: Span,
) -> Result<(Token, usize), (Diagnostic, usize)> {
    let pair = bytes.get(start..start + 2);
    let found = match pair {
        Some(b"==") => Some((Kind::EqEq, 2)),
        Some(b"!=") => Some((Kind::Ne, 2)),
        Some(b"<=") => Some((Kind::Le, 2)),
        Some(b">=") => Some((Kind::Ge, 2)),
        Some(b"&&") => Some((Kind::And, 2)),
        Some(b"||") => Some((Kind::Or, 2)),
        _ => None,
    };
    if let Some((kind, width)) = found {
        return Ok((Token { kind, span }, width));
    }
    let kind = match bytes[start] {
        b'+' => Kind::Plus,
        b'-' => Kind::Minus,
        b'*' => Kind::Star,
        b'/' => Kind::Slash,
        b'%' => Kind::Percent,
        b'!' => Kind::Bang,
        b'=' => Kind::Eq,
        b'<' => Kind::Lt,
        b'>' => Kind::Gt,
        b'.' => Kind::Dot,
        b',' => Kind::Comma,
        b'(' => Kind::Left,
        b')' => Kind::Right,
        b';' => Kind::Semi,
        _ => {
            return Err((
                Diagnostic::new(span, "unexpected character", "remove this character"),
                1,
            ));
        }
    };
    Ok((Token { kind, span }, 1))
}
