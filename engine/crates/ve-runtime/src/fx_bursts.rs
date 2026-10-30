//! One-shot particle bursts at a position, in any scene (ADR 0067): a crate that explodes, sparks where a thrown thing hits. A game's
//! `particles` effect and the world API's `fx.burst` queue a request; a short-lived particle system is made for it at that place, stepped
//! and drawn with a small pool of billboards that is created on the first burst (and only then).
use std::cell::RefCell;

use bevy_asset::Assets;
use bevy_camera::{Camera3d, visibility::Visibility};
use bevy_color::Color;
use bevy_ecs::{
    component::Component,
    query::{With, Without},
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use bevy_math::{Mat4, Vec3};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_time::Time;
use bevy_transform::components::Transform;
use ve_particles::{Colour, ParticleSystem, Preset, Vec3 as ParticleVec3};
use ve_render::VeMaterial;

/// The size of a burst a game rule asks for (the presets are authored for a stage scene; a crate-sized explosion is about a third of that).
pub(crate) const GAME_SCALE: f32 = 0.3;
/// Billboards in the pool; the most that can be drawn at once.
const POOL: usize = 700;
/// Most bursts alive at once; a request beyond it is dropped.
const MAX_LIVE: usize = 24;
/// A burst is removed this long after it began, or as soon as its particles are gone.
const LIFETIME: f32 = 4.0;

thread_local! {
    static QUEUE: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
}

/// Particles drawn by bursts this frame, and bursts requested so far, for smoke checks.
static DRAWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static REQUESTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `(requested, drawn_now)`.
pub(crate) fn counts() -> (usize, usize) {
    (
        REQUESTED.load(std::sync::atomic::Ordering::Relaxed),
        DRAWN.load(std::sync::atomic::Ordering::Relaxed),
    )
}

struct Request {
    layers: Vec<(Preset, usize, [f32; 3])>,
    position: Vec3,
    scale: f32,
}

/// A billboard of the burst pool.
#[derive(Component)]
pub(crate) struct BurstQuad;

/// The bursts alive now and whether the pool of billboards exists yet.
#[derive(Resource, Default)]
pub(crate) struct Bursts {
    live: Vec<(Vec<ParticleSystem>, Vec3, f32)>,
    pool_made: bool,
}

/// What a name asks for: `explode` is a fireball with sparks and smoke; the others are one preset each.
fn layers(name: &str) -> Option<Vec<(Preset, usize, [f32; 3])>> {
    let one = |preset, count, tint| Some(vec![(preset, count, tint)]);
    match name {
        "" | "explode" | "explosion" => Some(vec![
            (Preset::Explosion, 140, [1.0, 0.28, 0.035]),
            (Preset::Sparks, 80, [1.0, 0.75, 0.18]),
            (Preset::Smoke, 40, [0.34, 0.35, 0.4]),
        ]),
        "sparks" => one(Preset::Sparks, 70, [1.0, 0.75, 0.18]),
        "smoke" => one(Preset::Smoke, 50, [0.34, 0.35, 0.4]),
        "dust" => one(Preset::Dust, 60, [0.6, 0.55, 0.45]),
        "fire" => one(Preset::Fire, 80, [1.0, 0.32, 0.04]),
        "splash" => one(Preset::Splash, 70, [0.35, 0.75, 1.0]),
        "magic" => one(Preset::Magic, 70, [0.55, 0.2, 1.0]),
        "steam" => one(Preset::Steam, 50, [0.7, 0.78, 0.82]),
        "embers" => one(Preset::Embers, 60, [1.0, 0.22, 0.015]),
        _ => None,
    }
}

/// Queues a burst: `name` is `explode`, `sparks`, `smoke`, `dust`, `fire`, `splash`, `magic`, `steam` or `embers`; `scale` is its size
/// (1 is the standard one). Returns whether the name is one of those.
pub(crate) fn request(name: &str, position: [f64; 3], scale: f32) -> bool {
    let Some(layers) = layers(name) else {
        return false;
    };
    if position.iter().any(|v| !v.is_finite()) {
        return false;
    }
    #[expect(clippy::cast_possible_truncation, reason = "a position in metres")]
    let position = Vec3::new(position[0] as f32, position[1] as f32, position[2] as f32);
    let scale = if scale.is_finite() {
        scale.clamp(0.1, 10.0)
    } else {
        1.0
    };
    REQUESTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.len() < MAX_LIVE {
            queue.push(Request {
                layers,
                position,
                scale,
            });
        }
    });
    true
}

