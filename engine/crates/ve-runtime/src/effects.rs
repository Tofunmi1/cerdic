//! Particle effects authored in the language (`fx.emitter`, plan W4): each emitting part runs a `ve_particles::fx::System` placed at the part's pose,
//! and one dynamic mesh per emitter is rebuilt each frame from the live particles (soft camera-facing sprites, or ribbons along trails).
//! Drawn through the Bevy renderer only: the mesh's material asks for the flat, vertex-coloured, optionally additive look in `ve-bevy`.
use std::{collections::HashMap, sync::Arc};

use bevy_asset::{Assets, Handle, RenderAssetUsages};
use bevy_camera::{
    Camera3d,
    visibility::{NoFrustumCulling, Visibility},
};
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_time::Time;
use bevy_transform::components::{GlobalTransform, Transform};
use ve_modeling::fx::PartFx;
use ve_particles::fx::{
    Blend, Contact, Look, Plane, Render, System, World,
    draw::{Batch, Camera, cubes, ribbons, sprites},
};
use ve_render::{SurfaceFinish, VeMaterial};

/// What a particle can hit: the effect's ground plane, and (in play mode) every physics body of the scene; the deeper contact wins.
struct SceneWorld<'a> {
    ground: Option<Plane>,
    physics: std::cell::RefCell<&'a mut crate::physics::Physics>,
}

impl World for SceneWorld<'_> {
    fn contact(&self, position: [f32; 3], radius: f32) -> Option<Contact> {
        let plane = self
            .ground
            .and_then(|plane| plane.contact(position, radius));
        let body = self
            .physics
            .borrow_mut()
            .surface(position, radius)
            .map(|hit| Contact {
                normal: hit.normal,
                depth: hit.depth,
            });
        match (plane, body) {
            (Some(a), Some(b)) => Some(if a.depth >= b.depth { a } else { b }),
            (a, b) => a.or(b),
        }
    }
}

/// A part that emits particles: its effect, and the middle of the part when it was authored (where particles are born, wherever it has moved).
#[derive(Component, Clone)]
pub(crate) struct EffectEmitter {
    spec: Arc<PartFx>,
    centre: Vec3,
}

impl EffectEmitter {
    pub(crate) fn new(spec: Arc<PartFx>, centre: Vec3) -> Self {
        Self { spec, centre }
    }
}

/// The drawn mesh of one emitter.
#[derive(Component)]
pub(crate) struct EffectMesh;

struct Live {
    system: System,
    /// For each emitter of the effect: its mesh and the entity that draws it.
    draws: Vec<(Handle<Mesh>, Entity)>,
}

/// The running effect of every emitting part.
#[derive(Resource, Default)]
pub(crate) struct Running(HashMap<Entity, Live>);

fn mesh_of(batch: &Batch) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let count = batch.positions.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colours.clone());
    // The forward pipeline reads baked occlusion at location 2 for every mesh; particles are fully open.
    mesh.insert_attribute(ve_render::ATTRIBUTE_OCCLUSION, vec![1.0_f32; count]);
    mesh.insert_indices(Indices::U32(batch.indices.clone()));
    mesh
}

fn surface(look: &Look) -> VeMaterial {
    if look.render == Render::Cube {
        // Debris is an ordinary lit surface painted by its vertex colours.
        return VeMaterial {
            base_color: Color::WHITE,
            roughness: 0.8,
            ..VeMaterial::default()
        };
    }
    VeMaterial {
        base_color: Color::WHITE,
        finish: SurfaceFinish {
            particle: if look.blend == Blend::Additive { 2 } else { 1 },
            ..SurfaceFinish::default()
        },
        ..VeMaterial::default()
    }
}

/// Starts the simulation and the drawn meshes of a newly seen emitting part.
fn start(
    entity: Entity,
    emitter: &EffectEmitter,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
) -> Option<Live> {
    let system = System::new(
        emitter.spec.effect.clone(),
        u64::from(entity.index_u32()) ^ 0x4658_0000,
    )
    .ok()?;
    let draws = emitter
        .spec
        .effect
        .emitters
        .iter()
        .map(|each| {
            // One invisible triangle: the renderer warns about a mesh with no vertices, and the first real batch replaces it.
            let placeholder = Batch {
                positions: vec![[0.0; 3]; 3],
                colours: vec![[0.0; 4]; 3],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
            };
            let handle = meshes.add(mesh_of(&placeholder));
            let drawn = commands
                .spawn((
                    EffectMesh,
                    Mesh3d(handle.clone()),
                    surface(&each.look),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    // The mesh is rebuilt every frame and its bounds are not: never cull it by the first (empty) box.
                    NoFrustumCulling,
                ))
                .id();
            (handle, drawn)
        })
        .collect();
    Some(Live { system, draws })
}

