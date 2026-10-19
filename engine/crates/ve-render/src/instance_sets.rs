//! Instance sets: thousands of copies of one mesh (a forest, a crowd, a field of grass) kept as one component instead of one entity
//! each.
//!
//! The copies' instance data is built once into a retained GPU buffer, sorted along a Z-order curve and cut into chunks with a bounding
//! sphere each (`set_chunks.rs`). Every frame only the chunks are tested against the views (`set_visibility.rs`) and the visible ones
//! are drawn from the set's own buffer, so the work follows what is on screen and nothing is rebuilt for a scene that does not change.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy_asset::{AssetId, Handle};
use bevy_camera::visibility::InheritedVisibility;
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Vec3};
use bevy_mesh::Mesh;
use bevy_render::Extract;
use bevy_render::render_resource::{Buffer, BufferInitDescriptor, BufferUsages};
use bevy_render::renderer::RenderDevice;

use crate::extract::drawable;
use crate::material::VeMaterial;
use crate::prepare::{Batch, InstanceBatches, InstanceData, batch_flags};
use crate::set_chunks::{self, Chunk};
use crate::shadow_math::MAX_CASCADES;

/// One level of detail of an instance set: the mesh to draw while the copies are within `max_distance` metres of the camera. The
/// last level's distance is where the set stops being drawn (infinity draws it at any distance).
#[derive(Clone, Debug)]
pub struct SetMesh {
    /// The mesh of this level.
    pub mesh: Handle<Mesh>,
    /// How far from the camera this level is used, in metres.
    pub max_distance: f32,
}

/// Many copies of one mesh, drawn from a retained buffer with culling. All copies share the entity's [`VeMaterial`] (the copy
/// index still feeds the per-copy colour variation), so highlighting the entity highlights the set. The copies do not move: replace
/// the component to change them.
#[derive(Component, Clone, Debug)]
pub struct VeInstanceSet {
    /// Levels of detail, nearest first (at least one).
    pub meshes: Vec<SetMesh>,
    /// The copies' world matrices.
    pub transforms: Arc<[Mat4]>,
    /// A stable index per copy, parallel to `transforms`.
    pub indices: Arc<[u32]>,
    /// Centre of the mesh's bounds in its own space.
    pub bounds_centre: Vec3,
    /// Radius of a sphere around `bounds_centre` that holds the mesh.
    pub bounds_radius: f32,
}

impl VeInstanceSet {
    /// A set drawn with one mesh at any distance.
    #[must_use]
    pub fn new(
        mesh: Handle<Mesh>,
        transforms: Arc<[Mat4]>,
        indices: Arc<[u32]>,
        bounds: (Vec3, f32),
    ) -> Self {
        Self {
            meshes: vec![SetMesh {
                mesh,
                max_distance: f32::INFINITY,
            }],
            transforms,
            indices,
            bounds_centre: bounds.0,
            bounds_radius: bounds.1,
        }
    }
}

/// Every visible instance set this frame with its surface, copied from the main world.
#[derive(Resource, Default)]
pub(crate) struct ExtractedSets(pub(crate) Vec<(Entity, VeInstanceSet, VeMaterial)>);

/// Copies the visible instance sets across (the component is a few `Arc`s, so this costs nothing per copy).
pub(crate) fn extract_instance_sets(
    mut out: ResMut<ExtractedSets>,
    sets: Extract<
        Query<(
            Entity,
            &VeInstanceSet,
            Option<&VeMaterial>,
            Option<&InheritedVisibility>,
        )>,
    >,
) {
    out.0.clear();
    out.0.extend(
        sets.iter()
            .filter(|(_, _, _, visibility)| visibility.is_none_or(|v| v.get()))
            .map(|(entity, set, material, _)| {
                (entity, set.clone(), material.copied().unwrap_or_default())
            }),
    );
}

/// What decides whether a retained buffer must be rebuilt: the copies (by identity), the material and the bounds.
#[derive(Clone, PartialEq)]
struct Signature {
    transforms: (usize, usize),
    indices: usize,
    material: VeMaterial,
    bounds: [f32; 4],
}

impl Signature {
    fn of(set: &VeInstanceSet, material: &VeMaterial) -> Self {
        Self {
            transforms: (
                Arc::as_ptr(&set.transforms).cast::<u8>() as usize,
                set.transforms.len(),
            ),
            indices: Arc::as_ptr(&set.indices).cast::<u8>() as usize,
            material: *material,
            bounds: [
                set.bounds_centre.x,
                set.bounds_centre.y,
                set.bounds_centre.z,
                set.bounds_radius,
            ],
        }
    }
}

/// A set's retained buffer and the chunks over it.
pub(crate) struct Retained {
    signature: Signature,
    pub(crate) buffer: Buffer,
    pub(crate) chunks: Vec<Chunk>,
    /// Levels of detail, nearest first: the mesh and how far it is used.
    pub(crate) levels: Vec<(AssetId<Mesh>, f32)>,
    pub(crate) outlined: bool,
    pub(crate) cutout: bool,
    pub(crate) foliage: bool,
    /// Copies in the buffer.
    pub(crate) copies: u32,
}

/// Draw counts of the sets last frame, for telemetry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SetStats {
    /// Instance sets alive.
    pub sets: u32,
    /// Copies in all of them.
    pub copies: u32,
    /// Chunks in all of them.
    pub chunks: u32,
    /// Chunks drawn from the camera (the main pass).
    pub chunks_visible: u32,
    /// Copies in those chunks.
    pub copies_visible: u32,
}

