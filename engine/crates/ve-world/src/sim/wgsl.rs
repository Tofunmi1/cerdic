//! SimL to WGSL: one compute shader per kernel, the GPU executor of the same programs the CPU reference runs.
//!
//! Binding contract (group 0): 0 = `Clock` uniform, 1 = `params` (read-only f32s, every param at its [`GpuPlan::param_offsets`] offset), and for
//! state array `a`: `2 + 2a` = the live array (read/write when the kernel writes it, read-only otherwise) and `3 + 2a` = a snapshot of it taken
//! when the kernel started, bound only for arrays the kernel writes. A kernel reads other elements from the snapshot and its own element from
//! the live array, exactly the CPU rule, so the two executors agree (up to the GPU's own `sin`, `exp`, `log`, `pow`).
//! Programs that use mesh inputs, bones or `adjacent` are refused here and keep running on the CPU.
mod prelude;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use prelude::{PRELUDE, bone_helper, float_lit, helpers, mesh_helper, splat, tag, wty};

use super::Ty;
use super::ast::{Bin, Builtin, Count, Expr, Func, InputKind, Kind, Kernel, Program, Stmt, Step};

/// What the host needs to run one kernel.
#[derive(Debug)]
pub struct GpuKernel {
    /// The kernel's name.
    pub name: String,
    /// The WGSL source (entry point `main`, workgroup size 64).
    pub wgsl: String,
    /// The state array whose element count is the number of threads; none for a kernel that runs once.
    pub over: Option<usize>,
    /// (state array, bound to read/write, snapshot also bound): the arrays the kernel touches.
    pub arrays: Vec<(usize, bool, bool)>,
    /// The bone poses and meshes the kernel reads, and the binding each goes to.
    pub inputs: Vec<GpuBinding>,
}

/// One input bound to a kernel.
#[derive(Debug, PartialEq, Eq)]
pub struct GpuBinding {
    /// The binding number (group 0).
    pub binding: u32,
    /// The program's input number.
    pub input: usize,
    /// True for the one buffer of bone poses (16 f32s per input number, column-major), false for a mesh (see `MeshData::gpu_words`).
    pub bone: bool,
}

/// One step of the schedule.
#[derive(Debug)]
pub enum GpuStep {
    /// Run kernel number `n` of [`GpuPlan::kernels`].
    Run(usize),
    /// Repeat the inner steps.
    Repeat(u32, Vec<GpuStep>),
}

/// One state array as the host allocates it.
#[derive(Debug)]
pub struct GpuState {
    /// The array's name.
    pub name: String,
    /// f32s per element (a bool takes one).
    pub width: usize,
    /// Elements, with every param at its default (0 when the array has one element per vertex of a mesh input).
    pub count: usize,
    /// The mesh input whose vertex count is this array's element count.
    pub mesh: Option<usize>,
}

/// One `input` of the program.
#[derive(Debug)]
pub struct GpuInput {
    /// The input's name.
    pub name: String,
    /// A bone pose (true) or a mesh (false).
    pub bone: bool,
}

/// Everything the host needs to run a program on the GPU.
#[derive(Debug)]
pub struct GpuPlan {
    /// The state arrays, by the numbers kernels refer to them with.
    pub states: Vec<GpuState>,
    /// The program's inputs, by input number.
    pub inputs: Vec<GpuInput>,
    /// Every param's default numbers, flat, laid out by `param_offsets`.
    pub params: Vec<f32>,
    /// The kernels, in declaration order.
    pub kernels: Vec<GpuKernel>,
    /// The schedule over them.
    pub schedule: Vec<GpuStep>,
    /// Where each param starts in the params buffer, and how many f32s it takes.
    pub param_offsets: Vec<(usize, usize)>,
    /// Index of the `init` kernel, run once at the start.
    pub init: Option<usize>,
}

struct Gen<'a> {
    prog: &'a Program,
    param_offsets: &'a [(usize, usize)],
    /// (read, written) per state array used by this kernel.
    used: BTreeMap<usize, (bool, bool)>,
    /// The type of each local slot as the code emitted so far declared it.
    locals: BTreeMap<usize, Ty>,
    /// The inputs this kernel reads: meshes by input number, and whether any bone pose is read.
    meshes: BTreeSet<usize>,
    bones: bool,
}

