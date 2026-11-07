//! SimL tokens: identifiers, integers, floats, punctuation and statement-ending newlines (newlines inside brackets are skipped).
use super::Diagnostic;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Sym(&'static str),
    Newline,
}

#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) tok: Tok,
    pub(super) line: usize,
    pub(super) col: usize,
}

const SYMS: [&str; 26] = [
    "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "..", "(", ")", "[", "]", ",", ":", "=", "+", "-", "*", "/", "%", "<",
    ">", ".",
];
const SYMS_EXTRA: [&str; 4] = ["!", ";", "{", "}"];

fn diag(line: usize, col: usize, message: &str, suggestion: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: message.to_owned(),
        suggestion: suggestion.to_owned(),
    }
}

pub(super) fn lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let (mut at, mut line, mut col, mut depth) = (0_usize, 1_usize, 1_usize, 0_i32);
    while at < chars.len() {
        let c = chars[at];
        let (start_line, start_col) = (line, col);
        let take = |n: usize, at: &mut usize, col: &mut usize| {
            *at += n;
            *col += n;
        };
        if c == '\n' {
            if depth <= 0 {
                out.push(Token { tok: Tok::Newline, line: start_line, col: start_col });
            }
            at += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c == ' ' || c == '\t' || c == '\r' {
            take(1, &mut at, &mut col);
            continue;
        }
        if c == '#' {
            while at < chars.len() && chars[at] != '\n' {
                take(1, &mut at, &mut col);
            }
            continue;
        }
        if c.is_ascii_digit() {
            let begin = at;
            let mut is_float = false;
            while at < chars.len() && chars[at].is_ascii_digit() {
                take(1, &mut at, &mut col);
            }
            // `0..4` is a range, not the float `0.`
            if at + 1 < chars.len() && chars[at] == '.' && chars[at + 1].is_ascii_digit() {
                is_float = true;
                take(1, &mut at, &mut col);
                while at < chars.len() && chars[at].is_ascii_digit() {
                    take(1, &mut at, &mut col);
                }
            }
            if at < chars.len() && (chars[at] == 'e' || chars[at] == 'E') {
                let mut look = at + 1;
                if look < chars.len() && (chars[look] == '-' || chars[look] == '+') {
                    look += 1;
                }
                if look < chars.len() && chars[look].is_ascii_digit() {
                    is_float = true;
                    let n = look - at;
                    take(n, &mut at, &mut col);
                    while at < chars.len() && chars[at].is_ascii_digit() {
                        take(1, &mut at, &mut col);
                    }
                }
            }
            let text: String = chars[begin..at].iter().collect();
            let tok = if is_float {
                text.parse::<f64>().map(Tok::Float).map_err(|_| vec![diag(start_line, start_col, "bad number", "write the number again")])?
            } else {
                text.parse::<i64>().map(Tok::Int).map_err(|_| vec![diag(start_line, start_col, "integer too large", "use a smaller integer")])?
            };
            out.push(Token { tok, line: start_line, col: start_col });
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let begin = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
                take(1, &mut at, &mut col);
            }
            out.push(Token { tok: Tok::Ident(chars[begin..at].iter().collect()), line: start_line, col: start_col });
            continue;
        }
        let rest: String = chars[at..(at + 2).min(chars.len())].iter().collect();
        let sym = SYMS
            .iter()
            .chain(SYMS_EXTRA.iter())
            .find(|s| rest.starts_with(**s))
            .copied();
        let Some(sym) = sym else {
            return Err(vec![diag(start_line, start_col, &format!("unexpected character {c:?}"), "remove it")]);
        };
        match sym {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            _ => {}
        }
        out.push(Token { tok: Tok::Sym(sym), line: start_line, col: start_col });
        take(sym.len(), &mut at, &mut col);
    }
    out.push(Token { tok: Tok::Newline, line, col });
    Ok(out)
}
