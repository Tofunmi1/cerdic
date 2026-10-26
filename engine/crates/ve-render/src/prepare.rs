//! Per-frame GPU data: instance buffer, view uniforms and the lights uniform.

use std::ops::Range;

use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Vec4};
use bevy_mesh::Mesh;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindingResource, BufferUsages, DynamicUniformBuffer, PipelineCache,
    RawBufferVec, ShaderType,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::view::ExtractedView;
use bytemuck::{Pod, Zeroable};

use crate::extract::{ExtractedMesh, ExtractedMeshes};
use crate::grade::Grading;
use crate::material::PackedMaterial;
use crate::pipeline::ForwardPipelines;
use crate::quality::RenderQuality;
use crate::stereo::StereoViews;

#[path = "prepare/lights_gpu.rs"]
mod lights_gpu;
pub(crate) use lights_gpu::{LightsGpu, prepare_lights};
#[path = "prepare/view_uniform.rs"]
mod view_uniform;
pub(crate) use view_uniform::view_clip_matrices;

/// Per-instance vertex data: the model matrix (four `vec4` columns) and the material.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct InstanceData {
    pub(crate) world_from_local: [[f32; 4]; 4],
    pub(crate) base_color: [f32; 4],
    pub(crate) params: [f32; 4],
    pub(crate) emissive: [f32; 4],
    pub(crate) maps: [f32; 4],
    pub(crate) toon_a: [f32; 4],
    pub(crate) toon_b: [f32; 4],
    pub(crate) toon_c: [f32; 4],
    pub(crate) foliage: [f32; 4],
}

/// A run of consecutive instances that share a mesh and can be drawn with one call.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag chooses a different extra pass, and they are set independently"
)]
pub(crate) struct Batch {
    pub(crate) grass: Option<crate::FoliageStyle>,
    pub(crate) water: Option<crate::SurfaceWater>,
    pub(crate) terrain: Option<crate::SurfaceTerrain>,
    pub(crate) mesh: AssetId<Mesh>,
    pub(crate) instances: Range<u32>,
    pub(crate) transparent: bool,
    pub(crate) center: bevy_math::Vec3,
    pub(crate) sort_key: u64,
    /// Whether any instance of the batch has an outline, so the outline pass draws it.
    pub(crate) outlined: bool,
    /// Whether any instance is cut out by its albedo alpha, so the shadow and depth passes must test it too.
    pub(crate) cutout: bool,
    /// Whether any instance has foliage (a cutout or wind), so the batch is drawn two-sided.
    pub(crate) foliage: bool,
    /// The instance set whose retained buffer `instances` indexes, or `None` for this frame's own buffer.
    pub(crate) set: Option<Entity>,
}

impl InstanceData {
    /// The instance data of one copy: its matrix and its packed material.
    pub(crate) fn new(world_from_local: &Mat4, material: &PackedMaterial) -> Self {
        Self {
            world_from_local: world_from_local.to_cols_array_2d(),
            base_color: material.base_color,
            params: material.params,
            emissive: material.emissive,
            maps: material.maps,
            toon_a: material.toon_a,
            toon_b: material.toon_b,
            toon_c: material.toon_c,
            foliage: material.foliage,
        }
    }
}

/// Whether a material has an outline, a cutout, foliage (a cutout, sway or flutter): what decides which extra passes draw a batch.
pub(crate) fn batch_flags(material: &PackedMaterial) -> (bool, bool, bool) {
    (
        material.toon_c[3] > 0.0,
        material.foliage[0] > 0.0,
        material.grass.is_some() || material.foliage[..3].iter().any(|&lane| lane > 0.0),
    )
}

/// The instance buffer and the draw batches over it, and the retained instance sets (see `instance_sets.rs`).
#[derive(Resource)]
pub(crate) struct InstanceBatches {
    pub(crate) buffer: RawBufferVec<InstanceData>,
    pub(crate) batches: Vec<Batch>,
    pub(crate) sets: crate::instance_sets::RetainedSets,
}

impl Default for InstanceBatches {
    fn default() -> Self {
        Self {
            buffer: RawBufferVec::new(BufferUsages::VERTEX),
            batches: Vec::new(),
            sets: crate::instance_sets::RetainedSets::default(),
        }
    }
}

