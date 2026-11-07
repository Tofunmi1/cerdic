//! Live cloth (`cloth.sim`): a garment part whose points are stepped by the XPBD solver every frame.
//!
//! The part is uploaded like any other, in triangle-corner chunks, with its rest data kept. Its chunks share one solver; each frame the
//! pinned points are moved to where the bone they follow has carried them, the capsules of the collider bones follow their bones, the solver
//! steps, and every chunk's mesh gets the new corner positions and smooth normals. Nothing here is random, so the same scene and the same
//! frame times give the same cloth.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    system::{Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_time::{Real, Time};
use ve_modeling::cloth_sim::ClothSpec;
use ve_world::cloth::{Collider, Solver, cloth_from_mesh};

use crate::animation::PartPoses;

/// Most vertices a live cloth carries (the evaluation refuses more; this guards the staging call).
pub(crate) const MAX_POINTS: usize = ve_modeling::cloth_sim::MAX_LIVE_POINTS;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// How far the pins may move in one frame before it counts as a teleport, in metres.
const TELEPORT: f32 = 0.3;

/// The longest frame the solver is asked to cover; a stalled tab resumes gently instead of with one huge step.
const MAX_DT: f32 = 1.0 / 30.0;

/// What a cloth part needs from the evaluation.
#[derive(Debug)]
pub(crate) struct ClothData {
    pub(crate) spec: ClothSpec,
    /// Rest position of each vertex, in world metres.
    pub(crate) rest: Vec<[f32; 3]>,
    /// For each triangle corner of the part, the vertex it comes from.
    pub(crate) corners: Vec<u32>,
}

impl ClothData {
    /// Whether the arrays agree with each other and with the spec.
    pub(crate) fn valid(&self) -> bool {
        let spec = &self.spec;
        let numbers = [
            spec.radius,
            spec.density,
            spec.stretch,
            spec.bend,
            spec.thickness,
            spec.friction,
            spec.damping,
            spec.drag,
        ];
        !self.rest.is_empty()
            && self.rest.len() <= MAX_POINTS
            && self.corners.len().is_multiple_of(3)
            && !self.corners.is_empty()
            && self.corners.len() <= 3 * MAX_POINTS * 2
            && self.corners.iter().all(|&v| (v as usize) < self.rest.len())
            && spec.pins.iter().all(|&p| (p as usize) < self.rest.len())
            && spec.segments.len() == spec.collide_bones.len()
            && spec.substeps >= 1
            && spec.substeps <= 32
            && spec.density > 0.0
            && numbers
                .iter()
                .chain(spec.gravity.iter())
                .chain(spec.wind.iter())
                .all(|v| v.is_finite())
            && self.rest.iter().flatten().all(|v| v.is_finite())
            && spec
                .segments
                .iter()
                .all(|s| s.head.iter().chain(&s.tail).all(|v| v.is_finite()))
    }
}

/// One cloth's solver, shared by all the chunks of its part.
pub(crate) struct Shared {
    data: Arc<ClothData>,
    state: Mutex<State>,
}

struct State {
    solver: Option<Solver>,
    /// Smooth normal of each vertex after the last step.
    normals: Vec<[f32; 3]>,
    /// Whether the solver could not be built (so it is not tried every frame).
    failed: bool,
    /// How many times the solver has been stepped (one per frame).
    steps: u64,
}

impl Shared {
    /// The cloth's description (its spec, rest points and corners).
    pub(crate) fn data(&self) -> &ClothData {
        &self.data
    }

    pub(crate) fn new(data: Arc<ClothData>) -> Arc<Self> {
        Arc::new(Self {
            data,
            state: Mutex::new(State {
                solver: None,
                normals: Vec::new(),
                failed: false,
                steps: 0,
            }),
        })
    }
}

/// One uploaded chunk of a live cloth.
#[derive(Component, Clone)]
pub(crate) struct ClothLive {
    pub(crate) shared: Arc<Shared>,
    /// Which corner of the whole part this chunk starts at.
    pub(crate) corner_offset: usize,
}

