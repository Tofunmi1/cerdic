//! The deterministic CPU executor of SimL programs.
//!
//! Semantics shared with any parallel executor: a kernel run for each element reads the state as it was when the kernel started (a
//! snapshot of the arrays it writes) plus its own writes to its own element, and it may write only its own element. So the order the
//! elements run in cannot change the result.
#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "element numbers are small non-negative numbers carried as f32 in the language"
)]
use super::ast::{Bin, Builtin, Expr, Kind, Program, Stmt, Step};
use super::grid::Grid;
use super::inputs::Inputs;
use super::ops::{arith, call, map};
use super::{MAX_WORK_PER_ELEMENT, RunError, Ty, Value};

/// The time inputs of one step.
#[derive(Clone, Copy, Debug)]
pub(super) struct Clock {
    pub(super) dt: f32,
    pub(super) t: f32,
    pub(super) step: f32,
    pub(super) seed: u32,
}

struct Machine<'a> {
    prog: &'a Program,
    state: &'a mut Vec<Vec<f32>>,
    snap: Vec<Vec<f32>>,
    writes: &'a [bool],
    params: &'a [Value],
    inputs: &'a Inputs,
    clock: Clock,
    frame: Vec<Value>,
    /// The element this run belongs to; `usize::MAX` for a kernel that runs once.
    elem: usize,
    /// How many elements the kernel runs for (the builtin `n`).
    count: usize,
    /// The neighbour grid, built on the first `neighbors` loop of the kernel.
    grid: Option<Grid>,
    budget: u32,
}

fn fail<T>(line: usize, message: impl Into<String>) -> Result<T, RunError> {
    Err(RunError { line, message: message.into() })
}

fn vec3(v: [f32; 3]) -> Value {
    Value { ty: Ty::Vec3, v: [v[0], v[1], v[2], 0.0] }
}