/// Sorts instances by mesh (stably) and groups equal meshes into batches; materials ride along as
/// instance data; water also groups by the look whose wave uniform is bound. Returns data in draw order.
pub(crate) fn group_into_batches(meshes: &[ExtractedMesh]) -> (Vec<InstanceData>, Vec<Batch>) {
    let mut sorted = meshes.to_vec();
    sorted.sort_by(|a, b| {
        a.transparent
            .cmp(&b.transparent)
            .then_with(|| a.mesh.cmp(&b.mesh))
            .then_with(|| {
                if a.transparent {
                    a.sort_key.cmp(&b.sort_key)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
    });
    let mut data = Vec::with_capacity(sorted.len());
    let mut batches: Vec<Batch> = Vec::new();
    for mesh in &sorted {
        let index = u32::try_from(data.len()).unwrap_or(u32::MAX);
        data.push(InstanceData::new(&mesh.world_from_local, &mesh.material));
        let (outlined, cutout, foliage) = batch_flags(&mesh.material);
        match batches.last_mut() {
            Some(batch)
                if !batch.transparent
                    && !mesh.transparent
                    && batch.mesh == mesh.mesh
                    && batch.water == mesh.material.water
                    && batch.terrain == mesh.material.terrain
                    && batch.grass == mesh.material.grass =>
            {
                batch.instances.end = index + 1;
                batch.outlined |= outlined;
                batch.cutout |= cutout;
                batch.foliage |= foliage;
            }
            _ => batches.push(Batch {
                grass: mesh.material.grass,
                water: mesh.material.water,
                terrain: mesh.material.terrain,
                mesh: mesh.mesh,
                instances: index..index + 1,
                transparent: mesh.transparent,
                center: mesh.center,
                sort_key: mesh.sort_key,
                outlined,
                cutout,
                foliage,
                set: None,
            }),
        }
    }
    (data, batches)
}

/// Uploads this frame's instance data and batches.
pub(crate) fn prepare_instances(
    meshes: Res<ExtractedMeshes>,
    mut instances: ResMut<InstanceBatches>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (data, batches) = group_into_batches(&meshes.0);
    instances.buffer.clear();
    for item in data {
        instances.buffer.push(item);
    }
    instances.batches = batches;
    instances.buffer.write_buffer(&device, &queue);
}

/// Uniform data for one view.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(crate) struct ViewUniform {
    pub(crate) clip_from_world: Mat4,
    /// xyz: world position of the eye.
    pub(crate) camera_position: Vec4,
    /// x: exposure multiplier applied before tonemapping.
    pub(crate) exposure: Vec4,
    /// x: contrast, y: saturation (applied after tonemapping), w: grass lighting enabled.
    pub(crate) grading: Vec4,
    /// rgb: per-channel tint multiplier; w unused.
    pub(crate) tint: Vec4,
    /// x: specular AA, y: SSAO, z: bloom, w: stochastic sampling permitted.
    pub(crate) effects: Vec4,
    pub(crate) world_from_clip: Mat4,
    pub(crate) viewport: Vec4,
    pub(crate) horizon: Vec4,
    pub(crate) zenith: Vec4,
    pub(crate) ground: Vec4,
    pub(crate) sun: Vec4,
    pub(crate) cloud: Vec4,
    /// x, y: unit wind direction on the ground plane (x, z), z: strength, w: seconds on the animation clock (see `wind.rs`).
    pub(crate) wind: Vec4,
}

/// One drawing of the scene from a viewpoint: where its uniform lives in the shared dynamic buffer
/// and which part of the target it covers (`None` means the camera's whole target).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ViewSlot {
    pub(crate) offset: u32,
    pub(crate) viewport: Option<[u32; 4]>,
}

/// The slots of a view: one for a normal camera, two (left and right eye) for a stereo camera.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewSlots(pub(crate) Vec<ViewSlot>);

/// All view uniforms for this frame plus the bind group over them.
#[derive(Resource, Default)]
pub(crate) struct ViewUniforms {
    pub(crate) buffer: DynamicUniformBuffer<ViewUniform>,
    pub(crate) bind_group: Option<BindGroup>,
}

/// Writes every view's matrices into the dynamic uniform buffer and rebuilds the bind group.
pub(crate) fn prepare_view_uniforms(
    mut commands: Commands,
    mut uniforms: ResMut<ViewUniforms>,
    views: Query<(Entity, &ExtractedView, Option<&StereoViews>)>,
    grading: Res<Grading>,
    quality: Res<RenderQuality>,
    sky: Res<crate::SkyBackground>,
    delta: Res<crate::exposure::FrameDelta>,
    exposure: Res<crate::exposure::ExposureState>,
    wind: Res<crate::WindField>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    uniforms.buffer.clear();
    for (entity, view, stereo) in &views {
        let slots = if let Some(stereo) = stereo {
            stereo
                .eyes
                .iter()
                .map(|eye| {
                    let mut uniform = ViewUniform::new(eye.world_from_view, eye.clip_from_view);
                    uniform.set_grading(&grading);
                    uniform.set_quality(&quality);
                    uniform.set_auto_exposure(&quality, delta.0);
                    uniform.set_sky(&sky, eye.viewport);
                    uniform.set_wind(&wind);
                    ViewSlot {
                        offset: uniforms.buffer.push(&uniform),
                        viewport: Some(eye.viewport),
                    }
                })
                .collect()
        } else {
            let world_from_view = view.world_from_view.to_matrix();
            let mut uniform = ViewUniform::new(world_from_view, view.clip_from_view);
            if let Some(clip_from_world) = view.clip_from_world {
                uniform.clip_from_world = clip_from_world;
            }
            uniform.set_grading(&grading);
            uniform.set_quality(&quality);
            uniform.set_auto_exposure(&quality, delta.0);
            uniform.set_sky(&sky, view.viewport.to_array());
            uniform.set_wind(&wind);
            vec![ViewSlot {
                offset: uniforms.buffer.push(&uniform),
                viewport: None,
            }]
        };
        commands.entity(entity).insert(ViewSlots(slots));
    }
    uniforms.buffer.write_buffer(&device, &queue);
    // The bind group carries the view uniform and the exposure now in use (a 1 by 1 float texture), for every pass.
    uniforms.bind_group =
        uniforms
            .buffer
            .binding()
            .zip(exposure.current_view())
            .map(|(binding, exposure_view)| {
                let layout = pipeline_cache.get_bind_group_layout(&pipelines.view_layout);
                device.create_bind_group(
                    "ve_view_bind_group",
                    &layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: binding,
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(exposure_view),
                        },
                    ],
                )
            });
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