impl Gen<'_> {
    fn bin(op: Bin, a: &str, at: Ty, b: &str, bt: Ty, rt: Ty) -> String {
        let sym = match op {
            Bin::Or => "||",
            Bin::And => "&&",
            Bin::Eq => "==",
            Bin::Ne => "!=",
            Bin::Lt => "<",
            Bin::Le => "<=",
            Bin::Gt => ">",
            Bin::Ge => ">=",
            Bin::Add => "+",
            Bin::Sub => "-",
            Bin::Mul => "*",
            Bin::Div | Bin::Rem => {
                let (x, y) = (splat(a, at, rt), splat(b, bt, rt));
                return format!("{}_{}({x}, {y})", if op == Bin::Div { "div" } else { "rem" }, tag(rt));
            }
        };
        if rt == Ty::Bool || matches!(op, Bin::Or | Bin::And) {
            return format!("({a} {sym} {b})");
        }
        format!("({} {sym} {})", splat(a, at, rt), splat(b, bt, rt))
    }

    fn expr(&mut self, e: &Expr) -> Result<String, String> {
        Ok(match &e.kind {
            Kind::Num(x) => float_lit(*x),
            Kind::Bool(b) => b.to_string(),
            Kind::Local(slot) => format!("l{slot}"),
            Kind::Param(p) => {
                let (at, w) = self.param_offsets[*p];
                let ty = e.ty;
                if w == 1 {
                    let v = format!("params[{at}]");
                    if ty == Ty::Bool { format!("({v} != 0.0)") } else { v }
                } else {
                    format!("{}({})", wty(ty), (0..w).map(|k| format!("params[{}]", at + k)).collect::<Vec<_>>().join(", "))
                }
            }
            Kind::Builtin(b) => match b {
                Builtin::Dt => "clock.dt".to_owned(),
                Builtin::T => "clock.t".to_owned(),
                Builtin::Step => "clock.step".to_owned(),
                Builtin::Seed => "f32(clock.seed)".to_owned(),
                Builtin::N => "f32(clock.count)".to_owned(),
            },
            Kind::Elem { arr, idx } => {
                self.used.entry(*arr).or_default().0 = true;
                format!("rd{arr}(to_u({}), elem)", self.expr(idx)?)
            }
            Kind::Swizzle(inner, picks) => {
                let letters: String = picks.iter().map(|&p| ["x", "y", "z", "w"][p]).collect();
                format!("({}).{letters}", self.expr(inner)?)
            }
            Kind::Neg(inner) => format!("(-{})", self.expr(inner)?),
            Kind::Not(inner) => format!("(!{})", self.expr(inner)?),
            Kind::Bin(op, l, r) => {
                let (a, b) = (self.expr(l)?, self.expr(r)?);
                Self::bin(*op, &a, l.ty, &b, r.ty, e.ty)
            }
            Kind::Vector(parts) => {
                let args: Result<Vec<String>, String> = parts.iter().map(|p| self.expr(p)).collect();
                format!("{}({})", wty(e.ty), args?.join(", "))
            }
            Kind::Call(f, args) => self.call(*f, args, e.ty)?,
            Kind::Bone { input, vector, v } => {
                self.bones = true;
                format!("bone_xf({input}u, {}, {})", self.expr(v)?, if *vector { "0.0" } else { "1.0" })
            }
            Kind::Rest { input, idx } => {
                self.meshes.insert(*input);
                format!("rest{input}(to_u({}))", self.expr(idx)?)
            }
        })
    }

    fn call(&mut self, f: Func, args: &[Expr], ty: Ty) -> Result<String, String> {
        let a: Result<Vec<String>, String> = args.iter().map(|x| self.expr(x)).collect();
        let a = a?;
        let at = |i: usize| splat(&a[i], args[i].ty, ty);
        let zero = if ty == Ty::Float { "0.0".to_owned() } else { format!("{}(0.0)", wty(ty)) };
        Ok(match f {
            Func::Abs => format!("abs({})", a[0]),
            Func::Sign => format!("sign({})", a[0]),
            Func::Floor => format!("floor({})", a[0]),
            Func::Ceil => format!("ceil({})", a[0]),
            Func::Fract => format!("({0} - floor({0}))", a[0]),
            Func::Sqrt => format!("sqrt(max({}, {zero}))", a[0]),
            Func::Exp => format!("exp(min({}, {}))", a[0], splat("88.0", Ty::Float, ty)),
            Func::Log => format!("select(log(max({0}, {1})), {zero}, {0} <= {zero})", a[0], splat("1e-30", Ty::Float, ty)),
            Func::Sin => format!("sin({})", a[0]),
            Func::Cos => format!("cos({})", a[0]),
            Func::Min => format!("min({}, {})", at(0), at(1)),
            Func::Max => format!("max({}, {})", at(0), at(1)),
            Func::Pow => format!("select(pow(max({0}, {zero}), {1}), {zero}, {0} <= {zero})", at(0), at(1)),
            Func::Atan2 => format!("atan2({}, {})", at(0), at(1)),
            Func::Step => format!("step({}, {})", at(0), at(1)),
            Func::Clamp => format!("min(max({}, {}), {})", at(0), at(1), at(2)),
            Func::Mix => format!("({0} + ({1} - {0}) * {2})", at(0), at(1), at(2)),
            Func::Smoothstep => format!("ss_{}({}, {}, {})", tag(ty), at(0), at(1), at(2)),
            Func::Length => format!("length({})", a[0]),
            Func::Distance => format!("distance({}, {})", a[0], a[1]),
            Func::Dot => {
                if args[0].ty == Ty::Float { format!("({} * {})", a[0], a[1]) } else { format!("dot({}, {})", a[0], a[1]) }
            }
            Func::Cross => format!("cross({}, {})", a[0], a[1]),
            Func::Normalize => format!("nrm_{}({})", tag(ty), a[0]),
            Func::Select => format!("select({}, {}, {})", a[2], a[1], a[0]),
            Func::Hash => format!("f32(hash_u32(to_u({}), to_u({}), clock.seed) >> 8u)", a[0], a[1]),
            Func::Rand => format!("(f32(hash_u32(to_u({}), to_u({}), clock.seed) >> 8u) / 16777216.0)", a[0], a[1]),
            Func::SdfCapsule => format!("(length({0} - closest_seg({0}, {1}, {2})) - {3})", a[0], a[1], a[2], a[3]),
            Func::ClosestSegment => format!("closest_seg({}, {}, {})", a[0], a[1], a[2]),
            Func::Pull => format!("pull_move({}, {}, {})", a[0], a[1], a[2]),
        })
    }

    fn block(&mut self, body: &[Stmt], out: &mut String, depth: usize) -> Result<(), String> {
        for s in body {
            self.stmt(s, out, depth)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt, out: &mut String, depth: usize) -> Result<(), String> {
        let pad = "  ".repeat(depth);
        match s {
            Stmt::Let(slot, e) => {
                let v = self.expr(e)?;
                self.locals.insert(*slot, e.ty);
                let _ = writeln!(out, "{pad}var l{slot}: {} = {v};", wty(e.ty));
            }
            Stmt::SetLocal { slot, op, value } => {
                let v = self.expr(value)?;
                let ty = self.locals.get(slot).copied().unwrap_or(value.ty);
                let new = match op {
                    Some(op) => Self::bin(*op, &format!("l{slot}"), ty, &v, value.ty, ty),
                    None => v,
                };
                let _ = writeln!(out, "{pad}l{slot} = {new};");
            }
            Stmt::SetElem { arr, idx, comp, op, value, .. } => {
                self.used.entry(*arr).or_default().1 = true;
                let ix = self.expr(idx)?;
                let v = self.expr(value)?;
                let ty = self.prog.states[*arr].ty;
                let width = ty.width();
                let _ = writeln!(out, "{pad}{{");
                let _ = writeln!(out, "{pad}  let ix = to_u({ix});");
                let _ = writeln!(out, "{pad}  if (elem == 0xffffffffu || ix == elem) {{");
                let value_code = match (op, comp) {
                    (Some(op), Some(c)) => Self::bin(*op, &format!("S{arr}[ix * {width}u + {c}u]"), Ty::Float, &v, value.ty, Ty::Float),
                    (Some(op), None) => Self::bin(*op, &format!("rdl{arr}(ix)"), ty, &v, value.ty, ty),
                    (None, _) => v,
                };
                match comp {
                    Some(c) => {
                        let _ = writeln!(out, "{pad}    S{arr}[ix * {width}u + {c}u] = {value_code};");
                    }
                    None => {
                        let _ = writeln!(out, "{pad}    let nv = {value_code};");
                        for k in 0..width {
                            let part = if width == 1 { "nv".to_owned() } else { format!("nv[{k}]") };
                            let part = if ty == Ty::Bool { format!("select(0.0, 1.0, {part})") } else { part };
                            let _ = writeln!(out, "{pad}    S{arr}[ix * {width}u + {k}u] = {part};");
                        }
                    }
                }
                let _ = writeln!(out, "{pad}  }}\n{pad}}}");
            }
            Stmt::If(cond, then, other) => {
                let c = self.expr(cond)?;
                let _ = writeln!(out, "{pad}if ({c}) {{");
                self.block(then, out, depth + 1)?;
                if other.is_empty() {
                    let _ = writeln!(out, "{pad}}}");
                } else {
                    let _ = writeln!(out, "{pad}}} else {{");
                    self.block(other, out, depth + 1)?;
                    let _ = writeln!(out, "{pad}}}");
                }
            }
            Stmt::Repeat { slot, from, to, body } => {
                self.locals.insert(*slot, Ty::Float);
                let _ = writeln!(out, "{pad}for (var k{slot}: i32 = {from}; k{slot} < {to}; k{slot} = k{slot} + 1) {{");
                let _ = writeln!(out, "{pad}  var l{slot}: f32 = f32(k{slot});");
                self.block(body, out, depth + 1)?;
                let _ = writeln!(out, "{pad}}}");
            }
            Stmt::Neighbors { slot, arr, centre, radius, cap, body, .. } => {
                self.used.entry(*arr).or_default().0 = true;
                self.locals.insert(*slot, Ty::Float);
                let (c, r) = (self.expr(centre)?, self.expr(radius)?);
                let _ = writeln!(out, "{pad}{{\n{pad}  let c = {c}; let r = {r}; var visited: u32 = 0u;");
                let _ = writeln!(out, "{pad}  let len = arrayLength(&S{arr}) / 3u;");
                let _ = writeln!(out, "{pad}  for (var j: u32 = 0u; j < len; j = j + 1u) {{");
                let _ = writeln!(out, "{pad}    let p = rd{arr}(j, elem); let d = p - c;");
                let _ = writeln!(out, "{pad}    if (dot(d, d) > r * r) {{ continue; }}");
                let _ = writeln!(out, "{pad}    var l{slot}: f32 = f32(j);");
                self.block(body, out, depth + 2)?;
                let _ = writeln!(out, "{pad}    visited = visited + 1u;\n{pad}    if (visited >= {cap}u) {{ break; }}\n{pad}  }}\n{pad}}}");
            }
            Stmt::Adjacent { vertex, length, input, of, body, .. } => {
                self.meshes.insert(*input);
                self.locals.insert(*vertex, Ty::Float);
                self.locals.insert(*length, Ty::Float);
                let at = self.expr(of)?;
                let _ = writeln!(out, "{pad}{{\n{pad}  let av = min(to_u({at}), M{input}[0] - 1u);");
                let _ = writeln!(out, "{pad}  for (var e: u32 = M{input}[M{input}[1] + av]; e < M{input}[M{input}[1] + av + 1u]; e = e + 1u) {{");
                let _ = writeln!(out, "{pad}    var l{vertex}: f32 = f32(M{input}[M{input}[2] + e]);");
                let _ = writeln!(out, "{pad}    var l{length}: f32 = bitcast<f32>(M{input}[M{input}[3] + e]);");
                self.block(body, out, depth + 2)?;
                let _ = writeln!(out, "{pad}  }}\n{pad}}}");
            }
        }
        Ok(())
    }
}