fn transform(matrix: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    let m = matrix;
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

fn build(data: &ClothData) -> Option<Solver> {
    let spec = &data.spec;
    let triangles: Vec<[u32; 3]> = data
        .corners
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let mut solver = cloth_from_mesh(
        &data.rest,
        &triangles,
        spec.density,
        spec.stretch,
        spec.bend,
    )?;
    solver.set_triangles(triangles);
    solver.settings.gravity = spec.gravity;
    solver.settings.substeps = spec.substeps;
    solver.settings.damping = spec.damping;
    solver.settings.thickness = spec.thickness;
    solver.settings.friction = spec.friction;
    solver.settings.wind = spec.wind;
    solver.settings.drag = spec.drag;
    for &pin in &spec.pins {
        solver.pin(pin as usize);
    }
    Some(solver)
}

fn vertex_normals(solver: &Solver, corners: &[u32], into: &mut Vec<[f32; 3]>) {
    into.clear();
    into.resize(solver.pos.len(), [0.0; 3]);
    for tri in corners.chunks_exact(3) {
        let (a, b, c) = (
            solver.pos[tri[0] as usize],
            solver.pos[tri[1] as usize],
            solver.pos[tri[2] as usize],
        );
        let (u, v) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &i in tri {
            for axis in 0..3 {
                into[i as usize][axis] += n[axis];
            }
        }
    }
    for n in into.iter_mut() {
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        *n = if length > 1.0e-9 {
            [n[0] / length, n[1] / length, n[2] / length]
        } else {
            [0.0, 1.0, 0.0]
        };
    }
}

thread_local! {
    /// JSON about the live cloths as of the last frame, for tests and the console (see `published`).
    static PUBLISHED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `{ cloths: [{ points, steps, min, max, pin_matrix }] }` as of the last frame.
pub(crate) fn published() -> String {
    PUBLISHED.with(|list| format!("{{\"cloths\":[{}]}}", list.borrow().join(",")))
}

fn summary(solver: &Solver, steps: u64, pin_matrix: &[f32; 16]) -> String {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &solver.pos {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    format!(
        "{{\"points\":{},\"steps\":{steps},\"min\":{lo:?},\"max\":{hi:?},\"pin_matrix\":{pin_matrix:?}}}",
        solver.pos.len()
    )
}

/// Steps one cloth by `dt` with the bones where `poses` put them.
fn advance(shared: &Shared, poses: &PartPoses, dt: f32) {
    let Ok(mut state) = shared.state.lock() else {
        return;
    };
    if state.failed {
        return;
    }
    let data = &*shared.data;
    if state.solver.is_none() {
        match build(data) {
            Some(solver) => state.solver = Some(solver),
            None => {
                state.failed = true;
                return;
            }
        }
    }
    let spec = &data.spec;
    let pin_matrix = if spec.pin_bone.is_empty() {
        IDENTITY
    } else {
        poses.matrix_of(&spec.pin_bone).unwrap_or(IDENTITY)
    };
    let State {
        solver,
        normals,
        steps,
        ..
    } = &mut *state;
    let Some(solver) = solver.as_mut() else {
        return;
    };
    // A pin that jumps farther than a person could move it in one frame (a clip restarting, a respawn) is a teleport: the whole cloth goes with it
    // rather than being whipped by constraints that cannot follow, which leaves it folded over its own pins.
    if !spec.pins.is_empty() {
        let mut delta = [0.0_f32; 3];
        for &pin in &spec.pins {
            let to = transform(&pin_matrix, data.rest[pin as usize]);
            for axis in 0..3 {
                delta[axis] += to[axis] - solver.pos[pin as usize][axis];
            }
        }
        let count = spec.pins.len() as f32;
        delta = delta.map(|v| v / count);
        if (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt() > TELEPORT {
            for i in 0..solver.pos.len() {
                let p = solver.pos[i];
                solver.move_pin(i, [p[0] + delta[0], p[1] + delta[1], p[2] + delta[2]]);
                solver.vel[i] = [0.0; 3];
            }
        }
    }
    for &pin in &spec.pins {
        let i = pin as usize;
        solver.move_pin(i, transform(&pin_matrix, data.rest[i]));
    }
    solver.colliders.clear();
    for (name, segment) in spec.collide_bones.iter().zip(&spec.segments) {
        let matrix = poses.matrix_of(name).unwrap_or(IDENTITY);
        solver.colliders.push(Collider::Capsule {
            a: transform(&matrix, segment.head),
            b: transform(&matrix, segment.tail),
            radius: spec.radius,
        });
    }
    if let Some(height) = spec.floor {
        solver.colliders.push(Collider::Plane {
            point: [0.0, height, 0.0],
            normal: [0.0, 1.0, 0.0],
        });
    }
    if !solver.step(dt) {
        // The solver refused (non-finite state): start again from rest next frame rather than draw garbage.
        state.solver = None;
        return;
    }
    vertex_normals(solver, &data.corners, normals);
    *steps += 1;
    let text = summary(solver, *steps, &pin_matrix);
    PUBLISHED.with(|list| list.borrow_mut().push(text));
}

/// Steps every live cloth once and writes the new points into its chunks' meshes.
pub(crate) fn simulate(
    poses: Res<PartPoses>,
    time: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
    parts: Query<(&ClothLive, &Mesh3d)>,
) {
    let dt = time.delta_secs().clamp(0.0, MAX_DT);
    if dt <= 0.0 {
        return;
    }
    PUBLISHED.with(|list| list.borrow_mut().clear());
    let mut stepped: HashSet<*const Shared> = HashSet::new();
    for (live, handle) in &parts {
        if stepped.insert(Arc::as_ptr(&live.shared)) {
            advance(&live.shared, &poses, dt);
        }
        let Ok(state) = live.shared.state.lock() else {
            continue;
        };
        let Some(solver) = state.solver.as_ref() else {
            continue;
        };
        let Some(mut mesh) = meshes.get_mut(&handle.0) else {
            continue;
        };
        let count = mesh.count_vertices();
        let corners = &live.shared.data.corners;
        let (mut positions, mut normals) = (Vec::with_capacity(count), Vec::with_capacity(count));
        for corner in 0..count {
            let Some(&vertex) = corners.get(live.corner_offset + corner) else {
                break;
            };
            positions.push(solver.pos[vertex as usize]);
            normals.push(
                state
                    .normals
                    .get(vertex as usize)
                    .copied()
                    .unwrap_or([0.0, 1.0, 0.0]),
            );
        }
        if positions.len() == count {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        }
    }
}
