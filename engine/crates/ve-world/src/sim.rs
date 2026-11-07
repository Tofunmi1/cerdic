//! Simulation programs (SimL): state arrays, kernels and a schedule written as text, run by a deterministic CPU reference.
//!
//! A program declares `param`s (settable numbers), `state` arrays (one value per element) and `kernel`s that run for each element of an
//! array. Within one kernel every element reads the state as it was when the kernel started (plus its own writes to its own element), so
//! the result does not depend on the order elements run in and a GPU executor can run them in parallel. All arithmetic is `f32` with the
//! platform independent functions of the field-expression language, so a program gives the same bits on every platform.
mod ast;
mod grid;
mod inputs;
pub mod library;
mod lexer;
mod ops;
mod parser;
mod run;
mod typing;
mod wgsl;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_gpu;
#[cfg(test)]
mod tests_inputs;

use std::sync::Arc;

pub use crate::expr::Diagnostic;
pub use inputs::{IDENTITY, MeshData};
pub use wgsl::{GpuBinding, GpuInput, GpuKernel, GpuPlan, GpuState, GpuStep};

/// The most elements one state array holds.
pub const MAX_ELEMENTS: usize = 1 << 20;
/// The most bytes all state arrays of one simulation hold.
pub const MAX_STATE_BYTES: usize = 64 << 20;
/// The most source bytes a program may have.
pub const MAX_SOURCE_BYTES: usize = 64 << 10;
/// The most statements and loop turns one element may run in one kernel.
pub const MAX_WORK_PER_ELEMENT: u32 = 100_000;

/// A value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    /// One number.
    Float,
    /// Two numbers.
    Vec2,
    /// Three numbers.
    Vec3,
    /// Four numbers.
    Vec4,
    /// True or false.
    Bool,
}

impl Ty {
    /// How many numbers the type is stored as.
    #[must_use]
    pub fn width(self) -> usize {
        match self {
            Self::Float | Self::Bool => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Float => "f32",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Bool => "bool",
        }
    }
    pub(crate) fn of_width(width: usize) -> Option<Self> {
        match width {
            1 => Some(Self::Float),
            2 => Some(Self::Vec2),
            3 => Some(Self::Vec3),
            4 => Some(Self::Vec4),
            _ => None,
        }
    }
}

/// A typed value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Value {
    /// The type.
    pub ty: Ty,
    /// The components; unused ones are zero (a boolean is 0 or 1).
    pub v: [f32; 4],
}

impl Value {
    pub(crate) fn float(x: f32) -> Self {
        Self { ty: Ty::Float, v: [x, 0.0, 0.0, 0.0] }
    }
    pub(crate) fn boolean(b: bool) -> Self {
        Self { ty: Ty::Bool, v: [if b { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0] }
    }
}

/// A compiled program, shared by every simulation (every copy of a character) that runs it.
#[derive(Debug)]
pub struct Program {
    inner: ast::Program,
}

/// Compiles SimL source.
///
/// # Errors
/// Diagnostics with line, column and a suggestion.
pub fn compile(source: &str) -> Result<Arc<Program>, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic {
            line: 1,
            column: 1,
            message: format!("the program is longer than {MAX_SOURCE_BYTES} bytes"),
            suggestion: "split it into smaller systems".to_owned(),
        }]);
    }
    let tokens = lexer::lex(source)?;
    parser::parse(tokens).map(|inner| Arc::new(Program { inner }))
}