fn kernel(prog: &Program, k: &Kernel, offsets: &[(usize, usize)]) -> Result<GpuKernel, String> {
    let mut g = Gen { prog, param_offsets: offsets, used: BTreeMap::new(), locals: BTreeMap::new(), meshes: BTreeSet::new(), bones: false };
    if k.over.is_some() {
        g.locals.insert(0, Ty::Float);
    }
    let mut body = String::new();
    g.block(&k.body, &mut body, 2)?;
    let mut src = String::from(PRELUDE);
    src.push_str(&helpers());
    let mut arrays = Vec::new();
    for (&a, &(_, written)) in &g.used {
        let w = prog.states[a].ty.width();
        let ty = prog.states[a].ty;
        let writes = k.writes[a] && written;
        if writes {
            let _ = writeln!(src, "@group(0) @binding({}) var<storage, read_write> S{a}: array<f32>;", 2 + 2 * a);
            let _ = writeln!(src, "@group(0) @binding({}) var<storage, read> Snap{a}: array<f32>;", 3 + 2 * a);
        } else {
            let _ = writeln!(src, "@group(0) @binding({}) var<storage, read> S{a}: array<f32>;", 2 + 2 * a);
        }
        arrays.push((a, writes, writes));
        let load = |name: &str| {
            let comps: Vec<String> = (0..w).map(|c| format!("{name}{a}[j * {w}u + {c}u]")).collect();
            if ty == Ty::Bool { format!("({} != 0.0)", comps[0]) } else if w == 1 { comps[0].clone() } else { format!("{}({})", wty(ty), comps.join(", ")) }
        };
        let live = load("S");
        let _ = writeln!(src, "fn rdl{a}(j: u32) -> {} {{ return {live}; }}", wty(ty));
        let other = if writes { load("Snap") } else { live.clone() };
        let _ = writeln!(src, "fn rd{a}(i: u32, elem: u32) -> {} {{ let n = arrayLength(&S{a}) / {w}u; let j = min(i, n - 1u); if (j == elem) {{ return {live}; }} return {other}; }}", wty(ty));
    }
    let mut inputs = Vec::new();
    let first_input = u32::try_from(2 + 2 * prog.states.len()).unwrap_or(u32::MAX);
    if g.bones {
        let _ = writeln!(src, "@group(0) @binding({first_input}) var<storage, read> Bn: array<f32>;");
        src.push_str(&bone_helper());
        inputs.push(GpuBinding { binding: first_input, input: 0, bone: true });
    }
    for &m in &g.meshes {
        let binding = first_input + 1 + u32::try_from(m).unwrap_or(0);
        let _ = writeln!(src, "@group(0) @binding({binding}) var<storage, read> M{m}: array<u32>;");
        src.push_str(&mesh_helper(m));
        inputs.push(GpuBinding { binding, input: m, bone: false });
    }
    let count = k.over.map_or_else(|| "1u".to_owned(), |a| {
        let w = prog.states[a].ty.width();
        // the threads follow the array's length; its binding is declared above when the kernel touches it, so name it explicitly otherwise
        format!("arrayLength(&S{a}) / {w}u")
    });
    if let Some(a) = k.over
        && !g.used.contains_key(&a)
    {
        let _ = writeln!(src, "@group(0) @binding({}) var<storage, read> S{a}: array<f32>;", 2 + 2 * a);
        arrays.push((a, false, false));
    }
    let _ = writeln!(src, "@compute @workgroup_size(64)\nfn main(@builtin(global_invocation_id) gid: vec3<u32>) {{");
    if k.over.is_some() {
        let _ = writeln!(src, "  let elem = gid.x;\n  if (elem >= {count}) {{ return; }}");
        let _ = writeln!(src, "  var l0: f32 = f32(elem);");
    } else {
        let _ = writeln!(src, "  if (gid.x != 0u) {{ return; }}\n  let elem = 0xffffffffu;");
    }
    src.push_str(&body);
    src.push_str("}\n");
    Ok(GpuKernel { name: k.name.clone(), wgsl: src, over: k.over, arrays, inputs })
}