impl Machine<'_> {
    fn spend(&mut self, line: usize) -> Result<(), RunError> {
        if self.budget == 0 {
            return fail(line, format!("one element ran more than {MAX_WORK_PER_ELEMENT} statements in one kernel"));
        }
        self.budget -= 1;
        Ok(())
    }

    fn element_index(&self, arr: usize, at: f32, line: usize) -> Result<usize, RunError> {
        let width = self.prog.states[arr].ty.width();
        let len = self.state[arr].len() / width;
        if !at.is_finite() || at < 0.0 || at >= len as f32 {
            return fail(line, format!("index {at} is outside `{}` (0 to {})", self.prog.states[arr].name, len - 1));
        }
        Ok(at as usize)
    }

    fn mesh(&self, input: usize, line: usize) -> Result<&super::inputs::MeshData, RunError> {
        match self.inputs.meshes.get(input).and_then(Option::as_ref) {
            Some(mesh) => Ok(mesh),
            None => fail(line, format!("no mesh is bound to input `{}`", self.prog.inputs[input].name)),
        }
    }

    fn read(&self, arr: usize, idx: usize) -> Value {
        let ty = self.prog.states[arr].ty;
        let width = ty.width();
        let source = if self.writes[arr] && idx != self.elem { &self.snap[arr] } else { &self.state[arr] };
        let mut v = [0.0; 4];
        v[..width].copy_from_slice(&source[idx * width..(idx + 1) * width]);
        Value { ty, v }
    }

    fn write(&mut self, arr: usize, idx: usize, comp: Option<usize>, value: Value) {
        let width = self.prog.states[arr].ty.width();
        let base = idx * width;
        match comp {
            Some(c) => self.state[arr][base + c] = value.v[0],
            None => self.state[arr][base..base + width].copy_from_slice(&value.v[..width]),
        }
    }

    fn eval(&mut self, e: &Expr) -> Result<Value, RunError> {
        Ok(match &e.kind {
            Kind::Num(x) => Value::float(*x),
            Kind::Bool(b) => Value::boolean(*b),
            Kind::Local(slot) => self.frame[*slot],
            Kind::Param(p) => self.params[*p],
            Kind::Builtin(b) => Value::float(match b {
                Builtin::Dt => self.clock.dt,
                Builtin::T => self.clock.t,
                Builtin::Step => self.clock.step,
                Builtin::Seed => self.clock.seed as f32,
                Builtin::N => self.count as f32,
            }),
            Kind::Elem { arr, idx } => {
                let at = self.eval(idx)?.v[0];
                let i = self.element_index(*arr, at, e.line)?;
                self.read(*arr, i)
            }
            Kind::Swizzle(inner, picks) => {
                let v = self.eval(inner)?;
                let mut out = [0.0; 4];
                for (slot, &p) in out.iter_mut().zip(picks) {
                    *slot = v.v[p];
                }
                Value { ty: e.ty, v: out }
            }
            Kind::Neg(inner) => map(self.eval(inner)?, |x| -x),
            Kind::Not(inner) => Value::boolean(self.eval(inner)?.v[0] == 0.0),
            Kind::Bin(op, l, r) => {
                let (a, b) = (self.eval(l)?, self.eval(r)?);
                match op {
                    Bin::Or => Value::boolean(a.v[0] != 0.0 || b.v[0] != 0.0),
                    Bin::And => Value::boolean(a.v[0] != 0.0 && b.v[0] != 0.0),
                    Bin::Eq => Value::boolean(a.v[0] == b.v[0]),
                    Bin::Ne => Value::boolean(a.v[0] != b.v[0]),
                    Bin::Lt => Value::boolean(a.v[0] < b.v[0]),
                    Bin::Le => Value::boolean(a.v[0] <= b.v[0]),
                    Bin::Gt => Value::boolean(a.v[0] > b.v[0]),
                    Bin::Ge => Value::boolean(a.v[0] >= b.v[0]),
                    _ => arith(*op, a, b, e.ty),
                }
            }
            Kind::Vector(parts) => {
                let mut out = [0.0; 4];
                let mut at = 0;
                for part in parts {
                    let v = self.eval(part)?;
                    for k in 0..v.ty.width() {
                        out[at] = v.v[k];
                        at += 1;
                    }
                }
                if parts.len() == 1 && parts[0].ty == Ty::Float {
                    out = [out[0]; 4];
                }
                Value { ty: e.ty, v: out }
            }
            Kind::Call(f, args) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                call(*f, &vals, e.ty, self.clock.seed)
            }
            Kind::Bone { input, vector, v } => {
                let x = self.eval(v)?;
                vec3(self.inputs.carry(*input, [x.v[0], x.v[1], x.v[2]], if *vector { 0.0 } else { 1.0 }))
            }
            Kind::Rest { input, idx } => {
                let at = self.eval(idx)?.v[0];
                let mesh = self.mesh(*input, e.line)?;
                let point = if at.is_finite() && at >= 0.0 { mesh.point(at as usize) } else { None };
                match point {
                    Some(p) => vec3(p),
                    None => return fail(e.line, format!("point {at} is outside the mesh `{}`", self.prog.inputs[*input].name)),
                }
            }
        })
    }

    /// Elements that may be within `r` of `centre`, ascending: from the grid (built on the first query of the kernel) or every element.
    fn neighbour_candidates(&mut self, arr: usize, len: usize, centre: [f32; 3], r: f32) -> Vec<usize> {
        if self.grid.as_ref().is_none_or(|g| g.arr != arr) {
            let this = &*self;
            let built = Grid::build(arr, len, r, |j| {
                let p = this.read(arr, j);
                [p.v[0], p.v[1], p.v[2]]
            });
            self.grid = built;
        }
        let mut found = self.grid.as_ref().and_then(|g| g.candidates(centre, r)).unwrap_or_else(|| (0..len).collect());
        // This element's own position may have been written since the grid was built.
        if self.elem < len && found.binary_search(&self.elem).is_err() {
            found.push(self.elem);
            found.sort_unstable();
        }
        found
    }

    fn block(&mut self, body: &[Stmt]) -> Result<(), RunError> {
        for s in body {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), RunError> {
        match s {
            Stmt::Let(slot, e) => {
                self.spend(e.line)?;
                self.frame[*slot] = self.eval(e)?;
            }
            Stmt::SetLocal { slot, op, value } => {
                self.spend(value.line)?;
                let v = self.eval(value)?;
                let old = self.frame[*slot];
                self.frame[*slot] = match op {
                    Some(op) => arith(*op, old, v, old.ty),
                    None => v,
                };
            }
            Stmt::SetElem { arr, idx, comp, op, value, line } => {
                self.spend(*line)?;
                let at = self.eval(idx)?.v[0];
                let i = self.element_index(*arr, at, *line)?;
                if self.elem != usize::MAX && i != self.elem {
                    return fail(*line, format!("a kernel may write only its own element ({}), not {i}", self.elem));
                }
                let v = self.eval(value)?;
                let ty = self.prog.states[*arr].ty;
                let new = match op {
                    Some(op) => {
                        let old = self.read(*arr, i);
                        match comp {
                            Some(c) => arith(*op, Value::float(old.v[*c]), v, Ty::Float),
                            None => arith(*op, old, v, ty),
                        }
                    }
                    None => v,
                };
                self.write(*arr, i, *comp, new);
            }
            Stmt::If(cond, then, other) => {
                self.spend(cond.line)?;
                if self.eval(cond)?.v[0] != 0.0 {
                    self.block(then)?;
                } else {
                    self.block(other)?;
                }
            }
            Stmt::Repeat { slot, from, to, body } => {
                for k in *from..*to {
                    self.spend(0)?;
                    self.frame[*slot] = Value::float(k as f32);
                    self.block(body)?;
                }
            }
            Stmt::Neighbors { slot, arr, centre, radius, cap, body, line } => {
                self.spend(*line)?;
                let c = self.eval(centre)?;
                let r = self.eval(radius)?.v[0];
                let len = self.state[*arr].len() / 3;
                let mut visited = 0;
                for j in self.neighbour_candidates(*arr, len, [c.v[0], c.v[1], c.v[2]], r) {
                    let p = self.read(*arr, j);
                    let d = (p.v[0] - c.v[0], p.v[1] - c.v[1], p.v[2] - c.v[2]);
                    if d.0 * d.0 + d.1 * d.1 + d.2 * d.2 > r * r {
                        continue;
                    }
                    self.spend(*line)?;
                    self.frame[*slot] = Value::float(j as f32);
                    self.block(body)?;
                    visited += 1;
                    if visited >= *cap {
                        break;
                    }
                }
            }
            Stmt::Adjacent { vertex, length, input, of, body, line } => {
                self.spend(*line)?;
                let at = self.eval(of)?.v[0];
                let inputs = self.inputs;
                let mesh = self.mesh(*input, *line)?;
                if !(at.is_finite() && at >= 0.0 && (at as usize) < mesh.len()) {
                    return fail(*line, format!("point {at} is outside the mesh `{}`", self.prog.inputs[*input].name));
                }
                let joined = inputs.meshes[*input].as_ref().map_or(&[][..], |m| m.neighbours(at as usize));
                for &(j, len) in joined {
                    self.spend(*line)?;
                    self.frame[*vertex] = Value::float(j as f32);
                    self.frame[*length] = Value::float(len);
                    self.block(body)?;
                }
            }
        }
        Ok(())
    }
}

