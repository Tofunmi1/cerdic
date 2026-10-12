//! Bounded mesh staging and atomic document/scene swap after GPU preparation.
use crate::{
    document::staging,
    interact::{Interactable, Label},
};
use bevy_asset::{Assets, Handle, RenderAssetUsages};
use bevy_color::Color;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::With,
    system::{Commands, Query, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d, PrimitiveTopology};
use bevy_transform::components::Transform;
use serde_json::json;
use std::cell::RefCell;
use std::collections::VecDeque;
use ve_kernel::Bounds;
use ve_modeling::JobToken;
use ve_render::{VeMaterial, mesh_ready};

const MAX_CHUNK_BYTES: usize = 64 * 1024;
const MAX_QUEUED_BYTES: usize = 8 * 1024 * 1024;

/// Generated chunks are removed together when the authored revision changes.
#[derive(Component)]
pub(crate) struct Authored {
    node: String,
    revision: u64,
}

/// The initial comparison props disappear on the first evaluated document swap.
#[derive(Component)]
pub(crate) struct GalleryObject;

struct Chunk {
    node: String,
    name: String,
    data: Vec<f32>,
}
struct Uploaded {
    node: String,
    name: String,
    handle: Handle<Mesh>,
    bounds: Bounds,
}
struct Candidate {
    token: JobToken,
    queue: VecDeque<Chunk>,
    uploaded: Vec<Uploaded>,
    queued_bytes: usize,
    total_bytes: usize,
    finished: bool,
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
            queue: VecDeque::new(),
            uploaded: Vec::new(),
            queued_bytes: 0,
            total_bytes: 0,
            finished: false,
        });
    });
    true
}

pub(crate) fn chunk(text: &str, node: &str, name: &str, data: &[f32]) -> bool {
    if !data.len().is_multiple_of(18)
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
        {
            return false;
        }
        candidate.queued_bytes += size_of_val(data);
        candidate.total_bytes += size_of_val(data);
        candidate.queue.push_back(Chunk {
            node: node.to_owned(),
            name: name.to_owned(),
            data: data.to_vec(),
        });
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

pub(crate) fn update(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<(Entity, &Mesh3d), With<Authored>>,
    gallery: Query<(Entity, &Mesh3d), With<GalleryObject>>,
) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        for handle in state.discarded.drain(..) {
            meshes.remove(handle.id());
        }
        let Some(mut candidate) = state.candidate.take() else {
            return;
        };
        if !staging::current(&candidate.token) {
            state
                .discarded
                .extend(candidate.uploaded.into_iter().map(|item| item.handle));
            return;
        }
        if let Some(chunk) = candidate.queue.pop_front() {
            let began = now();
            let bytes = size_of_val(chunk.data.as_slice());
            candidate.queued_bytes -= bytes;
            candidate.uploaded.push(upload(chunk, &mut meshes));
            state.last_ms = now() - began;
            state.max_ms = state.max_ms.max(state.last_ms);
            state.upload_frames += 1;
            state.max_frame_bytes = state.max_frame_bytes.max(bytes);
        }
        if candidate.finished
            && candidate.queue.is_empty()
            && candidate
                .uploaded
                .iter()
                .all(|item| mesh_ready(item.handle.id()))
            && staging::accept(&candidate.token)
        {
            for (entity, mesh) in old.iter().chain(gallery.iter()) {
                commands.entity(entity).despawn();
                meshes.remove(mesh.0.id());
            }
            state.parts.clear();
            for item in candidate.uploaded {
                state
                    .parts
                    .push((item.node.clone(), candidate.token.revision));
                commands.spawn((
                    Mesh3d(item.handle),
                    VeMaterial {
                        base_color: Color::srgb(0.68, 0.73, 0.87),
                        roughness: 0.5,
                        ..VeMaterial::default()
                    },
                    Transform::IDENTITY,
                    Interactable::selection_only(&item.bounds),
                    Label(item.name),
                    Authored {
                        node: item.node,
                        revision: candidate.token.revision,
                    },
                ));
            }
            state.committed = Some(candidate.token);
            return;
        }
        state.candidate = Some(candidate);
    });
}

fn upload(chunk: Chunk, meshes: &mut Assets<Mesh>) -> Uploaded {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut bounds = Bounds {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    for vertex in chunk.data.chunks_exact(6) {
        positions.push([vertex[0], vertex[1], vertex[2]]);
        normals.push([vertex[3], vertex[4], vertex[5]]);
        for (axis, value) in vertex.iter().take(3).enumerate() {
            bounds.min[axis] = bounds.min[axis].min(f64::from(*value));
            bounds.max[axis] = bounds.max[axis].max(f64::from(*value));
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    Uploaded {
        node: chunk.node,
        name: chunk.name,
        handle: meshes.add(mesh),
        bounds,
    }
}

pub(crate) fn status() -> String {
    STATE.with(|cell| {
        let state = cell.borrow();
        json!({"committed":state.committed,
        "queued_bytes":state.candidate.as_ref().map_or(0, |c| c.queued_bytes),
        "staging_meshes":state.candidate.as_ref().map_or(0, |c| c.uploaded.len()),
        "live_meshes":state.parts.len(), "selection_map":state.parts,
        "upload_frames":state.upload_frames,"max_frame_bytes":state.max_frame_bytes,
        "last_upload_ms":state.last_ms,"max_upload_ms":state.max_ms})
        .to_string()
    })
}

pub(crate) fn selections(query: &Query<(Entity, &Authored)>) -> serde_json::Value {
    query.iter().map(|(entity, item)| json!({"entity":entity.to_bits(),"node":item.node,"revision":item.revision})).collect()
}