/// Makes the systems of requested bursts, steps every live one, and shows their particles on the pool.
pub(crate) fn step(
    mut commands: Commands,
    time: Res<Time>,
    mut bursts: ResMut<Bursts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut quads: Query<(&mut Transform, &mut VeMaterial, &mut Visibility), With<BurstQuad>>,
    cameras: Query<&Transform, (With<Camera3d>, Without<BurstQuad>)>,
) {
    let requests: Vec<Request> = QUEUE.with(|queue| std::mem::take(&mut *queue.borrow_mut()));
    if requests.is_empty() && bursts.live.is_empty() {
        DRAWN.store(0, std::sync::atomic::Ordering::Relaxed);
        return;
    }
    if !bursts.pool_made {
        bursts.pool_made = true;
        let quad = meshes.add(crate::fx::quad_mesh());
        for _ in 0..POOL {
            commands.spawn((
                BurstQuad,
                Mesh3d(quad.clone()),
                VeMaterial::default(),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
    }
    for (index, request) in requests.into_iter().enumerate() {
        if bursts.live.len() >= MAX_LIVE {
            break;
        }
        let systems = request
            .layers
            .iter()
            .enumerate()
            .map(|(layer, (preset, count, tint))| {
                let mut spec = preset.spec(
                    request.scale,
                    Colour {
                        r: tint[0],
                        g: tint[1],
                        b: tint[2],
                        a: 1.0,
                    },
                );
                spec.seed = 0x4255_5253_0000 + (bursts.live.len() * 16 + index * 4 + layer) as u64;
                spec.max_particles = 140;
                spec.spawn_rate = 0.0;
                spec.bursts.clear();
                let mut system = ParticleSystem::new(spec);
                system.burst(*count);
                system
            })
            .collect();
        bursts.live.push((systems, request.position, 0.0));
    }
    let camera = cameras.iter().next().copied().unwrap_or_default();
    let dt = time.delta_secs();
    let mut drawn = Vec::new();
    for (systems, origin, age) in &mut bursts.live {
        *age += dt;
        for system in systems.iter_mut() {
            system.step(dt, Mat4::from_translation(*origin));
            drawn.extend(system.sorted_instances(ParticleVec3 {
                x: camera.translation.x,
                y: camera.translation.y,
                z: camera.translation.z,
            }));
        }
    }
    bursts.live.retain(|(systems, _, age)| {
        *age < LIFETIME
            && (*age < 0.2 || systems.iter().any(|system| !system.particles().is_empty()))
    });
    // Farthest first, so the soft puffs blend in order; whatever does not fit the pool is dropped.
    drawn.sort_by(|a, b| {
        let d = |p: &ve_particles::ParticleInstance| {
            let (x, y, z) = (
                p.position.x - camera.translation.x,
                p.position.y - camera.translation.y,
                p.position.z - camera.translation.z,
            );
            x * x + y * y + z * z
        };
        d(b).total_cmp(&d(a))
    });
    drawn.truncate(POOL);
    DRAWN.store(drawn.len(), std::sync::atomic::Ordering::Relaxed);
    for (index, (mut transform, mut material, mut visibility)) in quads.iter_mut().enumerate() {
        let Some(particle) = drawn.get(index) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        *visibility = Visibility::Visible;
        transform.translation = Vec3::new(
            particle.position.x,
            particle.position.y,
            particle.position.z,
        );
        transform.rotation = camera.rotation * bevy_math::Quat::from_rotation_z(particle.rotation);
        transform.scale = Vec3::splat(particle.size.max(0.001));
        let c = particle.colour;
        material.base_color = Color::linear_rgba(c.r, c.g, c.b, c.a);
        material.opacity = c.a;
        material.emissive = Color::linear_rgb(c.r * 4.0, c.g * 4.0, c.b * 4.0);
        material.roughness = 0.0;
    }
}
