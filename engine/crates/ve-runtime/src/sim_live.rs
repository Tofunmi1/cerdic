//! Live simulation programs (`sim.program`): a part drawn by a SimL program, either as one small tetrahedron per particle or as the
//! vertices of a mesh the program drives (cloth, hair, ropes).
//!
//! The program is stepped at its own fixed rate (however fast frames come) with the bone poses it reads, then every chunk of the part is
//! given its slice of the result. Each instance (the world's, or one copy of a character) owns its own `Sim`, so copies never touch each
//! other, and a copy reads the bones of its own scope through the (program name, part name) pairs.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    query::Without,
    system::{Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_time::{Real, Time};
use ve_modeling::sim_program::{MAX_MESH_POINTS, MAX_PARTICLES, SimSpec, TETRA_CORNERS, TETRA_FACES};
use ve_world::sim::{IDENTITY, MeshData, Program, Sim, compile};

use crate::animation::PartPoses;

/// The longest frame the simulation is asked to cover; more is dropped rather than caught up.
const MAX_FRAME: f32 = 0.1;
/// Most steps one frame may run.
const MAX_STEPS_PER_FRAME: u32 = 8;

/// The mesh a program drives: its rest points and which triangle corner is which point.
struct Bound {
    data: Arc<MeshData>,
    corners: Vec<u32>,
}

/// One simulation instance, shared by all the chunks of its part.
pub(crate) struct Shared {
    spec: SimSpec,
    program: Arc<Program>,
    mesh: Option<Bound>,
    /// (the program's `input bone` name, the part whose pose it reads).
    bones: Vec<(String, String)>,
    /// (the state array whose summed vec3s are a change of speed, the part whose physics body gets it), from `output push`.
    pushes: Vec<(String, String)>,
    state: Mutex<State>,
}

struct State {
    sim: Option<Sim>,
    carry: f32,
    points: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    failed: Option<String>,
}

fn fresh() -> Mutex<State> {
    Mutex::new(State { sim: None, carry: 0.0, points: Vec::new(), normals: Vec::new(), failed: None })
}

impl Shared {
    /// `None` when the description cannot run (a program that does not compile, bad numbers, too many points, a mesh program with no mesh).
    /// `mesh` is the part's points and the vertex of each triangle corner, for a program that drives a mesh.
    pub(crate) fn new(spec: SimSpec, mesh: Option<(Vec<[f32; 3]>, Vec<u32>)>) -> Option<Arc<Self>> {
        let is_mesh = spec.mode == "mesh";
        let valid = spec.rate.is_finite()
            && (1.0..=240.0).contains(&spec.rate)
            && spec.size.is_finite()
            && spec.size > 0.0
            && (is_mesh || spec.count as usize <= MAX_PARTICLES)
            && matches!(spec.mode.as_str(), "mesh" | "particles");
        let program = valid.then(|| compile(&spec.source).ok()).flatten()?;
        let bound = match (is_mesh, mesh) {
            (true, Some((points, corners))) => {
                let n = points.len();
                if n > MAX_MESH_POINTS || corners.len() % 3 != 0 || corners.iter().any(|&c| c as usize >= n) || spec.count as usize != n {
                    return None;
                }
                let edges: Vec<(u32, u32)> = corners.chunks_exact(3).flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]).collect();
                Some(Bound { data: MeshData::new(points, &edges), corners })
            }
            (true, None) => return None,
            (false, _) => None,
        };
        let bones = spec.bones.iter().map(|b| (b.clone(), b.clone())).collect();
        let pushes = program.pushes().into_iter().map(|(body, state)| (state.to_owned(), body.to_owned())).collect();
        Some(Arc::new(Self { spec, program, mesh: bound, bones, pushes, state: fresh() }))
    }

    /// A new instance of the same program with its own state, reading the bones that `rename` maps the originals to (one more copy of a
    /// character).
    pub(crate) fn copy(&self, rename: &dyn Fn(&str) -> String) -> Arc<Self> {
        Arc::new(Self {
            spec: self.spec.clone(),
            program: Arc::clone(&self.program),
            mesh: self.mesh.as_ref().map(|m| Bound { data: Arc::clone(&m.data), corners: m.corners.clone() }),
            bones: self.bones.iter().map(|(program, part)| (program.clone(), rename(part))).collect(),
            pushes: self.pushes.iter().map(|(state, part)| (state.clone(), rename(part))).collect(),
            state: fresh(),
        })
    }

    /// The parts whose poses this program reads; they must be posed by the animation pass even with no track of their own.
    pub(crate) fn bone_parts(&self) -> impl Iterator<Item = &str> {
        self.bones.iter().map(|(_, part)| part.as_str())
    }
}

