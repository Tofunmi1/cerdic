//! Game entities drawn as their own copies of an animated model (ADR 0066).
//!
//! A kind whose part is the root of an animated hierarchy (parts that follow parts, a state machine, IK or gait chains) is a *model kind*.
//! Every game entity of it gets a clone of each part of the hierarchy with names made unique by `#<entity id>`, its own state machine and
//! its own gait; the game places the clone every frame (`InstancePlacement`) from the entity's position and heading, and feeds the machine
//! the speed the entity really moves at. The authored hierarchy stays in the scene, hidden, as the prototype.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy_asset::Assets;
use bevy_camera::visibility::{NoFrustumCulling, Visibility};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{With, Without},
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use bevy_math::{Mat4, Vec3};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_time::{Real, Time};
use bevy_transform::components::Transform;
use ve_modeling::rig::Blend;
use ve_render::VeMaterial;

use crate::{
    animation::{Animated, IkTarget, InstancePlacement, MachineDrive},
    cloth_live::{ClothData, ClothLive, Shared},
    physics::Body,
    sim_live::SimLive,
    game::GameRuntime,
    game_view::render_position,
    projection::Authored,
    skinning::{SkinData, Skinned},
};

/// The most live instances of models at once (a model of N parts costs N entities each).
const MAX_INSTANCES: usize = 128;
/// Longest chain of parents followed when looking for the root of a hierarchy.
const MAX_DEPTH: usize = 32;

/// One part of a game entity's copy of a model.
#[derive(Component)]
pub(crate) struct ModelPart {
    /// The game entity this copy belongs to.
    entity: u32,
    /// Where the part stood in the authored scene (a part with no animation of its own is placed by the game from this).
    base: Transform,
}

/// An authored part hidden because a model kind draws copies of it; remembers how it was shown.
#[derive(Component)]
pub(crate) struct ModelPrototype(Option<Visibility>);

/// The live instances: what was cloned for each game entity, and which of its parts carry a state machine.
#[derive(Resource, Default)]
pub(crate) struct ModelInstances {
    live: HashMap<u32, Instance>,
    /// Parts of kinds that are models, so the plain per-entity copies skip them.
    pub(crate) model_parts: HashSet<String>,
    last_seen: HashMap<u32, Vec3>,
}

struct Instance {
    part: String,
    parts: Vec<Entity>,
    /// The names (suffixed) of the parts that carry a state machine.
    machines: Vec<String>,
    /// How many of `parts` are skinned chunks.
    skins: usize,
}

impl ModelInstances {
    /// The state machine parts of this game entity's copy of its model, `None` when it is not drawn as a model.
    pub(crate) fn machines_of(&self, entity: u32) -> Option<&[String]> {
        self.live
            .get(&entity)
            .map(|instance| instance.machines.as_slice())
    }

    /// How many skinned chunks the copies of models carry in all.
    pub(crate) fn skinned_parts(&self) -> usize {
        self.live.values().map(|instance| instance.skins).sum()
    }
}

/// The part names that belong to the hierarchy of `part`: it, its parents up to the root, and every part whose root is that one.
fn hierarchy_of(part: &str, parents: &HashMap<&str, Option<&str>>) -> HashSet<String> {
    let root_of = |name: &str| {
        let mut at = name.to_owned();
        for _ in 0..MAX_DEPTH {
            match parents.get(at.as_str()).copied().flatten() {
                Some(parent) => at = parent.to_owned(),
                None => break,
            }
        }
        at
    };
    let root = root_of(part);
    let mut members: HashSet<String> = parents
        .keys()
        .filter(|name| root_of(name) == root)
        .map(|name| (*name).to_owned())
        .collect();
    members.insert(part.to_owned());
    members.insert(root);
    members
}

fn suffixed(name: &str, id: u32) -> String {
    format!("{name}#{id}")
}

/// The chain of an IK or gait target with every name it mentions made the instance's own.
fn renamed_chain(chain: &ve_modeling::rig::IkChain, id: u32) -> ve_modeling::rig::IkChain {
    let mut chain = chain.clone();
    for bone in &mut chain.bones {
        *bone = suffixed(bone, id);
    }
    if let Some(gait) = chain.gait.as_mut() {
        gait.body = suffixed(&gait.body, id);
    }
    chain
}

