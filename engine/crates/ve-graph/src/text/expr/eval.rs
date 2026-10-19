//! Reading and evaluating one expression: numbers, constants, `+ - * /`, brackets and a few functions.

use super::{Env, problem};
use crate::diag::{Diagnostic, Span};
use crate::text::lex::{Fail, error};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(char),
    Open,
    Close,
    Comma,
}

const ALLOWED: &str = "use numbers, constants, + - * /, parentheses and the functions min, max, abs, sqrt, floor, ceil, round, clamp, sin, cos, radians";

fn tokens(text: &str, span: Span) -> Result<Vec<Token>, Fail> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_digit() || chars[at] == '.') {
                at += 1;
            }
            let exponent = at < chars.len()
                && matches!(chars[at], 'e' | 'E')
                && chars
                    .get(at + 1)
                    .is_some_and(|n| n.is_ascii_digit() || *n == '-' || *n == '+');
            if exponent {
                at += 2;
                while at < chars.len() && chars[at].is_ascii_digit() {
                    at += 1;
                }
            }
            let word: String = chars[start..at].iter().collect();
            if at < chars.len() && chars[at].is_ascii_alphabetic() {
                return Err(problem(
                    span,
                    format!("{word}{} looks like a number with a unit", chars[at]),
                    "write units after the closing brace: {width * 2}mm",
                ));
            }
            let value = word.parse::<f64>().map_err(|_| {
                problem(
                    span,
                    format!("{word:?} is not a number"),
                    "write numbers like 1.5 or 2e-3",
                )
            })?;
            out.push(Token::Number(value));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            out.push(Token::Name(chars[start..at].iter().collect()));
        } else {
            at += 1;
            out.push(match c {
                '+' | '-' | '*' | '/' => Token::Op(c),
                '(' => Token::Open,
                ')' => Token::Close,
                ',' => Token::Comma,
                other => {
                    return Err(problem(
                        span,
                        format!("unexpected character {other:?} in an expression"),
                        ALLOWED,
                    ));
                }
            });
        }
    }
    Ok(out)
}

struct Reader<'a> {
    tokens: Vec<Token>,
    at: usize,
    span: Span,
    env: &'a Env,
}

/// The value of `text`, which must be a complete expression with a finite result.
pub(super) fn evaluate(text: &str, span: Span, env: &Env) -> Result<f64, Fail> {
    let mut reader = Reader {
        tokens: tokens(text, span)?,
        at: 0,
        span,
        env,
    };
    if reader.tokens.is_empty() {
        return Err(problem(
            span,
            "the expression is empty",
            "write a value, for example {width * 2}",
        ));
    }
    let value = reader.sum()?;
    if reader.at != reader.tokens.len() {
        return Err(problem(
            span,
            "the expression has something left over after a complete value",
            "check for a missing operator or an extra bracket",
        ));
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err(problem(
            span,
            "the expression does not give a finite number",
            "check for a division by zero or a square root of a negative",
        ))
    }
}

impl Reader<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn sum(&mut self) -> Result<f64, Fail> {
        let mut value = self.product()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.at += 1;
            let right = self.product()?;
            value = if op == '+' {
                value + right
            } else {
                value - right
            };
        }
        Ok(value)
    }

    fn product(&mut self) -> Result<f64, Fail> {
        let mut value = self.unary()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.peek().cloned() {
            self.at += 1;
            let right = self.unary()?;
            if op == '/' && right == 0.0 {
                return Err(problem(
                    self.span,
                    "division by zero",
                    "change the divisor so it is not zero",
                ));
            }
            value = if op == '*' {
                value * right
            } else {
                value / right
            };
        }
        Ok(value)
    }

    fn unary(&mut self) -> Result<f64, Fail> {
        match self.peek() {
            Some(Token::Op('-')) => {
                self.at += 1;
                Ok(-self.unary()?)
            }
            Some(Token::Op('+')) => {
                self.at += 1;
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> Result<f64, Fail> {
        let token = self.peek().cloned();
        self.at += 1;
        match token {
            Some(Token::Number(value)) => Ok(value),
            Some(Token::Open) => {
                let value = self.sum()?;
                self.expect_close()?;
                Ok(value)
            }
            Some(Token::Name(name)) => {
                if self.peek() == Some(&Token::Open) {
                    self.at += 1;
                    self.call(&name)
                } else {
                    self.constant(&name)
                }
            }
            _ => Err(problem(
                self.span,
                "a value was expected here",
                "check for a missing number or constant",
            )),
        }
    }

    fn expect_close(&mut self) -> Result<(), Fail> {
        if self.peek() == Some(&Token::Close) {
            self.at += 1;
            Ok(())
        } else {
            Err(problem(
                self.span,
                "a closing bracket is missing",
                "close every ( with a )",
            ))
        }
    }

    fn constant(&self, name: &str) -> Result<f64, Fail> {
        match name {
            "pi" => Ok(std::f64::consts::PI),
            "tau" => Ok(std::f64::consts::TAU),
            _ => self.env.get(name).copied().ok_or_else(|| {
                let known: Vec<&str> = self.env.keys().map(String::as_str).collect();
                let hint = if known.is_empty() {
                    "define it first with a line `let name = value`".to_owned()
                } else {
                    format!("defined constants: {}", known.join(", "))
                };
                Box::new(Diagnostic {
                    suggestion: Some(hint),
                    ..error(
                        "parse.expression",
                        self.span,
                        format!("{name} is not defined"),
                    )
                })
            }),
        }
    }

    fn call(&mut self, name: &str) -> Result<f64, Fail> {
        let mut args = vec![self.sum()?];
        while self.peek() == Some(&Token::Comma) {
            self.at += 1;
            args.push(self.sum()?);
        }
        self.expect_close()?;
        let wrong = |count: &str| {
            problem(
                self.span,
                format!("{name} takes {count}"),
                "check the number of arguments",
            )
        };
        match (name, args.as_slice()) {
            ("min", [first, rest @ ..]) if !rest.is_empty() => {
                Ok(rest.iter().fold(*first, |a, b| a.min(*b)))
            }
            ("max", [first, rest @ ..]) if !rest.is_empty() => {
                Ok(rest.iter().fold(*first, |a, b| a.max(*b)))
            }
            ("min" | "max", _) => Err(wrong("two or more values")),
            ("abs", [x]) => Ok(x.abs()),
            ("sqrt", [x]) => Ok(x.sqrt()),
            ("floor", [x]) => Ok(x.floor()),
            ("ceil", [x]) => Ok(x.ceil()),
            ("round", [x]) => Ok(x.round()),
            ("sin", [x]) => Ok(x.sin()),
            ("cos", [x]) => Ok(x.cos()),
            ("radians", [x]) => Ok(x.to_radians()),
            ("clamp", [x, low, high]) => Ok(x.clamp(*low, *high)),
            ("abs" | "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" | "radians", _) => {
                Err(wrong("one value"))
            }
            ("clamp", _) => Err(wrong("three values: clamp(x, low, high)")),
            _ => Err(problem(
                self.span,
                format!("{name} is not a function here"),
                "the functions are min, max, abs, sqrt, floor, ceil, round, clamp, sin, cos and radians",
            )),
        }
    }
}
