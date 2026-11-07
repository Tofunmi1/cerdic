//! Live liquids (`fluid.sim`): a part whose mesh is the surface of a block of particles that the position-based fluid solver steps every frame.
//!
//! The part is uploaded like any other, in chunks of triangles with their corner offsets kept. Its chunks share one solver. Each frame the capsules
//! of the collider bones follow their bones, the solver steps, the surface is meshed from the particles, and every chunk is given its slice of the
//! new surface's triangles; the part was padded at evaluation so the chunks have room for the surface to grow, and triangles beyond the surface
//! collapse to a point inside the liquid.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    system::{Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_time::{Real, Time};
use ve_modeling::fluid_sim::FluidSpec;
use ve_world::cloth::Collider;
use ve_world::fluid::{Apic, Fluid, Liquid, Settings, Surface, surface};

use crate::animation::PartPoses;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// The longest frame the solver is asked to cover.
const MAX_DT: f32 = 1.0 / 30.0;

/// One liquid's solver, shared by all the chunks of its part.
pub(crate) struct Shared {
    spec: FluidSpec,
    state: Mutex<State>,
}

struct State {
    fluid: Option<Box<dyn Liquid>>,
    surface: Option<Surface>,
    steps: u64,
    failed: bool,
}

impl Shared {
    /// `None` when the description cannot make a liquid (an empty or too large block, non-finite numbers).
    pub(crate) fn new(spec: FluidSpec) -> Option<Arc<Self>> {
        let numbers = [
            spec.spacing,
            spec.viscosity,
            spec.friction,
            spec.radius,
            spec.cell,
            spec.level,
        ];
        let vectors = [
            spec.min,
            spec.max,
            spec.bounds_min,
            spec.bounds_max,
            spec.gravity,
        ];
        let valid = numbers.iter().all(|v| v.is_finite())
            && vectors.iter().flatten().all(|v| v.is_finite())
            && (1..=8).contains(&spec.substeps)
            && (1..=8).contains(&spec.iterations)
            && spec.cell > 0.0
            && spec.segments.len() == spec.collide_bones.len()
            && spec
                .segments
                .iter()
                .all(|s| s.head.iter().chain(&s.tail).all(|v| v.is_finite()))
            && spec.max_triangles >= 200
            && build(&spec).is_some();
        valid.then(|| {
            Arc::new(Self {
                spec,
                state: Mutex::new(State {
                    fluid: None,
                    surface: None,
                    steps: 0,
                    failed: false,
                }),
            })
        })
    }
}

/// The solver a description asks for, set up as described, or `None` when it cannot make a liquid.
fn build(spec: &FluidSpec) -> Option<Box<dyn Liquid>> {
    let settings = Settings {
        gravity: spec.gravity,
        substeps: spec.substeps,
        iterations: spec.iterations,
        viscosity: spec.viscosity,
        friction: spec.friction,
        bounds: Some((spec.bounds_min, spec.bounds_max)),
        ..Settings::default()
    };
    let mut liquid: Box<dyn Liquid> = if spec.apic {
        Box::new(Apic::block(
            spec.min,
            spec.max,
            spec.spacing,
            Some((spec.bounds_min, spec.bounds_max)),
        )?)
    } else {
        Box::new(Fluid::block(spec.min, spec.max, spec.spacing)?)
    };
    liquid.configure(settings);
    Some(liquid)
}

/// One uploaded chunk of a live liquid.
#[derive(Component, Clone)]
pub(crate) struct FluidLive {
    pub(crate) shared: Arc<Shared>,
    /// Which corner of the whole part this chunk starts at.
    pub(crate) corner_offset: usize,
}

thread_local! {
    /// JSON about the live liquids as of the last frame (see `published`).
    static PUBLISHED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `{ fluids: [{ particles, steps, min, max, triangles, fastest }] }` as of the last frame.
pub(crate) fn published() -> String {
    PUBLISHED.with(|list| format!("{{\"fluids\":[{}]}}", list.borrow().join(",")))
}

fn transform(matrix: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    let m = matrix;
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

fn advance(shared: &Shared, poses: &PartPoses, dt: f32) {
    let Ok(mut state) = shared.state.lock() else {
        return;
    };
    if state.failed {
        return;
    }
    let spec = &shared.spec;
    if state.fluid.is_none() {
        let Some(fluid) = build(spec) else {
            state.failed = true;
            return;
        };
        state.fluid = Some(fluid);
    }
    let State {
        fluid,
        surface: out,
        steps,
        ..
    } = &mut *state;
    let Some(fluid) = fluid.as_mut() else {
        return;
    };
    fluid.colliders_mut().clear();
    for (name, segment) in spec.collide_bones.iter().zip(&spec.segments) {
        let matrix = poses.matrix_of(name).unwrap_or(IDENTITY);
        fluid.colliders_mut().push(Collider::Capsule {
            a: transform(&matrix, segment.head),
            b: transform(&matrix, segment.tail),
            radius: spec.radius,
        });
    }
    if !fluid.step(dt) {
        state.fluid = None; // start again from the block rather than draw garbage
        return;
    }
    *out = surface(fluid.pos(), spec.spacing, spec.cell, spec.level);
    *steps += 1;
    let (mut lo, mut hi, mut fastest) = ([f32::MAX; 3], [f32::MIN; 3], 0.0_f32);
    for (p, v) in fluid.pos().iter().zip(fluid.vel()) {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
        fastest = fastest.max((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
    }
    let triangles = out.as_ref().map_or(0, |s| s.triangles.len());
    let text = format!(
        "{{\"particles\":{},\"steps\":{},\"min\":{lo:?},\"max\":{hi:?},\"triangles\":{triangles},\"fastest\":{fastest:?}}}",
        fluid.pos().len(),
        *steps
    );
    PUBLISHED.with(|list| list.borrow_mut().push(text));
}

/// Steps every live liquid once and gives each chunk its slice of the new surface.
pub(crate) fn simulate(
    poses: Res<PartPoses>,
    time: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
    parts: Query<(&FluidLive, &Mesh3d)>,
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
        let (Some(fluid), Some(surf)) = (state.fluid.as_ref(), state.surface.as_ref()) else {
            continue;
        };
        let Some(mut mesh) = meshes.get_mut(&handle.0) else {
            continue;
        };
        let count = mesh.count_vertices();
        let spec = &live.shared.spec;
        let limit = spec.max_triangles as usize;
        // Where a triangle beyond the surface collapses to: the middle of the particles.
        let centre = fluid.pos().first().copied().unwrap_or([0.0; 3]);
        let (mut positions, mut normals) = (Vec::with_capacity(count), Vec::with_capacity(count));
        for corner in 0..count {
            let triangle = (live.corner_offset + corner) / 3;
            let which = (live.corner_offset + corner) % 3;
            match surf.triangles.get(triangle).filter(|_| triangle < limit) {
                Some(t) => {
                    let v = t[which] as usize;
                    positions.push(surf.positions[v]);
                    normals.push(surf.normals[v]);
                }
                None => {
                    positions.push(centre);
                    normals.push([0.0, 1.0, 0.0]);
                }
            }
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        if mesh.contains_attribute(ve_render::ATTRIBUTE_OCCLUSION.id) {
            mesh.insert_attribute(ve_render::ATTRIBUTE_OCCLUSION, vec![1.0_f32; count]);
        }
    }
}