/// The authored parts a model can be cloned from (not the clones themselves).
type Sources<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Authored,
        &'static Mesh3d,
        &'static VeMaterial,
        &'static Transform,
        Option<&'static Animated>,
        Option<&'static MachineDrive>,
        Option<&'static IkTarget>,
        Option<&'static Visibility>,
        Option<&'static ModelPrototype>,
        Option<&'static Skinned>,
        Option<&'static ClothLive>,
        Option<&'static SimLive>,
    ),
    Without<ModelPart>,
>;

/// Keeps one clone of the model per game entity of a model kind, placed and fed its speed; removes the clones of entities that are gone.
#[expect(
    clippy::too_many_lines,
    reason = "one system keeps instances, placements, speeds and hidden prototypes in step"
)]
pub(crate) fn sync(
    mut commands: Commands,
    time: Res<Time<Real>>,
    runtime: Res<GameRuntime>,
    mut state: ResMut<ModelInstances>,
    mut placements: ResMut<InstancePlacement>,
    mut meshes: ResMut<Assets<Mesh>>,
    sources: Sources,
    bodies: Query<&Body>,
    mut clones: Query<(&ModelPart, &mut Transform), (With<ModelPart>, Without<Animated>, Without<Body>)>,
    alive: Query<(), With<Authored>>,
) {
    let Some(world) = runtime.world.as_ref() else {
        retire_all(&mut commands, &mut state, &mut placements, &sources);
        return;
    };
    let ModelInstances {
        live,
        model_parts,
        last_seen,
    } = &mut *state;
    let mut parents: HashMap<&str, Option<&str>> = HashMap::new();
    for (.., animated, _, _, _, _, _, _, _) in &sources {
        if let Some(animated) = animated {
            parents.insert(animated.name.as_str(), animated.parent.as_deref());
        }
    }
    // Which kinds are models: their part has a hierarchy, a machine or tracks of its own, and no skinned mesh among them.
    let mut models: HashMap<&str, HashSet<String>> = HashMap::new();
    model_parts.clear();
    for kind in world.kinds() {
        let Some(part) = kind.part.as_deref() else {
            continue;
        };
        let mut members = hierarchy_of(part, &parents);
        // A skinned part is never parented (it would lose its skin); it belongs to the model whose bones it follows.
        let skins: Vec<String> = sources
            .iter()
            .filter(|(.., skin, _, _)| {
                skin.is_some_and(|skin| skin.data.bones.iter().any(|bone| members.contains(bone)))
            })
            .map(|(_, authored, ..)| authored.name().to_owned())
            .collect();
        members.extend(skins);
        // A live cloth (a cape pinned to a chest bone, a skirt that collides with the legs) belongs to the model whose bone holds its pins.
        let cloths: Vec<String> = sources
            .iter()
            .filter(|(.., cloth, _)| {
                cloth.is_some_and(|cloth| {
                    let spec = &cloth.shared.data().spec;
                    members.contains(&spec.pin_bone)
                        || spec.collide_bones.iter().any(|bone| members.contains(bone))
                })
            })
            .map(|(_, authored, ..)| authored.name().to_owned())
            .collect();
        members.extend(cloths);
        // A simulation program belongs to the model whose bones it reads (a cape on the chest, hair on the head).
        let sims: Vec<String> = sources
            .iter()
            .filter(|(.., sim)| sim.is_some_and(|sim| sim.shared.bone_parts().any(|bone| members.contains(bone))))
            .map(|(_, authored, ..)| authored.name().to_owned())
            .collect();
        members.extend(sims);
        let own = sources
            .iter()
            .any(|(_, authored, _, _, _, animated, machine, ..)| {
                authored.name() == part && (animated.is_some() || machine.is_some())
            });
        if members.len() > 1 || own {
            models.insert(part, members);
            model_parts.insert(part.to_owned());
        }
    }
    let dt = time.delta_secs().max(1e-4);
    let mut wanted: HashSet<u32> = HashSet::new();
    for entity in world.entities() {
        let Some(part) = world
            .kind(&entity.kind)
            .and_then(|kind| kind.part.as_deref())
        else {
            continue;
        };
        let Some(members) = models.get(part) else {
            continue;
        };
        if wanted.len() >= MAX_INSTANCES {
            break;
        }
        wanted.insert(entity.id);
        let position = render_position(entity.position);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a heading in radians, well inside f32"
        )]
        let placement =
            Mat4::from_translation(position) * Mat4::from_rotation_y(entity.heading as f32);
        placements.0.insert(entity.id, placement);
        // An instance whose clones are gone (the document was evaluated again) is built again.
        let intact = live.get(&entity.id).is_some_and(|instance| {
            instance.part == part && instance.parts.iter().all(|part| alive.contains(*part))
        });
        if !intact {
            if let Some(old) = live.remove(&entity.id) {
                for part in old.parts {
                    commands.entity(part).despawn();
                }
            }
            let mut parts = Vec::new();
            let mut machines = Vec::new();
            let mut skins = 0;
            // One renamed skin per source skin, shared by its chunks, so a copy's chunks share one set of joints.
            let mut renamed: HashMap<usize, Arc<SkinData>> = HashMap::new();
            for (source, authored, mesh, material, transform, animated, machine, ik, _, _, skin, cloth, sim) in
                &sources
            {
                if !members.contains(authored.name()) {
                    continue;
                }
                let name = suffixed(authored.name(), entity.id);
                if let Some(sim) = sim {
                    // Each copy runs its own program instance on its own mesh, reading the bones of this copy.
                    let own = meshes.get(&mesh.0).cloned().map(|rest| Mesh3d(meshes.add(rest)));
                    let clone = commands.spawn((
                        own.unwrap_or_else(|| mesh.clone()),
                        *material,
                        *transform,
                        SimLive {
                            shared: sim.shared.copy(&|part| suffixed(part, entity.id)),
                            corner_offset: sim.corner_offset,
                        },
                        NoFrustumCulling,
                        Authored::instance(name, suffixed(authored.node(), entity.id)),
                    ));
                    parts.push(clone.id());
                    continue;
                }
                if let Some(cloth) = cloth {
                    // Each copy gets its own solver and its own mesh (the solver rewrites the points every frame); the bones it names are this copy's.
                    let data = cloth.shared.data();
                    let mut spec = data.spec.clone();
                    spec.pin_bone = suffixed(&spec.pin_bone, entity.id);
                    for bone in &mut spec.collide_bones {
                        *bone = suffixed(bone, entity.id);
                    }
                    let copy = Arc::new(ClothData {
                        spec,
                        rest: data.rest.clone(),
                        corners: data.corners.clone(),
                    });
                    let own = meshes.get(&mesh.0).cloned().map(|rest| Mesh3d(meshes.add(rest)));
                    let clone = commands.spawn((
                        own.unwrap_or_else(|| mesh.clone()),
                        *material,
                        *transform,
                        ClothLive {
                            shared: Shared::new(copy),
                            corner_offset: cloth.corner_offset,
                        },
                        NoFrustumCulling,
                        Authored::instance(name, suffixed(authored.node(), entity.id)),
                    ));
                    parts.push(clone.id());
                    continue;
                }
                if let Some(skin) = skin {
                    // The vertices are in world space at rest and the bone poses carry the placement, so the copy is not placed.
                    let data = renamed
                        .entry(Arc::as_ptr(&skin.data) as usize)
                        .or_insert_with(|| Arc::new(skin.data.for_instance(entity.id)));
                    let linear = data.blend == Blend::Linear;
                    // The GPU moves a linear skin, so copies share its mesh; the CPU rewrites any other, so each copy gets its own.
                    let own = (!linear)
                        .then(|| meshes.get(&mesh.0).cloned())
                        .flatten()
                        .map(|rest| Mesh3d(meshes.add(rest)));
                    let mut clone = commands.spawn((
                        own.unwrap_or_else(|| mesh.clone()),
                        *material,
                        *transform,
                        Skinned::new(Arc::clone(data), skin.rest.to_vec(), skin.corner_offset),
                        NoFrustumCulling, // the deformed chunk outgrows the box Bevy measured at the origin
                        Authored::instance(name, suffixed(authored.node(), entity.id)),
                    ));
                    if linear {
                        // `attach_gpu_skins` shows a linear skin once its joints exist (see `skinning_gpu`).
                        clone.insert(Visibility::Hidden);
                    }
                    parts.push(clone.id());
                    skins += 1;
                    continue;
                }
                // A part with a physics body starts where the copy is placed and is moved by the physics world from then on.
                let body = bodies.get(source).ok();
                let start = if body.is_some() {
                    Transform::from_matrix(placement * transform.to_matrix())
                } else {
                    *transform
                };
                let mut clone = commands.spawn((
                    mesh.clone(),
                    *material,
                    start,
                    ModelPart {
                        entity: entity.id,
                        base: *transform,
                    },
                    Authored::instance(name.clone(), suffixed(authored.node(), entity.id)),
                ));
                if let Some(body) = body {
                    clone.insert(body.copy_for(&|part| suffixed(part, entity.id)));
                }
                if let Some(animated) = animated {
                    clone.insert(Animated {
                        tracks: animated.tracks.clone(),
                        name: suffixed(&animated.name, entity.id),
                        node: suffixed(&animated.node, entity.id),
                        parent: animated
                            .parent
                            .as_deref()
                            .map(|parent| suffixed(parent, entity.id)),
                        instance: Some(entity.id),
                    });
                }
                if let Some(machine) = machine {
                    clone.insert(machine.fresh());
                    machines.push(name);
                }
                if let Some(ik) = ik {
                    clone.insert(IkTarget {
                        chain: std::sync::Arc::new(renamed_chain(&ik.chain, entity.id)),
                        centre: ik.centre,
                    });
                }
                parts.push(clone.id());
            }
            live.insert(
                entity.id,
                Instance {
                    part: part.to_owned(),
                    parts,
                    machines,
                    skins,
                },
            );
        }
        // The speed the entity really moves at drives the walk and run clips.
        if let Some(instance) = live.get(&entity.id) {
            if let Some(before) = last_seen.get(&entity.id) {
                let moved = position - *before;
                let speed = Vec3::new(moved.x, 0.0, moved.z).length() / dt;
                for machine in &instance.machines {
                    crate::animation::request_float(machine, "speed", speed);
                }
            }
        }
        last_seen.insert(entity.id, position);
    }
    // Parts that are not animated (a carrier that only holds the machine) are placed here; animated ones by `animate`.
    for (part, mut transform) in &mut clones {
        if let Some(placed) = placements.0.get(&part.entity) {
            let target = Transform::from_matrix(*placed * part.base.to_matrix());
            if *transform != target {
                *transform = target;
            }
        }
    }
    // Entities that are gone, or no longer model kinds, lose their copies.
    live.retain(|id, instance| {
        if wanted.contains(id) {
            true
        } else {
            for part in &instance.parts {
                commands.entity(*part).despawn();
            }
            false
        }
    });
    placements.0.retain(|id, _| wanted.contains(id));
    last_seen.retain(|id, _| wanted.contains(id));
    // Every authored part of a model is hidden while the kind is a model, and shown again when it stops being one.
    for (source, authored, _, _, _, _, _, _, visibility, saved, _, _, _) in &sources {
        let in_model = models
            .values()
            .any(|members| members.contains(authored.name()));
        match (in_model, saved) {
            (true, None) => {
                commands
                    .entity(source)
                    .insert((ModelPrototype(visibility.copied()), Visibility::Hidden));
            }
            (false, Some(saved)) => {
                match saved.0 {
                    Some(old) => commands.entity(source).insert(old),
                    None => commands.entity(source).remove::<Visibility>(),
                };
                commands.entity(source).remove::<ModelPrototype>();
            }
            _ => {}
        }
    }
}

/// With no game loaded, nothing is drawn as a model and every hidden prototype comes back.
fn retire_all(
    commands: &mut Commands,
    state: &mut ModelInstances,
    placements: &mut InstancePlacement,
    sources: &Sources,
) {
    for (_, instance) in state.live.drain() {
        for part in instance.parts {
            commands.entity(part).despawn();
        }
    }
    state.model_parts.clear();
    state.last_seen.clear();
    placements.0.clear();
    for (source, .., saved, _, _, _) in sources {
        if let Some(saved) = saved {
            match saved.0 {
                Some(old) => commands.entity(source).insert(old),
                None => commands.entity(source).remove::<Visibility>(),
            };
            commands.entity(source).remove::<ModelPrototype>();
        }
    }
}