impl Program {
    /// The program name from its `sim` line.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }
    /// Names of the declared parameters.
    #[must_use]
    pub fn param_names(&self) -> Vec<&str> {
        self.inner.params.iter().map(|p| p.name.as_str()).collect()
    }
    /// Names of the declared state arrays.
    #[must_use]
    pub fn state_names(&self) -> Vec<&str> {
        self.inner.states.iter().map(|s| s.name.as_str()).collect()
    }
    /// The program as compute shaders for the GPU executor.
    ///
    /// # Errors
    /// A message naming what the GPU executor cannot run yet (mesh inputs, bones, `adjacent`); such programs run on the CPU.
    pub fn gpu_plan(&self) -> Result<GpuPlan, String> {
        wgsl::generate(&self.inner)
    }
    /// Names of the `input bone` declarations: the bone parts whose poses the host must supply.
    #[must_use]
    pub fn bone_inputs(&self) -> Vec<&str> {
        self.inner.inputs.iter().filter(|i| i.kind == ast::InputKind::Bone).map(|i| i.name.as_str()).collect()
    }
    /// Names of the `input mesh` declarations, in the order [`Sim::with_meshes`] takes them.
    #[must_use]
    pub fn mesh_inputs(&self) -> Vec<&str> {
        self.inner.inputs.iter().filter(|i| i.kind == ast::InputKind::Mesh).map(|i| i.name.as_str()).collect()
    }
    /// The `output push BODY array` declarations as (body part name, state name): each step the sum of the array's vec3s is a change of
    /// speed (metres per second) the host gives the physics body of that part.
    #[must_use]
    pub fn pushes(&self) -> Vec<(&str, &str)> {
        self.inner
            .outputs
            .iter()
            .filter_map(|o| Some((o.target.as_deref()?, self.inner.states[o.state].name.as_str())))
            .collect()
    }
    /// Declared outputs as (kind, state name).
    #[must_use]
    pub fn outputs(&self) -> Vec<(&str, &str)> {
        self.inner
            .outputs
            .iter()
            .map(|o| (o.kind.as_str(), self.inner.states[o.state].name.as_str()))
            .collect()
    }
}

/// A runtime failure with the source position of the statement that caused it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunError {
    /// One-based source line, 0 when the failure has none.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for RunError {}

/// One running simulation: the state of a program for one scope (the world, or one copy of a character).
#[derive(Debug)]
pub struct Sim {
    program: Arc<Program>,
    state: Vec<Vec<f32>>,
    params: Vec<Value>,
    inputs: inputs::Inputs,
    seed: u32,
    steps: u64,
    time: f32,
}

impl Sim {
    /// Allocates the state of `program` and runs its `init` kernel. A program with `input mesh` declarations needs [`Sim::with_meshes`].
    ///
    /// # Errors
    /// A state array over [`MAX_ELEMENTS`], all state over [`MAX_STATE_BYTES`], or a failure inside `init`.
    pub fn new(program: Arc<Program>, seed: u32) -> Result<Self, RunError> {
        Self::with_meshes(program, seed, Vec::new())
    }

    /// Like [`Sim::new`] with a mesh bound to each `input mesh`, in declaration order.
    ///
    /// # Errors
    /// As [`Sim::new`], and a wrong number of meshes.
    pub fn with_meshes(program: Arc<Program>, seed: u32, meshes: Vec<Arc<MeshData>>) -> Result<Self, RunError> {
        let wanted = program.mesh_inputs().len();
        if meshes.len() != wanted {
            return Err(RunError { line: 0, message: format!("the program has {wanted} mesh inputs and {} meshes were given", meshes.len()) });
        }
        let params = program.inner.params.iter().map(|p| p.default).collect();
        let mut bound = meshes.into_iter();
        let mut inputs = inputs::Inputs::default();
        for decl in &program.inner.inputs {
            inputs.bones.push(IDENTITY);
            inputs.meshes.push(if decl.kind == ast::InputKind::Mesh { bound.next() } else { None });
        }
        let mut sim = Self { program, state: Vec::new(), params, inputs, seed, steps: 0, time: 0.0 };
        sim.reset()?;
        Ok(sim)
    }

    /// Sets the pose (a column-major 4x4 matrix) of the bone input called `name`; false when the program has no such input.
    pub fn set_bone(&mut self, name: &str, pose: [f32; 16]) -> bool {
        let at = self.program.inner.inputs.iter().position(|i| i.name == name && i.kind == ast::InputKind::Bone);
        at.is_some_and(|i| {
            self.inputs.bones[i] = pose;
            true
        })
    }