/// Retained sets and what is visible of them this frame.
#[derive(Default)]
pub(crate) struct RetainedSets {
    pub(crate) sets: HashMap<Entity, Retained>,
    /// Chunk runs to draw from the camera.
    pub(crate) main: Vec<Batch>,
    /// Chunk runs to draw into each shadow cascade.
    pub(crate) cascades: [Vec<Batch>; MAX_CASCADES],
    /// One empty batch per level of every set, so pipelines are ready before a level first comes into view.
    pub(crate) queue: Vec<Batch>,
    pub(crate) stats: SetStats,
}

impl InstanceBatches {
    /// Every batch a pipeline might be needed for: this frame's entity batches and every level of every set.
    pub(crate) fn queue_batches(&self) -> impl Iterator<Item = &Batch> {
        self.batches.iter().chain(&self.sets.queue)
    }

    /// The batches the camera draws (and the occlusion depth pass): entity batches and the visible chunk runs of the sets.
    pub(crate) fn main_batches(&self) -> impl Iterator<Item = &Batch> {
        self.batches.iter().chain(&self.sets.main)
    }

    /// The batches shadow cascade `index` draws.
    pub(crate) fn cascade_batches(&self, index: usize) -> impl Iterator<Item = &Batch> {
        self.batches
            .iter()
            .chain(self.sets.cascades.get(index).into_iter().flatten())
    }

    /// The buffer holding `batch`'s instances: this frame's buffer, or the retained buffer of its set. `None` while that is not
    /// ready, in which case the batch is skipped.
    pub(crate) fn instance_buffer(&self, batch: &Batch) -> Option<&Buffer> {
        match batch.set {
            Some(entity) => self.sets.sets.get(&entity).map(|set| &set.buffer),
            None => self.buffer.buffer(),
        }
    }
}

/// The instance data of every drawable copy, sorted into chunks, uploaded once.
fn build(device: &RenderDevice, set: &VeInstanceSet, material: &VeMaterial) -> Option<Retained> {
    let usable: Vec<usize> = (0..set.transforms.len())
        .filter(|&i| drawable(&set.transforms[i]))
        .collect();
    if usable.is_empty() || set.meshes.is_empty() {
        return None;
    }
    let spheres: Vec<(Vec3, f32)> = usable
        .iter()
        .map(|&i| {
            let matrix = &set.transforms[i];
            let scale = matrix
                .x_axis
                .truncate()
                .length()
                .max(matrix.y_axis.truncate().length())
                .max(matrix.z_axis.truncate().length());
            (
                matrix.transform_point3(set.bounds_centre),
                set.bounds_radius * scale,
            )
        })
        .collect();
    let (order, chunks) = set_chunks::build(&spheres, set_chunks::chunk_size_for(spheres.len()));
    let data: Vec<InstanceData> = order
        .iter()
        .map(|&position| {
            let i = usable[position as usize];
            let index = set
                .indices
                .get(i)
                .copied()
                .unwrap_or_else(|| u32::try_from(i).unwrap_or(0));
            InstanceData::new(&set.transforms[i], &material.pack_for_instance(index))
        })
        .collect();
    let (outlined, cutout, foliage) = batch_flags(&material.pack_for_instance(0));
    let buffer = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("ve_instance_set"),
        contents: bytemuck::cast_slice(&data),
        usage: BufferUsages::VERTEX,
    });
    Some(Retained {
        signature: Signature::of(set, material),
        buffer,
        chunks,
        levels: Vec::new(),
        outlined,
        cutout,
        foliage,
        copies: u32::try_from(data.len()).unwrap_or(u32::MAX),
    })
}

/// Builds the retained buffer of every new or changed set, forgets the sets that are gone, and lists a pipeline-warming batch for each
/// level of each set.
pub(crate) fn prepare_sets(
    extracted: Res<ExtractedSets>,
    mut batches: ResMut<InstanceBatches>,
    device: Res<RenderDevice>,
) {
    let sets = &mut batches.sets;
    let alive: HashSet<Entity> = extracted.0.iter().map(|(entity, _, _)| *entity).collect();
    sets.sets.retain(|entity, _| alive.contains(entity));
    for (entity, set, material) in &extracted.0 {
        let current = sets
            .sets
            .get(entity)
            .is_some_and(|retained| retained.signature == Signature::of(set, material));
        if !current {
            // A set with nothing drawable has no buffer, and so draws nothing.
            if let Some(retained) = build(&device, set, material) {
                sets.sets.insert(*entity, retained);
            } else {
                sets.sets.remove(entity);
                continue;
            }
        }
        // The levels follow the component every frame: they are cheap, and a level's mesh may only now have loaded.
        if let Some(retained) = sets.sets.get_mut(entity) {
            retained.levels = set
                .meshes
                .iter()
                .map(|level| (level.mesh.id(), level.max_distance))
                .collect();
        }
    }
    sets.queue.clear();
    for retained in sets.sets.values() {
        for (mesh, _) in &retained.levels {
            sets.queue.push(Batch {
                mesh: *mesh,
                instances: 0..0,
                transparent: false,
                center: Vec3::ZERO,
                sort_key: 0,
                outlined: retained.outlined,
                cutout: retained.cutout,
                foliage: retained.foliage,
                set: None,
            });
        }
    }
}
