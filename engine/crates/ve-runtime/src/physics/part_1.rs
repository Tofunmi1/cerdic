use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use bevy_ecs::{
    component::Component,
    entity::Entity,
    resource::Resource,
    system::{Query, ResMut},
};
use bevy_math::{Quat, Vec3};
use bevy_transform::components::Transform;
use ve_modeling::physics::{
    BodyKind, BodyShape, JointKind as GraphJoint, PhysBody, PhysJoint, WorldSettings,
};
use ve_physics::{BodyDesc, BodyId, JointDesc, JointKind, Kind, STEP, Shape, WalkerDesc, World};

use crate::{behavior::Behaviors, viewer::ViewerRig};

/// Points kept for a convex shape.
const HULL_POINTS: usize = 256;
/// Steps of motion a throw is measured over.
const THROW_WINDOW: usize = 6;
/// Per-second position snapshots kept for comparison.
const MAX_MARKS: usize = 12;
/// Earth's gravity, when the scene says nothing.
const EARTH: [f64; 3] = [0.0, -9.81, 0.0];
/// How far below the top of the walker's capsule the eye sits, in metres.
const EYE_DROP: f32 = 0.1;

/// What the physics of one authored part needs, shared by every chunk entity of the part.
#[derive(Debug)]
pub(crate) struct BodySpec {
    pub(crate) node: String,
    /// The part's output name, which other parts' joints refer to.
    name: String,
    /// Joints from this body to another body or to the world.
    joints: Vec<PhysJoint>,
    /// Scene-wide settings this body carries (gravity, the walking viewer).
    pub(crate) world: Option<WorldSettings>,
    kind: BodyKind,
    shape: Shape,
    /// Where the shape's centre is at rest.
    centre: Vec3,
    density: f32,
    friction: f32,
    restitution: f32,
    linear_damping: f32,
    angular_damping: f32,
}

impl BodySpec {
    /// Whether the body can move (anything but a static one).
    pub(crate) fn is_movable(&self) -> bool {
        self.kind != BodyKind::Static
    }
}

/// Marks a chunk entity as part of a physics body.
#[derive(Component, Clone)]
pub(crate) struct Body(pub(crate) Arc<BodySpec>);

/// At most [`HULL_POINTS`] points that keep the extremes: the farthest point along 26 fixed directions, then an even
/// stride through the rest. Deterministic.
fn hull_points(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if points.len() <= HULL_POINTS {
        return points.to_vec();
    }
    let mut chosen: Vec<usize> = Vec::with_capacity(HULL_POINTS);
    for x in -1..=1_i8 {
        for y in -1..=1_i8 {
            for z in -1..=1_i8 {
                if (x, y, z) == (0, 0, 0) {
                    continue;
                }
                let d = [f32::from(x), f32::from(y), f32::from(z)];
                let best = points
                    .iter()
                    .enumerate()
                    .max_by(|a, b| {
                        let score = |p: &[f32; 3]| p[0] * d[0] + p[1] * d[1] + p[2] * d[2];
                        score(a.1).total_cmp(&score(b.1)).then(b.0.cmp(&a.0))
                    })
                    .map(|(i, _)| i);
                if let Some(best) = best
                    && !chosen.contains(&best)
                {
                    chosen.push(best);
                }
            }
        }
    }
    let stride = points.len().div_ceil(HULL_POINTS - chosen.len());
    let room = HULL_POINTS - chosen.len();
    let evenly: Vec<usize> = (0..points.len())
        .step_by(stride.max(1))
        .filter(|i| !chosen.contains(i))
        .take(room)
        .collect();
    chosen.extend(evenly);
    chosen.into_iter().map(|i| points[i]).collect()
}