    /// Restores the initial state: zeros, then the `init` kernel if the program has one.
    ///
    /// # Errors
    /// Over-size state, or a failure inside `init`.
    pub fn reset(&mut self) -> Result<(), RunError> {
        let mut bytes = 0_usize;
        let mut state = Vec::new();
        let mesh_points: Vec<usize> = self.inputs.meshes.iter().map(|m| m.as_ref().map_or(0, |m| m.len())).collect();
        for decl in &self.program.inner.states {
            let count = decl.count.resolve(&self.params, &mesh_points);
            if count == 0 || count > MAX_ELEMENTS {
                return Err(RunError {
                    line: decl.line,
                    message: format!("`{}` has {count} elements; allowed 1 to {MAX_ELEMENTS}", decl.name),
                });
            }
            bytes += count * decl.ty.width() * 4;
            if bytes > MAX_STATE_BYTES {
                return Err(RunError { line: decl.line, message: format!("the state is over {MAX_STATE_BYTES} bytes") });
            }
            state.push(vec![0.0; count * decl.ty.width()]);
        }
        self.state = state;
        self.steps = 0;
        self.time = 0.0;
        if let Some(init) = self.program.inner.init {
            run::kernel(&self.program.inner, &mut self.state, &self.params, &self.inputs, init, run::Clock { dt: 0.0, t: 0.0, step: 0.0, seed: self.seed })?;
        }
        Ok(())
    }

    /// Advances one step of `dt` seconds: the schedule, once.
    ///
    /// # Errors
    /// A failure inside a kernel (index out of range, too much work for one element).
    pub fn step(&mut self, dt: f32) -> Result<(), RunError> {
        #[expect(clippy::cast_precision_loss, reason = "the step number is only a number the program can read")]
        let clock = run::Clock { dt, t: self.time, step: self.steps as f32, seed: self.seed };
        run::schedule(&self.program.inner, &mut self.state, &self.params, &self.inputs, &self.program.inner.schedule, clock)?;
        self.steps += 1;
        self.time += dt;
        Ok(())
    }

    /// Advances `steps` steps of `dt` seconds.
    ///
    /// # Errors
    /// As [`Sim::step`].
    pub fn run(&mut self, steps: u32, dt: f32) -> Result<(), RunError> {
        for _ in 0..steps {
            self.step(dt)?;
        }
        Ok(())
    }

    /// Steps taken since the last reset.
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// The numbers of a state array, element after element.
    #[must_use]
    pub fn state(&self, name: &str) -> Option<&[f32]> {
        let at = self.program.inner.states.iter().position(|s| s.name == name)?;
        self.state.get(at).map(Vec::as_slice)
    }

    /// Sets a parameter to numbers (as many as its type has).
    ///
    /// # Errors
    /// An unknown name or the wrong count of numbers.
    pub fn set_param(&mut self, name: &str, values: &[f32]) -> Result<(), RunError> {
        let at = self
            .program
            .inner
            .params
            .iter()
            .position(|p| p.name == name)
            .ok_or_else(|| RunError { line: 0, message: format!("no parameter `{name}`") })?;
        let ty = self.params[at].ty;
        if values.len() != ty.width() || values.iter().any(|x| !x.is_finite()) {
            return Err(RunError { line: 0, message: format!("`{name}` is a {} and takes {} finite numbers", ty.name(), ty.width()) });
        }
        let mut v = [0.0; 4];
        v[..values.len()].copy_from_slice(values);
        self.params[at] = Value { ty, v };
        Ok(())
    }

    /// A 64-bit fingerprint of every state number's bits: equal for equal simulations on every platform.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for array in &self.state {
            for x in array {
                h ^= u64::from(x.to_bits());
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        h
    }
}