/// Runs every emitting part's effect and redraws its meshes.
pub(crate) fn step(
    time: Res<Time>,
    clock: Res<crate::animation::AnimationClock>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut running: ResMut<Running>,
    mut physics: ResMut<crate::physics::Physics>,
    emitters: Query<(Entity, &EffectEmitter, &GlobalTransform)>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut visibility: Query<&mut Visibility, With<EffectMesh>>,
) {
    // Drop the effects of parts that are gone (the scene was replaced or the part deleted).
    let gone: Vec<Entity> = running
        .0
        .keys()
        .copied()
        .filter(|key| emitters.get(*key).is_err())
        .collect();
    for key in gone {
        if let Some(live) = running.0.remove(&key) {
            for (_, drawn) in live.draws {
                commands.entity(drawn).despawn();
            }
        }
    }
    let view = cameras.iter().next().map(|camera| {
        let (_, rotation, translation) = camera.to_scale_rotation_translation();
        Camera {
            position: translation.to_array(),
            right: (rotation * Vec3::X).to_array(),
            up: (rotation * Vec3::Y).to_array(),
        }
    });
    let dt = time.delta_secs() * clock.speed;
    for (entity, emitter, global) in &emitters {
        if !running.0.contains_key(&entity) {
            if let Some(live) = start(entity, emitter, &mut commands, &mut meshes) {
                running.0.insert(entity, live);
            }
            continue;
        }
        let Some(live) = running.0.get_mut(&entity) else {
            continue;
        };
        let (_, rotation, _) = global.to_scale_rotation_translation();
        live.system.place(
            global.transform_point(emitter.centre).to_array(),
            rotation.to_array(),
        );
        let world = SceneWorld {
            ground: emitter.spec.ground.map(|height| Plane { height }),
            physics: std::cell::RefCell::new(&mut physics),
        };
        live.system.step(dt, &world);
        let Some(camera) = &view else {
            continue;
        };
        for (index, (handle, drawn)) in live.draws.iter().enumerate() {
            let each = &emitter.spec.effect.emitters[index];
            let batch = match each.look.render {
                Render::Ribbon => ribbons(&live.system, &emitter.spec.effect, index, camera),
                Render::Cube => cubes(&live.system, &emitter.spec.effect, index),
                Render::Sprite => sprites(&live.system, &emitter.spec.effect, index, camera),
            };
            if let Ok(mut shown) = visibility.get_mut(*drawn) {
                *shown = if batch.positions.is_empty() {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                };
            }
            if !batch.positions.is_empty()
                && let Some(mut mesh) = meshes.get_mut(handle)
            {
                *mesh = mesh_of(&batch);
            }
        }
    }
}

/// `fx.status {}`: every emitting part with, for each of its emitters, how many particles are alive and the box they fill:
/// `{parts: [{part, emitters: [{name, alive, min, max}]}]}`. For checking that an effect runs and where it goes.
pub(crate) fn status(world: &mut bevy_ecs::world::World) -> serde_json::Value {
    let mut query = world.query::<(Entity, &crate::projection::Authored, &EffectEmitter)>();
    let rows: Vec<(Entity, String, Arc<PartFx>)> = query
        .iter(world)
        .map(|(entity, authored, emitter)| {
            (
                entity,
                authored.name().to_owned(),
                Arc::clone(&emitter.spec),
            )
        })
        .collect();
    let running = world.resource::<Running>();
    let parts: Vec<serde_json::Value> = rows
        .iter()
        .map(|(entity, name, spec)| {
            let emitters: Vec<serde_json::Value> = spec
                .names
                .iter()
                .enumerate()
                .map(|(index, emitter)| {
                    let alive = running.0.get(entity).map(|live| live.system.alive(index)).unwrap_or_default();
                    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for particle in &alive {
                        for axis in 0..3 {
                            low[axis] = low[axis].min(particle.position[axis]);
                            high[axis] = high[axis].max(particle.position[axis]);
                        }
                    }
                    if alive.is_empty() {
                        serde_json::json!({ "name": emitter, "alive": 0 })
                    } else {
                        serde_json::json!({ "name": emitter, "alive": alive.len(), "min": low, "max": high })
                    }
                })
                .collect();
            serde_json::json!({ "part": name, "emitters": emitters })
        })
        .collect();
    serde_json::json!({ "parts": parts })
}