/// One uploaded chunk of a live simulation.
#[derive(Component, Clone)]
pub(crate) struct SimLive {
    pub(crate) shared: Arc<Shared>,
    /// Which corner of the whole part this chunk starts at.
    pub(crate) corner_offset: usize,
}

thread_local! {
    /// JSON about the live simulations as of the last frame (see `published`).
    static PUBLISHED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `{ sims: [{ mode, particles, steps, min, max, error }] }` as of the last frame.
pub(crate) fn published() -> String {
    PUBLISHED.with(|list| format!("{{\"sims\":[{}]}}", list.borrow().join(",")))
}

/// Smooth vertex normals from the triangles (`corners` are three vertex numbers per triangle).
fn vertex_normals(points: &[[f32; 3]], corners: &[u32]) -> Vec<[f32; 3]> {
    let mut sums = vec![[0.0_f32; 3]; points.len()];
    for t in corners.chunks_exact(3) {
        let (a, b, c) = (points[t[0] as usize], points[t[1] as usize], points[t[2] as usize]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        for &i in t {
            for k in 0..3 {
                sums[i as usize][k] += n[k];
            }
        }
    }
    for n in &mut sums {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        *n = if len > 1.0e-12 { [n[0] / len, n[1] / len, n[2] / len] } else { [0.0, 1.0, 0.0] };
    }
    sums
}

/// Makes the `Sim` of an instance on first use and feeds it the poses of the bones it reads; false when it cannot run.
fn prepare(shared: &Shared, state: &mut State, poses: &PartPoses) -> bool {
    if state.failed.is_some() {
        return false;
    }
    if state.sim.is_none() {
        let made = match &shared.mesh {
            Some(bound) => Sim::with_meshes(Arc::clone(&shared.program), shared.spec.seed, vec![Arc::clone(&bound.data)]),
            None => Sim::new(Arc::clone(&shared.program), shared.spec.seed),
        };
        let made = made.and_then(|mut sim| {
            for (name, numbers) in &shared.spec.values {
                sim.set_param(name, numbers)?;
            }
            sim.reset()?;
            Ok(sim)
        });
        match made {
            Ok(sim) => state.sim = Some(sim),
            Err(error) => {
                state.failed = Some(error.to_string());
                return false;
            }
        }
    }
    if let Some(sim) = state.sim.as_mut() {
        for (program_name, part) in &shared.bones {
            // A part a physics body moves (its pose is the truth: the animation pass lists the part with an identity pose), else a part
            // posed by an animation, else a part that stands where it was built.
            let pose = crate::physics::body_matrix(part).or_else(|| poses.matrix_of(part)).unwrap_or(IDENTITY);
            sim.set_bone(program_name, pose);
        }
    }
    true
}

/// Runs `steps` steps of 1/rate seconds; a failure stops the instance and is reported by `published`.
fn run_steps(shared: &Shared, state: &mut State, steps: u32) {
    let step = 1.0 / shared.spec.rate;
    for _ in 0..steps {
        let Some(sim) = state.sim.as_mut() else {
            return;
        };
        if let Err(error) = sim.step(step) {
            state.failed = Some(error.to_string());
            return;
        }
        // What the program asks of physics bodies: the sum of each push array is a change of speed for the body of the named part.
        for (array, part) in &shared.pushes {
            if let Some(numbers) = sim.state(array) {
                let sum = numbers.chunks_exact(3).fold([0.0_f32; 3], |s, c| [s[0] + c[0], s[1] + c[1], s[2] + c[2]]);
                crate::physics::queue_push(part, sum);
            }
        }
    }
}

/// Advances an instance by `steps` steps now, whatever the frame clock says (for tools and tests); returns the steps taken so far.
pub(crate) fn step_now(shared: &Shared, poses: &PartPoses, steps: u32) -> Result<u64, String> {
    let mut state = shared.state.lock().map_err(|_| "the simulation is locked".to_owned())?;
    if prepare(shared, &mut state, poses) {
        run_steps(shared, &mut state, steps.min(10_000));
        refresh(shared, &mut state);
    }
    state.failed.clone().map_or_else(|| Ok(state.sim.as_ref().map_or(0, Sim::steps)), Err)
}

/// Starts the instance again from its initial state (the next frame builds it afresh).
pub(crate) fn reset(shared: &Shared) {
    if let Ok(mut state) = shared.state.lock() {
        state.sim = None;
        state.carry = 0.0;
        state.failed = None;
    }
}

/// Sets one parameter of the running instance.
pub(crate) fn set_param(shared: &Shared, name: &str, values: &[f32]) -> Result<(), String> {
    let mut state = shared.state.lock().map_err(|_| "the simulation is locked".to_owned())?;
    match state.sim.as_mut() {
        Some(sim) => sim.set_param(name, values).map_err(|e| e.message),
        None => Err("the simulation has not started yet; step it once first".to_owned()),
    }
}

/// What a tool wants to know about an instance.
pub(crate) fn describe(shared: &Shared) -> serde_json::Value {
    let Ok(state) = shared.state.lock() else {
        return serde_json::json!({ "error": "locked" });
    };
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &state.points {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    serde_json::json!({
        "mode": shared.spec.mode,
        "rate": shared.spec.rate,
        "elements": state.points.len(),
        "steps": state.sim.as_ref().map_or(0, Sim::steps),
        "params": shared.program.param_names(),
        "states": shared.program.state_names(),
        "bones": shared.bones,
        "min": lo, "max": hi,
        "fingerprint": state.sim.as_ref().map(|s| format!("{:016x}", s.fingerprint())),
        "error": state.failed,
    })
}

fn advance(shared: &Shared, poses: &PartPoses, dt: f32) {
    let Ok(mut state) = shared.state.lock() else {
        return;
    };
    if !prepare(shared, &mut state, poses) {
        return;
    }
    let step = 1.0 / shared.spec.rate;
    state.carry += dt.min(MAX_FRAME);
    let mut ran = 0;
    while state.carry >= step && ran < MAX_STEPS_PER_FRAME {
        state.carry -= step;
        ran += 1;
    }
    run_steps(shared, &mut state, ran);
    if ran == MAX_STEPS_PER_FRAME {
        state.carry = 0.0;
    }
    refresh(shared, &mut state);
}

/// Copies the drawn state into points (and normals) and publishes the instance's numbers.
fn refresh(shared: &Shared, state: &mut State) {
    let State { sim, points, normals, .. } = &mut *state;
    if let Some(sim) = sim.as_ref()
        && let Some(numbers) = sim.state(&shared.spec.particles)
    {
        points.clear();
        points.extend(numbers.chunks_exact(3).map(|c| [c[0], c[1], c[2]]));
    }
    if let Some(bound) = &shared.mesh {
        *normals = vertex_normals(points, &bound.corners);
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in points.iter() {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let steps = state.sim.as_ref().map_or(0, Sim::steps);
    let error = state.failed.as_ref().map_or_else(|| "null".to_owned(), |e| format!("{e:?}"));
    let text = format!("{{\"mode\":\"{}\",\"particles\":{},\"steps\":{steps},\"min\":{lo:?},\"max\":{hi:?},\"error\":{error}}}", shared.spec.mode, state.points.len());
    PUBLISHED.with(|list| list.borrow_mut().push(text));
}

/// Steps every live simulation and gives each chunk its slice of the result.
pub(crate) fn simulate(
    poses: Res<PartPoses>,
    time: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
    parts: Query<(&SimLive, &Mesh3d), Without<crate::game_models::ModelPrototype>>,
) {
    let dt = time.delta_secs().max(0.0);
    PUBLISHED.with(|list| list.borrow_mut().clear());
    let mut stepped: HashSet<*const Shared> = HashSet::new();
    for (live, handle) in &parts {
        if stepped.insert(Arc::as_ptr(&live.shared)) {
            advance(&live.shared, &poses, dt);
        }
        let Ok(state) = live.shared.state.lock() else {
            continue;
        };
        if state.points.is_empty() {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&handle.0) else {
            continue;
        };
        let count = mesh.count_vertices();
        let mut positions = Vec::with_capacity(count);
        if let Some(bound) = &live.shared.mesh {
            let mut normals = Vec::with_capacity(count);
            for corner in 0..count {
                let Some(&vertex) = bound.corners.get(live.corner_offset + corner) else {
                    break;
                };
                positions.push(state.points[vertex as usize]);
                normals.push(state.normals.get(vertex as usize).copied().unwrap_or([0.0, 1.0, 0.0]));
            }
            if positions.len() == count {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            }
        } else {
            let size = live.shared.spec.size;
            for corner in 0..count {
                let whole = live.corner_offset + corner;
                let (particle, local) = (whole / 12, whole % 12);
                let c = state.points.get(particle).copied().unwrap_or([0.0; 3]);
                let o = TETRA_CORNERS[TETRA_FACES[local / 3][local % 3]];
                positions.push([c[0] + o[0] * size, c[1] + o[1] * size, c[2] + o[2] * size]);
            }
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}