fn steps(list: &[Step]) -> Vec<GpuStep> {
    list.iter()
        .map(|s| match s {
            Step::Run(k) => GpuStep::Run(*k),
            Step::Repeat(n, inner) => GpuStep::Repeat(*n, steps(inner)),
        })
        .collect()
}

/// Generates the GPU plan of a program.
///
/// # Errors
/// A message naming what the GPU executor cannot do yet (mesh inputs, bones, `adjacent`).
pub(super) fn generate(prog: &Program) -> Result<GpuPlan, String> {
    let mut offsets = Vec::new();
    let mut params = Vec::new();
    for p in &prog.params {
        offsets.push((params.len(), p.ty.width()));
        params.extend_from_slice(&p.default.v[..p.ty.width()]);
    }
    let defaults: Vec<super::Value> = prog.params.iter().map(|p| p.default).collect();
    let states = prog
        .states
        .iter()
        .map(|s| GpuState {
            name: s.name.clone(),
            width: s.ty.width(),
            count: s.count.resolve(&defaults, &[]),
            mesh: if let Count::Mesh(m) = s.count { Some(m) } else { None },
        })
        .collect();
    let inputs = prog.inputs.iter().map(|i| GpuInput { name: i.name.clone(), bone: i.kind == InputKind::Bone }).collect();
    let kernels: Result<Vec<GpuKernel>, String> = prog.kernels.iter().map(|k| kernel(prog, k, &offsets)).collect();
    Ok(GpuPlan { states, inputs, params, kernels: kernels?, schedule: steps(&prog.schedule), param_offsets: offsets, init: prog.init })
}
