use crate::{
    document::staging,
    interact::{Interactable, Label},
};
use bevy_asset::{Assets, Handle};
use bevy_color::Color;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::With,
    system::{Commands, Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::Transform;
use serde_json::json;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use ve_kernel::Bounds;
use ve_modeling::{JobToken, SceneEnvironment, SceneLight};
use ve_render::{VeAmbientLight, VeMaterial, mesh_ready};

const MAX_CHUNK_BYTES: usize = 64 * 1024;
const MAX_QUEUED_BYTES: usize = 8 * 1024 * 1024;
/// How long, and how many bytes, one frame spends turning staged chunks into meshes.
const FRAME_UPLOAD_MS: f64 = 6.0;
const FRAME_UPLOAD_BYTES: usize = 2 * 1024 * 1024;
pub(crate) mod bulk;
pub(crate) mod environment;
pub(crate) mod gi;
pub(crate) mod lights;
mod maps;
mod placements;
mod surfaces;
pub(crate) mod survey;
mod upload;
pub(crate) use maps::{live_update, maps};

/// Puts the live texture pictures the page queued (`viewer_texture_update`) into the renderer's maps; runs every frame.
pub(crate) fn apply_live_textures(mut textures: bevy_ecs::system::ResMut<ve_render::MaterialTextures>) {
    maps::apply_live(&mut textures);
}
mod motion;
use maps::{Maps, assign_layers};
pub(crate) use motion::{animation, behavior, body, cloth, fluid, fx, ik, machine, parent, ragdoll, sim, skin};
pub(crate) use placements::{InstanceIndex, append as instances};
use upload::upload;

/// Generated chunks are removed together when the authored revision changes.
#[derive(Component)]
pub(crate) struct Authored {
    node: String,
    name: String,
    revision: u64,
}

impl Authored {
    /// A part of a game entity's own copy of a model (ADR 0066), with the unique names the copy gives it.
    pub(crate) fn instance(name: String, node: String) -> Self {
        Self { node, name, revision: 0 }
    }

    /// The document node whose output this part shows.
    pub(crate) fn node(&self) -> &str {
        &self.node
    }

    /// The output part name rendered by this entity.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

/// The initial comparison props disappear on the first evaluated document swap.
#[derive(Component)]
pub(crate) struct GalleryObject;

struct Chunk {
    group: u32,
    region: u32,
    node: String,
    name: String,
    data: Vec<f32>,
    /// Two more floats per vertex: texture coordinates.
    textured: bool,
    /// Four more floats per vertex: a vertex colour.
    coloured: bool,
}
struct Uploaded {
    group: u32,
    region: u32,
    node: String,
    name: String,
    handle: Handle<Mesh>,
    bounds: Bounds,
    /// Rest position and normal per corner, kept only for a skinned part.
    rest: Vec<f32>,
    /// Which corner of the whole part this chunk starts at (skinned parts only).
    corner_offset: usize,
}
/// A physics body waiting for the scene to commit, with the vertices its shape is built from.
struct StagedBody {
    body: ve_modeling::physics::PhysBody,
    points: Vec<f32>,
    triangles: Vec<u32>,
}
struct Candidate {
    token: JobToken,
    /// Surfaces by authored node and material region (0 unless one solid has several colours); a
    /// part without one is drawn in the default blue-grey.
    materials: HashMap<(String, u32), VeMaterial>,
    /// Baked texture maps by authored node and material region.
    maps: HashMap<(String, u32), Maps>,
    /// What the distinct staged maps take on the GPU, kept as they arrive.
    map_costs: ve_render::MapCosts,
    /// Keyframed transform tracks by authored node (`anim.track`).
    animations: HashMap<String, Vec<ve_modeling::anim::Track>>,
    /// Responses by authored node (`behavior.on`).
    behaviors: HashMap<String, ve_modeling::anim::Behavior>,
    /// Animation state machines by authored node (`anim.machine`).
    machines: HashMap<String, std::sync::Arc<ve_animstate::StateMachine>>,
    /// The part each authored node's part follows (`anim.parent`), by output name.
    parents: HashMap<String, String>,
    /// Skin data by authored node (`rig.skin`), and how many corners of each have been uploaded so far.
    skins: HashMap<String, std::sync::Arc<crate::skinning::SkinData>>,
    corners: HashMap<String, usize>,
    /// Chains of bones reaching for a target, by the target's authored node (`rig.ik`), and the names of the bones in them.
    iks: HashMap<String, std::sync::Arc<ve_modeling::rig::IkChain>>,
    ik_bones: std::collections::HashSet<String>,
    /// Particle effects by authored node (`fx.emitter`).
    fxs: HashMap<String, std::sync::Arc<ve_modeling::fx::PartFx>>,
    /// Ragdoll definitions by authored node (`phys.ragdoll`).
    ragdolls: HashMap<String, std::sync::Arc<ve_modeling::ragdoll::RagdollSpec>>,
    /// Live cloths by authored node (`cloth.sim`); the chunks of a part share the solver.
    cloths: HashMap<String, std::sync::Arc<crate::cloth_live::Shared>>,
    /// Live liquids by authored node (`fluid.sim`); the chunks of a part share the solver.
    fluids: HashMap<String, std::sync::Arc<crate::fluid_live::Shared>>,
    /// Simulation programs by authored node (`sim.program`); the chunks of a part share the instance.
    sims: HashMap<String, std::sync::Arc<crate::sim_live::Shared>>,
    /// Physics bodies by authored node (`phys.body`).
    bodies: HashMap<String, StagedBody>,
    placements: HashMap<u32, placements::Placements>,
    lights: Vec<SceneLight>,
    environment: Option<SceneEnvironment>,
    placement_count: usize,
    queue: VecDeque<Chunk>,
    uploaded: Vec<Uploaded>,
    queued_bytes: usize,
    total_bytes: usize,
    finished: bool,
    /// A draft shows the parts finished so far while the evaluation continues: it replaces the scene on screen but never commits the
    /// document, never counts as the committed scene, and does not trace light probes. The finished evaluation replaces it.
    draft: bool,
}
#[derive(Default)]
struct State {
    candidate: Option<Candidate>,
    committed: Option<JobToken>,
    discarded: Vec<Handle<Mesh>>,
    upload_frames: usize,
    max_frame_bytes: usize,
    last_ms: f64,
    max_ms: f64,
    parts: Vec<(String, u64)>,
    light_count: usize,
    /// How many parts of the scene on screen carry `anim.track` tracks.
    animated_parts: usize,
    /// The environment applied by the last commit, or `None` when the document authored none.
    environment: Option<SceneEnvironment>,
    instance_sets: std::collections::BTreeMap<String, usize>,
    /// Baked texture maps held by the scene on screen, and the GPU bytes they take (mip chains included).
    texture_maps: usize,
    texture_bytes: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

pub(crate) fn begin(text: &str) -> bool {
    let Ok(token) = serde_json::from_str::<JobToken>(text) else {
        return false;
    };
    if !staging::current(&token) {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        discard(&mut state);
        state.candidate = Some(Candidate {
            token,
            materials: HashMap::new(),
            maps: HashMap::new(),
            map_costs: ve_render::MapCosts::default(),
            animations: HashMap::new(),
            behaviors: HashMap::new(),
            machines: HashMap::new(),
            parents: HashMap::new(),
            skins: HashMap::new(),
            corners: HashMap::new(),
            iks: HashMap::new(),
            ik_bones: std::collections::HashSet::new(),
            fxs: HashMap::new(),
            ragdolls: HashMap::new(),
            cloths: HashMap::new(),
            fluids: HashMap::new(),
            sims: HashMap::new(),
            bodies: HashMap::new(),
            placements: HashMap::new(),
            lights: Vec::new(),
            environment: None,
            placement_count: 0,
            queue: VecDeque::new(),
            uploaded: Vec::new(),
            queued_bytes: 0,
            total_bytes: 0,
            finished: false,
            draft: false,
        });
    });
    true
}

/// Sets the surface of the part made by `node` in the scene being staged.
pub(crate) fn material(text: &str, node: &str, json: &str) -> bool {
    material_region(text, node, 0, json)
}

/// Sets the surface of one material region of the part made by `node` (a fused solid of several
/// colours has one region per colour).
pub(crate) fn material_region(text: &str, node: &str, region: u32, json: &str) -> bool {
    let (Some(parsed), true) = (
        surfaces::parse(json, node),
        ve_graph::NodeId::parse(node).is_ok(),
    ) else {
        return false;
    };
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.materials.len() >= 4096
        {
            return false;
        }
        candidate
            .materials
            .insert((node.to_owned(), region), parsed);
        true
    })
}

