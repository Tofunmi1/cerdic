//! The typed, slot-resolved syntax tree of a SimL program.
use super::{Ty, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bin {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Func {
    Abs,
    Sign,
    Floor,
    Ceil,
    Fract,
    Sqrt,
    Exp,
    Log,
    Sin,
    Cos,
    Min,
    Max,
    Pow,
    Atan2,
    Clamp,
    Mix,
    Step,
    Smoothstep,
    Length,
    Distance,
    Dot,
    Cross,
    Normalize,
    Select,
    Hash,
    Rand,
    SdfCapsule,
    ClosestSegment,
    Pull,
}

pub(super) const FUNCS: [(&str, Func); 29] = [
    ("pull", Func::Pull),
    ("sdf_capsule", Func::SdfCapsule),
    ("closest_segment", Func::ClosestSegment),
    ("abs", Func::Abs),
    ("sign", Func::Sign),
    ("floor", Func::Floor),
    ("ceil", Func::Ceil),
    ("fract", Func::Fract),
    ("sqrt", Func::Sqrt),
    ("exp", Func::Exp),
    ("log", Func::Log),
    ("sin", Func::Sin),
    ("cos", Func::Cos),
    ("min", Func::Min),
    ("max", Func::Max),
    ("pow", Func::Pow),
    ("atan2", Func::Atan2),
    ("clamp", Func::Clamp),
    ("mix", Func::Mix),
    ("step", Func::Step),
    ("smoothstep", Func::Smoothstep),
    ("length", Func::Length),
    ("distance", Func::Distance),
    ("dot", Func::Dot),
    ("cross", Func::Cross),
    ("normalize", Func::Normalize),
    ("select", Func::Select),
    ("hash", Func::Hash),
    ("rand", Func::Rand),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Builtin {
    Dt,
    T,
    Step,
    Seed,
    N,
}

#[derive(Clone, Debug)]
pub(super) enum Kind {
    Num(f32),
    Bool(bool),
    Local(usize),
    Param(usize),
    Builtin(Builtin),
    Elem { arr: usize, idx: Box<Expr> },
    Swizzle(Box<Expr>, Vec<usize>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(Bin, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
    Vector(Vec<Expr>),
    /// A point (or, with `vector`, a direction) carried from a bone's own space to the world by the bone's pose.
    Bone { input: usize, vector: bool, v: Box<Expr> },
    /// Where vertex `idx` of a mesh input was at rest.
    Rest { input: usize, idx: Box<Expr> },
}

#[derive(Clone, Debug)]
pub(super) struct Expr {
    pub(super) kind: Kind,
    pub(super) ty: Ty,
    pub(super) line: usize,
}

#[derive(Clone, Debug)]
pub(super) enum Stmt {
    Let(usize, Expr),
    SetLocal { slot: usize, op: Option<Bin>, value: Expr },
    SetElem { arr: usize, idx: Expr, comp: Option<usize>, op: Option<Bin>, value: Expr, line: usize },
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Repeat { slot: usize, from: i32, to: i32, body: Vec<Stmt> },
    Neighbors { slot: usize, arr: usize, centre: Expr, radius: Expr, cap: u32, body: Vec<Stmt>, line: usize },
    /// Runs the body for each vertex joined by an edge to `of`, with the neighbour's number and the edge's rest length.
    Adjacent { vertex: usize, length: usize, input: usize, of: Expr, body: Vec<Stmt>, line: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InputKind {
    Bone,
    Mesh,
}

#[derive(Debug)]
pub(super) struct InputDecl {
    pub(super) name: String,
    pub(super) kind: InputKind,
}

#[derive(Debug)]
pub(super) struct Kernel {
    pub(super) name: String,
    /// The array whose elements it runs for; none for a kernel that runs once.
    pub(super) over: Option<usize>,
    pub(super) body: Vec<Stmt>,
    pub(super) locals: usize,
    /// Which state arrays it writes.
    pub(super) writes: Vec<bool>,
}

#[derive(Debug)]
pub(super) enum Step {
    Run(usize),
    Repeat(u32, Vec<Step>),
}

#[derive(Debug)]
pub(super) struct ParamDecl {
    pub(super) name: String,
    pub(super) ty: Ty,
    pub(super) default: Value,
}

#[derive(Debug)]
pub(super) enum Count {
    Lit(usize),
    Param(usize),
    /// One element per vertex of a mesh input.
    Mesh(usize),
}

impl Count {
    /// `mesh_points` gives the vertex count of each input (0 for one that is not a bound mesh).
    pub(super) fn resolve(&self, params: &[Value], mesh_points: &[usize]) -> usize {
        match self {
            Self::Lit(n) => *n,
            Self::Mesh(i) => mesh_points.get(*i).copied().unwrap_or(0),
            Self::Param(p) => {
                let x = params[*p].v[0];
                if x.is_finite() && x >= 0.0 {
                    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "checked finite and non-negative; huge values saturate and are refused by the cap")]
                    let n = x as usize;
                    n
                } else {
                    0
                }
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct StateDecl {
    pub(super) name: String,
    pub(super) ty: Ty,
    pub(super) count: Count,
    pub(super) line: usize,
}

#[derive(Debug)]
pub(super) struct Output {
    pub(super) kind: String,
    pub(super) state: usize,
    /// For `push`: the name of the body part the array pushes.
    pub(super) target: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct Program {
    pub(super) name: String,
    pub(super) params: Vec<ParamDecl>,
    pub(super) inputs: Vec<InputDecl>,
    pub(super) states: Vec<StateDecl>,
    pub(super) kernels: Vec<Kernel>,
    pub(super) schedule: Vec<Step>,
    pub(super) init: Option<usize>,
    pub(super) outputs: Vec<Output>,
}
