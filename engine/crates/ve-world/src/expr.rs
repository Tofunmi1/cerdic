//! Bounded, deterministic field expressions for world sampling (ADR 0060).
mod builtins;
mod lexer;
pub(crate) mod math;
mod noise;
#[path = "expr/parser/root.rs"]
mod parser;

use parser::{Node, compile_program};

/// Maximum UTF-8 source length accepted by the compiler.
pub const MAX_SOURCE_BYTES: usize = 8192;
/// Maximum number of non-leaf operations in a compiled expression.
pub const MAX_OPERATIONS: usize = 256;

/// Expression value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// A single 32-bit float.
    Float,
    /// A two component vector.
    Vec2,
    /// A three component vector.
    Vec3,
    /// A four component vector.
    Vec4,
    /// A boolean value.
    Bool,
}

impl Type {
    pub(super) fn size(self) -> usize {
        match self {
            Self::Float | Self::Bool => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Bool => "bool",
        }
    }
}

/// A scalar, vector, or boolean expression result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// A scalar.
    Float(f32),
    /// A two component vector.
    Vec2([f32; 2]),
    /// A three component vector.
    Vec3([f32; 3]),
    /// A four component vector.
    Vec4([f32; 4]),
    /// A boolean value.
    Bool(bool),
}

impl Value {
    /// Returns the components, encoding `true` as one and `false` as zero.
    #[must_use]
    pub fn floats(&self) -> Vec<f32> {
        match self {
            Self::Float(v) => vec![*v],
            Self::Vec2(v) => v.to_vec(),
            Self::Vec3(v) => v.to_vec(),
            Self::Vec4(v) => v.to_vec(),
            Self::Bool(v) => vec![if *v { 1.0 } else { 0.0 }],
        }
    }
    pub(super) fn ty(self) -> Type {
        match self {
            Self::Float(_) => Type::Float,
            Self::Vec2(_) => Type::Vec2,
            Self::Vec3(_) => Type::Vec3,
            Self::Vec4(_) => Type::Vec4,
            Self::Bool(_) => Type::Bool,
        }
    }
    pub(super) fn array(self) -> [f32; 4] {
        match self {
            Self::Float(v) => [v, 0.0, 0.0, 0.0],
            Self::Vec2(v) => [v[0], v[1], 0.0, 0.0],
            Self::Vec3(v) => [v[0], v[1], v[2], 0.0],
            Self::Vec4(v) => v,
            Self::Bool(v) => [if v { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        }
    }
    pub(super) fn from_array(ty: Type, v: [f32; 4]) -> Self {
        match ty {
            Type::Float => Self::Float(v[0]),
            Type::Vec2 => Self::Vec2([v[0], v[1]]),
            Type::Vec3 => Self::Vec3([v[0], v[1], v[2]]),
            Type::Vec4 => Self::Vec4(v),
            Type::Bool => Self::Bool(v[0] != 0.0),
        }
    }
}

/// The ordered host-provided field bindings visible to a program.
#[derive(Clone, Debug, Default)]
pub struct Bindings {
    /// Named host values in sample slot order.
    pub fields: Vec<(String, Type)>,
}

/// One input sample to an expression.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample<'a> {
    /// World position.
    pub p: [f32; 3],
    /// Surface normal.
    pub n: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Named element attributes.
    pub attrs: &'a [(&'a str, Value)],
    /// Bound field values in [`Bindings`] order.
    pub bound: &'a [Value],
}

/// A source location and actionable compiler diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// One-based source line.
    pub line: usize,
    /// One-based source column.
    pub column: usize,
    /// Explanation of the error.
    pub message: String,
    /// Suggested correction.
    pub suggestion: String,
}

impl Diagnostic {
    pub(super) fn new(
        span: Span,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            line: span.line,
            column: span.column,
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Span {
    pub line: usize,
    pub column: usize,
}

/// Error returned when a non-scalar expression is evaluated as a float batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExprError {
    /// The expression output is not scalar.
    NotFloat,
}

/// An immutable compiled expression.
#[derive(Clone, Debug)]
pub struct Expr {
    root: Node,
    output: Type,
    attrs: Vec<(String, Type)>,
    operations: usize,
    bindings: Bindings,
}

impl Expr {
    /// Compiles a source program and infers the type of its `out` variable.
    ///
    /// # Errors
    ///
    /// Returns source diagnostics when parsing or type checking fails.
    pub fn compile(source: &str, bindings: &Bindings) -> Result<Self, Vec<Diagnostic>> {
        Self::compile_inner(source, bindings, None)
    }
    /// Compiles a source program and requires the output type to match `wanted`.
    ///
    /// # Errors
    ///
    /// Returns source diagnostics when parsing, type checking, or output checking fails.
    pub fn compile_as(
        source: &str,
        bindings: &Bindings,
        wanted: Type,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::compile_inner(source, bindings, Some(wanted))
    }
    fn compile_inner(
        source: &str,
        bindings: &Bindings,
        wanted: Option<Type>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(vec![Diagnostic::new(
                Span { line: 1, column: 1 },
                "source too long, limit 8192 bytes",
                "shorten the expression",
            )]);
        }
        let tokens = lexer::lex(source)?;
        let (root, output, attrs, operations) = compile_program(&tokens, bindings)?;
        if let Some(expected) = wanted
            && output != expected
        {
            return Err(vec![Diagnostic::new(
                root.span,
                format!("wanted {} but found {}", expected.name(), output.name()),
                format!("make out a {} expression", expected.name()),
            )]);
        }
        if operations > MAX_OPERATIONS {
            return Err(vec![Diagnostic::new(
                root.span,
                "expression too complex: too many operations, limit 256",
                "simplify the expression",
            )]);
        }
        Ok(Self {
            root,
            output,
            attrs,
            operations,
            bindings: bindings.clone(),
        })
    }
    /// Returns the inferred output type.
    #[must_use]
    pub fn output_type(&self) -> Type {
        self.output
    }
    /// Returns referenced attributes in first-use order.
    #[must_use]
    pub fn attributes(&self) -> &[(String, Type)] {
        &self.attrs
    }
    /// Returns the count of non-leaf operations.
    #[must_use]
    pub fn operation_count(&self) -> usize {
        self.operations
    }
    /// Evaluates this expression for one sample. Non-finite values are sanitized.
    #[must_use]
    pub fn eval(&self, sample: &Sample<'_>) -> Value {
        self.root.eval(sample, &self.bindings)
    }
    /// Evaluates a scalar expression over samples, clearing and reusing `out`.
    ///
    /// # Errors
    ///
    /// Returns [`ExprError::NotFloat`] when the compiled output is not scalar.
    pub fn eval_floats(&self, samples: &[Sample<'_>], out: &mut Vec<f32>) -> Result<(), ExprError> {
        out.clear();
        if self.output != Type::Float {
            return Err(ExprError::NotFloat);
        }
        out.reserve(samples.len());
        for sample in samples {
            if let Value::Float(v) = self.eval(sample) {
                out.push(v);
            }
        }
        Ok(())
    }
}
