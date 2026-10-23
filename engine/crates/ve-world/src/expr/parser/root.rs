use std::collections::BTreeMap;

use super::{
    Bindings, Diagnostic, Sample, Span, Type, Value,
    lexer::{Kind, Token},
};
mod calls;
mod check;
mod expression;
use check::check_call;

#[derive(Clone, Debug)]
pub(super) struct Node {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub(super) enum ExprKind {
    Literal(Value),
    Input(Input),
    Unary(Unary, Box<Node>),
    Binary(Binary, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
    Swizzle(Box<Node>, Vec<usize>),
    Attribute(String),
}

#[derive(Clone, Debug)]
pub(super) enum Input {
    Position,
    Normal,
    Uv,
    Bound(usize),
    Pi,
    Tau,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unary {
    Negate,
    Not,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl Node {
    pub(super) fn eval(&self, sample: &Sample<'_>, bindings: &Bindings) -> Value {
        match &self.kind {
            ExprKind::Literal(value) => sanitize(*value),
            ExprKind::Input(input) => match input {
                Input::Position => sanitize(Value::Vec3(sample.p)),
                Input::Normal => sanitize(Value::Vec3(sample.n)),
                Input::Uv => sanitize(Value::Vec2(sample.uv)),
                Input::Bound(index) => sample.bound.get(*index).copied().map_or_else(
                    || zero(self.ty),
                    |value| Value::from_array(self.ty, value.array()),
                ),
                Input::Pi => Value::Float(core::f32::consts::PI),
                Input::Tau => Value::Float(core::f32::consts::TAU),
            },
            ExprKind::Attribute(name) => sample
                .attrs
                .iter()
                .find(|(key, _)| *key == name)
                .map_or_else(|| zero(self.ty), |(_, value)| sanitize(*value)),
            ExprKind::Unary(op, child) => unary(*op, child.eval(sample, bindings)),
            ExprKind::Binary(op, left, right) => super::builtins::binary(
                op_code(*op),
                left.eval(sample, bindings),
                right.eval(sample, bindings),
                self.ty,
            ),
            ExprKind::Call(name, args) => {
                super::builtins::call(name, args, sample, bindings, self.ty)
            }
            ExprKind::Swizzle(child, indices) => {
                let source = child.eval(sample, bindings).array();
                let mut output = [0.0; 4];
                for (i, index) in indices.iter().enumerate() {
                    output[i] = source[*index];
                }
                Value::from_array(self.ty, output)
            }
        }
    }
}

pub(super) fn compile_program(
    tokens: &[Token],
    bindings: &Bindings,
) -> Result<(Node, Type, Vec<(String, Type)>, usize), Vec<Diagnostic>> {
    let mut parser = Parser {
        tokens,
        cursor: 0,
        vars: BTreeMap::new(),
        bindings,
        attrs: Vec::new(),
        operations: 0,
        depth: 0,
        errors: Vec::new(),
    };
    while !matches!(parser.peek().kind, Kind::End) {
        while matches!(parser.peek().kind, Kind::Semi) {
            parser.cursor += 1;
        }
        if matches!(parser.peek().kind, Kind::End) {
            break;
        }
        let token = parser.take();
        let Kind::Name(name) = token.kind else {
            parser.error(
                token.span,
                "expected an assignment name",
                "write name = expression",
            );
            break;
        };
        if !matches!(parser.peek().kind, Kind::Eq) {
            parser.error(
                token.span,
                "expected `=` after assignment name",
                "write name = expression",
            );
            break;
        }
        parser.cursor += 1;
        if matches!(name.as_str(), "p" | "n" | "uv" | "pi" | "tau")
            || bindings.fields.iter().any(|(field, _)| field == &name)
        {
            parser.error(
                token.span,
                format!("cannot assign to input `{name}`"),
                "choose another variable name",
            );
        }
        if parser.vars.contains_key(&name) {
            parser.error(
                token.span,
                format!("variable `{name}` is assigned more than once"),
                "assign each variable once",
            );
        }
        let expr = parser.expression(0);
        parser.vars.insert(name, expr);
        if !matches!(parser.peek().kind, Kind::Semi | Kind::End) {
            parser.error(
                parser.peek().span,
                "expected a statement separator",
                "use `;` or a newline",
            );
            break;
        }
    }
    if let Some(root) = parser.vars.get("out").cloned() {
        if parser.errors.is_empty() {
            Ok((root.clone(), root.ty, parser.attrs, parser.operations))
        } else {
            Err(parser.errors)
        }
    } else {
        parser.errors.push(Diagnostic::new(
            parser.peek().span,
            "program is missing `out`",
            "write out = ...",
        ));
        Err(parser.errors)
    }
}

pub(super) struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
    vars: BTreeMap<String, Node>,
    bindings: &'a Bindings,
    attrs: Vec<(String, Type)>,
    operations: usize,
    depth: usize,
    errors: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.tokens[self.cursor.min(self.tokens.len() - 1)]
    }
    fn take(&mut self) -> Token {
        let token = self.peek().clone();
        self.cursor = (self.cursor + 1).min(self.tokens.len() - 1);
        token
    }
    fn error(&mut self, span: Span, message: impl Into<String>, suggestion: impl Into<String>) {
        self.errors.push(Diagnostic::new(span, message, suggestion));
    }
}

fn literal(value: f32, span: Span) -> Node {
    Node {
        kind: ExprKind::Literal(Value::Float(value)),
        ty: Type::Float,
        span,
    }
}
fn zero(ty: Type) -> Value {
    Value::from_array(ty, [0.0; 4])
}
fn sanitize(value: Value) -> Value {
    let mut array = value.array();
    for item in &mut array {
        if !item.is_finite() {
            *item = if item.is_sign_positive() {
                f32::MAX
            } else {
                -f32::MAX
            };
        }
    }
    Value::from_array(value.ty(), array)
}
fn unary(op: Unary, value: Value) -> Value {
    if matches!(op, Unary::Not) {
        return Value::Bool(!matches!(value, Value::Bool(true)));
    }
    let mut array = value.array();
    for item in &mut array {
        *item = -*item;
    }
    sanitize(Value::from_array(value.ty(), array))
}
fn op_code(op: Binary) -> u8 {
    match op {
        Binary::Add => 0,
        Binary::Sub => 1,
        Binary::Mul => 2,
        Binary::Div => 3,
        Binary::Mod => 4,
        Binary::Eq => 5,
        Binary::Ne => 6,
        Binary::Lt => 7,
        Binary::Le => 8,
        Binary::Gt => 9,
        Binary::Ge => 10,
        Binary::And => 11,
        Binary::Or => 12,
    }
}
impl From<Op> for Binary {
    fn from(op: Op) -> Self {
        match op {
            Op::Add => Self::Add,
            Op::Sub => Self::Sub,
            Op::Mul => Self::Mul,
            Op::Div => Self::Div,
            Op::Mod => Self::Mod,
            Op::Eq => Self::Eq,
            Op::Ne => Self::Ne,
            Op::Lt => Self::Lt,
            Op::Le => Self::Le,
            Op::Gt => Self::Gt,
            Op::Ge => Self::Ge,
            Op::And => Self::And,
            Op::Or => Self::Or,
        }
    }
}