/// Builds the body spec for a part whose vertices are `points` (flat xyz) with `triangles` (flat index triples; both
/// empty when the shape fits the bounds) and bounding box `(min, max)`. A shape the world cannot build is refused later,
/// when the world is made.
pub(crate) fn build_spec(
    node: &str,
    name: &str,
    body: &PhysBody,
    points: &[f32],
    triangles: &[u32],
    (min, max): ([f32; 3], [f32; 3]),
) -> BodySpec {
    let centre = Vec3::from_array(std::array::from_fn(|a| (min[a] + max[a]) * 0.5));
    let half: [f32; 3] = std::array::from_fn(|a| ((max[a] - min[a]) * 0.5).max(1.0e-3));
    let around: Vec<[f32; 3]> = points
        .chunks_exact(3)
        .map(|p| [p[0] - centre.x, p[1] - centre.y, p[2] - centre.z])
        .collect();
    let radius = half[0].max(half[2]);
    let shape = match body.shape {
        BodyShape::Box => Shape::Box { half },
        BodyShape::Sphere => Shape::Sphere {
            radius: half[0].max(half[1]).max(half[2]),
        },
        BodyShape::Capsule => Shape::Capsule {
            half_height: (half[1] - radius).max(0.0),
            radius,
        },
        BodyShape::Cylinder => Shape::Cylinder {
            half_height: half[1],
            radius,
        },
        BodyShape::Auto | BodyShape::Convex => {
            if around.len() >= 4 {
                Shape::Convex {
                    points: hull_points(&around),
                }
            } else {
                Shape::Box { half }
            }
        }
        BodyShape::Mesh => Shape::Mesh {
            points: around,
            triangles: triangles
                .chunks_exact(3)
                .map(|t| [t[0], t[1], t[2]])
                .collect(),
        },
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "authored physical constants are small"
    )]
    let f = |v: f64| v as f32;
    BodySpec {
        node: node.to_owned(),
        name: name.to_owned(),
        joints: body.joints.clone(),
        world: body.world.clone(),
        kind: body.kind,
        shape,
        centre,
        density: f(body.density),
        friction: f(body.friction),
        restitution: f(body.restitution),
        linear_damping: f(body.linear_damping),
        angular_damping: f(body.angular_damping),
    }
}

/// A body in the running world.
struct Live {
    node: String,
    id: BodyId,
    kind: BodyKind,
    centre: Vec3,
    held: bool,
    history: VecDeque<Vec3>,
    entities: Vec<Entity>,
    spec: Arc<BodySpec>,
}

/// The running world, present only in play mode.
#[derive(Resource, Default)]
pub(crate) struct Physics {
    world: Option<World>,
    bodies: Vec<Live>,
    /// Steps taken since play mode started.
    steps: u64,
    /// Entities carrying a body in the scene, and how many of them can move (play mode has something to show).
    entities: usize,
    movable: usize,
    /// Why bodies were left out of the world (a shape that could not be built).
    refused: Vec<String>,
    /// Positions of every body at steps 60, 120, ... (the first [`MAX_MARKS`]), in body order.
    marks: Vec<(u64, Vec<[f32; 3]>)>,
    /// Pairs of bodies touching after the last step, to tell when a touch begins.
    touching: Vec<(BodyId, BodyId)>,
    /// The walking viewer, when the scene has one and play mode is on.
    walker: Option<Walking>,
}

/// A viewer walked by physics.
struct Walking {
    height: f32,
    speed: f32,
    jump: f32,
    /// Where the capsule's centre started, to put it back if it falls out of the world.
    start: [f32; 3],
    /// The last state the world reported.
    state: Option<ve_physics::WalkerState>,
}

/// The settings that apply to the whole scene: those of the one body that carries them, or Earth's gravity.
fn scene_settings(parts: &Query<(Entity, &Body, &mut Transform)>) -> Option<WorldSettings> {
    parts.iter().find_map(|(_, body, _)| body.0.world.clone())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "authored scene numbers are small"
)]
fn narrow(v: [f64; 3]) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}

/// Puts a walker in the world where the viewer's eye is, when the scene asks for one and the desktop viewer is on.
fn start_walker(
    world: &mut World,
    settings: &WorldSettings,
    rig: &ViewerRig,
    bodies: &[Live],
) -> Option<Walking> {
    let input = rig.walk_input().filter(|_| settings.walker)?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "authored scene numbers are small"
    )]
    let (height, radius, speed, jump) = (
        settings.height as f32,
        settings.radius as f32,
        settings.speed as f32,
        settings.jump as f32,
    );
    let eye_to_centre = Vec3::Y * (height * 0.5 - EYE_DROP);
    let standing = height * 0.5 + 0.05;
    // Start under the camera if there is ground there (falling at most 3 m); else on top of the body that carries the scene
    // settings (the floor), since the camera is often far outside the scene when play begins.
    let start = if let Some(ground) = world.ground_below(input.eye.x, input.eye.z, input.eye.y) {
        let centre = input.eye - eye_to_centre;
        Vec3::new(
            centre.x,
            centre.y.clamp(ground + standing, ground + standing + 3.0),
            centre.z,
        )
    } else {
        let carrier = bodies.iter().find(|body| body.spec.world.is_some())?;
        let middle = Vec3::from_array(world.pose(carrier.id).ok()?.position);
        let top = world
            .ground_below(middle.x, middle.z, middle.y + 500.0)
            .unwrap_or(middle.y);
        Vec3::new(middle.x, top + standing, middle.z)
    }
    .to_array();
    world
        .set_walker(&WalkerDesc {
            position: start,
            height,
            radius,
        })
        .ok()?;
    Some(Walking {
        height,
        speed,
        jump,
        start,
        state: None,
    })
}
