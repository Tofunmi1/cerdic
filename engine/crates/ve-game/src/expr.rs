//! Small arithmetic and comparison expression evaluator.
#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Number(f64),
    Name(String),
    Unary(char, Box<Self>),
    Binary(Box<Self>, String, Box<Self>),
}
impl Expr {
    pub(crate) fn parse(s: &str) -> Result<Self, String> {
        let tokens = tokenize(s)?;
        let mut p = Parser { t: tokens, i: 0 };
        let e = p.expr(0)?;
        if p.i != p.t.len() {
            return Err("unexpected token; remove trailing text".into());
        }
        Ok(e)
    }
    pub(crate) fn eval(&self, lookup: &impl Fn(&str) -> f64) -> f64 {
        match self {
            Self::Number(n) => *n,
            Self::Name(n) => lookup(n),
            Self::Unary('-', e) => -e.eval(lookup),
            Self::Unary('!', e) => f64::from(e.eval(lookup) == 0.0),
            Self::Unary(_, e) => e.eval(lookup),
            Self::Binary(a, op, b) => {
                let (x, y) = (a.eval(lookup), b.eval(lookup));
                match op.as_str() {
                    "+" => x + y,
                    "-" => x - y,
                    "*" => x * y,
                    "/" => {
                        if y == 0.0 {
                            0.0
                        } else {
                            x / y
                        }
                    }
                    "<" => f64::from(x < y),
                    "<=" => f64::from(x <= y),
                    ">" => f64::from(x > y),
                    ">=" => f64::from(x >= y),
                    "==" => f64::from(numeric_equal(x, y)),
                    "!=" => f64::from(!numeric_equal(x, y)),
                    "and" => f64::from(x != 0.0 && y != 0.0),
                    "or" => f64::from(x != 0.0 || y != 0.0),
                    _ => 0.0,
                }
            }
        }
    }
}
fn numeric_equal(left: f64, right: f64) -> bool {
    let left_bits = left.to_bits();
    let right_bits = right.to_bits();
    left_bits == right_bits || left_bits << 1 == 0 && right_bits << 1 == 0
}
fn tokenize(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if c[i].is_ascii_digit() || c[i] == '.' {
            i += 1;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
        } else if c[i].is_ascii_alphabetic() || c[i] == '_' {
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_' || c[i] == '.') {
                i += 1;
            }
        } else if i + 1 < c.len() && matches!((c[i], c[i + 1]), ('=' | '!' | '<' | '>', '=')) {
            i += 2;
        } else if "+-*/()<>!".contains(c[i]) {
            i += 1;
        } else {
            return Err(format!(
                "unexpected character '{}'; use numbers, names and operators",
                c[i]
            ));
        }
        out.push(c[start..i].iter().collect());
    }
    Ok(out)
}
struct Parser {
    t: Vec<String>,
    i: usize,
}
impl Parser {
    fn expr(&mut self, min: u8) -> Result<Expr, String> {
        let token = self.t.get(self.i).ok_or("expected an expression")?.clone();
        self.i += 1;
        let mut lhs = if token == "(" {
            let e = self.expr(0)?;
            if self.t.get(self.i).map(String::as_str) != Some(")") {
                return Err("missing ')'; close the parenthesized expression".into());
            }
            self.i += 1;
            e
        } else if token == "-" || token == "not" || token == "!" {
            Expr::Unary(
                if token == "-" { '-' } else { '!' },
                Box::new(self.expr(7)?),
            )
        } else if let Ok(n) = token.parse() {
            Expr::Number(n)
        } else if matches!(
            token.as_str(),
            "+" | "*" | "/" | "<" | ">" | "<=" | ">=" | "==" | "!=" | "and" | "or"
        ) {
            return Err("expected a number, name or parenthesized expression".into());
        } else {
            Expr::Name(token)
        };
        while let Some(op) = self.t.get(self.i) {
            let prec = precedence(op);
            if prec < min {
                break;
            }
            let op = op.clone();
            self.i += 1;
            let rhs = self.expr(prec + 1)?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
        }
        Ok(lhs)
    }
}
fn precedence(op: &str) -> u8 {
    match op {
        "or" => 1,
        "and" => 2,
        "==" | "!=" | "<" | "<=" | ">" | ">=" => 3,
        "+" | "-" => 4,
        "*" | "/" => 5,
        _ => 0,
    }
}