/// Evaluates a constant expression (a parameter default) given the parameters declared before it.
pub(super) fn constant(e: &Expr, params: &[Value]) -> Result<Value, String> {
    let prog = Program::default();
    let (mut state, inputs) = (Vec::new(), Inputs::default());
    let mut m = Machine {
        prog: &prog,
        state: &mut state,
        snap: Vec::new(),
        writes: &[],
        params,
        inputs: &inputs,
        clock: Clock { dt: 0.0, t: 0.0, step: 0.0, seed: 0 },
        frame: Vec::new(),
        elem: usize::MAX,
        count: 0,
        grid: None,
        budget: MAX_WORK_PER_ELEMENT,
    };
    m.eval(e).map_err(|err| err.message)
}

/// Runs kernel `index` for every element of its array (or once).
pub(super) fn kernel(prog: &Program, state: &mut Vec<Vec<f32>>, params: &[Value], inputs: &Inputs, index: usize, clock: Clock) -> Result<(), RunError> {
    let k = &prog.kernels[index];
    let snap = k.writes.iter().enumerate().map(|(a, w)| if *w { state[a].clone() } else { Vec::new() }).collect();
    let count = k.over.map_or(1, |a| state[a].len() / prog.states[a].ty.width());
    let mut m = Machine {
        prog,
        state,
        snap,
        writes: &k.writes,
        params,
        inputs,
        clock,
        frame: vec![Value::float(0.0); k.locals],
        elem: usize::MAX,
        count,
        grid: None,
        budget: MAX_WORK_PER_ELEMENT,
    };
    for e in 0..count {
        m.budget = MAX_WORK_PER_ELEMENT;
        if k.over.is_some() {
            m.elem = e;
            m.frame[0] = Value::float(e as f32);
        }
        m.block(&k.body)?;
    }
    Ok(())
}

/// Runs a schedule once.
pub(super) fn schedule(prog: &Program, state: &mut Vec<Vec<f32>>, params: &[Value], inputs: &Inputs, steps: &[Step], clock: Clock) -> Result<(), RunError> {
    for s in steps {
        match s {
            Step::Run(k) => kernel(prog, state, params, inputs, *k, clock)?,
            Step::Repeat(n, inner) => {
                for _ in 0..*n {
                    schedule(prog, state, params, inputs, inner, clock)?;
                }
            }
        }
    }
    Ok(())
}