/// A chunk with seven floats per vertex (position, normal, occlusion) and no texture coordinates.
pub(crate) fn chunk(text: &str, node: &str, name: &str, data: &[f32]) -> bool {
    chunk_group(text, 0, node, name, data)
}

pub(crate) fn chunk_group(text: &str, group: u32, node: &str, name: &str, data: &[f32]) -> bool {
    chunk_part(text, (group, 0), node, name, data, 0)
}

/// A chunk of one part: `(group, region)` says which placement set and which material region. `layout`
/// bit 0 gives each vertex two more floats, texture coordinates (only parts with baked maps need them,
/// which keeps every other mesh 22% smaller on the GPU); bit 1 gives it four more, a vertex colour
/// (water carries flow, depth and foam that way).
pub(crate) fn chunk_part(
    text: &str,
    (group, region): (u32, u32),
    node: &str,
    name: &str,
    data: &[f32],
    layout: u32,
) -> bool {
    let (textured, coloured) = (layout & 1 != 0, layout & 2 != 0);
    let triangle = 3 * (7 + if textured { 2 } else { 0 } + if coloured { 4 } else { 0 });
    if !data.len().is_multiple_of(triangle)
        || data.is_empty()
        || size_of_val(data) > MAX_CHUNK_BYTES
        || name.len() > 256
        || ve_graph::NodeId::parse(node).is_err()
        || data.iter().any(|v| !v.is_finite() || v.abs() > 1.0e6)
    {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.queued_bytes + size_of_val(data) > MAX_QUEUED_BYTES
            || candidate.total_bytes + size_of_val(data) > 144 * 1024 * 1024
            || (group != 0
                && candidate
                    .placements
                    .get(&group)
                    .is_none_or(|p| p.node != node))
        {
            return false;
        }
        candidate.queued_bytes += size_of_val(data);
        candidate.total_bytes += size_of_val(data);
        candidate.queue.push_back(Chunk {
            group,
            region,
            node: node.to_owned(),
            name: name.to_owned(),
            data: data.to_vec(),
            textured,
            coloured,
        });
        true
    })
}

/// Like [`finish`], but the staged scene is only a draft (see `Candidate::draft`).
pub(crate) fn finish_draft(text: &str) -> bool {
    STATE.with(|cell| {
        if !finish(text) {
            return false;
        }
        if let Some(candidate) = cell.borrow_mut().candidate.as_mut() {
            candidate.draft = true;
        }
        true
    })
}

pub(crate) fn finish(text: &str) -> bool {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
        {
            return false;
        }
        candidate.finished = true;
        true
    })
}

fn discard(state: &mut State) {
    if let Some(candidate) = state.candidate.take() {
        state
            .discarded
            .extend(candidate.uploaded.into_iter().map(|item| item.handle));
    }
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |clock| clock.now())
}
