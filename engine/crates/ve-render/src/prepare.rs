//! Per-frame GPU data: instance buffer, view uniforms and depth textures.

use std::ops::Range;

use bevy_asset::AssetId;
use bevy_camera::{Camera3d, Camera3dDepthLoadOp};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, UVec2};
use bevy_mesh::Mesh;
use bevy_render::camera::ExtractedCamera;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BufferUsages, DynamicUniformBuffer, PipelineCache, RawBufferVec,
    ShaderType, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::texture::TextureCache;
use bevy_render::view::{ExtractedView, Msaa, ViewDepthStencilTexture};
use bytemuck::{Pod, Zeroable};

use crate::extract::{ExtractedMesh, ExtractedMeshes};
use crate::pipeline::ForwardPipelines;

/// Depth format of the forward pass. Bevy projections are reverse-z, so depth clears to 0.
pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// Model matrix of one instance, laid out as four `vec4` columns (instance vertex attributes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct InstanceData {
    pub(crate) world_from_local: [[f32; 4]; 4],
}

/// A run of consecutive instances that share a mesh and can be drawn with one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Batch {
    pub(crate) mesh: AssetId<Mesh>,
    pub(crate) instances: Range<u32>,
}

/// The instance buffer and the draw batches over it.
#[derive(Resource)]
pub(crate) struct InstanceBatches {
    pub(crate) buffer: RawBufferVec<InstanceData>,
    pub(crate) batches: Vec<Batch>,
}

impl Default for InstanceBatches {
    fn default() -> Self {
        Self {
            buffer: RawBufferVec::new(BufferUsages::VERTEX),
            batches: Vec::new(),
        }
    }
}

/// Sorts instances by mesh and groups equal meshes into consecutive batches.
///
/// Returns the per-instance data in draw order and the batches over it. The sort is stable, so
/// instances of one mesh keep their extraction order.
pub(crate) fn group_into_batches(meshes: &[ExtractedMesh]) -> (Vec<InstanceData>, Vec<Batch>) {
    let mut sorted = meshes.to_vec();
    sorted.sort_by_key(|m| m.mesh);
    let mut data = Vec::with_capacity(sorted.len());
    let mut batches: Vec<Batch> = Vec::new();
    for mesh in &sorted {
        let index = u32::try_from(data.len()).unwrap_or(u32::MAX);
        data.push(InstanceData {
            world_from_local: mesh.world_from_local.to_cols_array_2d(),
        });
        match batches.last_mut() {
            Some(batch) if batch.mesh == mesh.mesh => batch.instances.end = index + 1,
            _ => batches.push(Batch {
                mesh: mesh.mesh,
                instances: index..index + 1,
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

#[cfg(test)]
mod tests {
    use bevy_asset::uuid::Uuid;

    use super::*;

    fn mesh_id(n: u128) -> AssetId<Mesh> {
        AssetId::Uuid {
            uuid: Uuid::from_u128(n),
        }
    }

    fn instance(mesh: u128, x: f32) -> ExtractedMesh {
        ExtractedMesh {
            mesh: mesh_id(mesh),
            world_from_local: Mat4::from_translation(bevy_math::Vec3::new(x, 0.0, 0.0)),
        }
    }

    #[test]
    fn no_instances_gives_no_batches() {
        let (data, batches) = group_into_batches(&[]);
        assert!(data.is_empty() && batches.is_empty());
    }

    #[test]
    fn one_batch_per_distinct_mesh_covering_all_instances() {
        let input = [
            instance(2, 0.0),
            instance(1, 1.0),
            instance(2, 2.0),
            instance(1, 3.0),
            instance(3, 4.0),
        ];
        let (data, batches) = group_into_batches(&input);
        assert_eq!(data.len(), 5);
        assert_eq!(batches.len(), 3);
        let covered: u32 = batches
            .iter()
            .map(|b| b.instances.end - b.instances.start)
            .sum();
        assert_eq!(covered, 5);
        // Batches are contiguous, in order, and start at zero.
        assert_eq!(batches[0].instances.start, 0);
        for pair in batches.windows(2) {
            assert_eq!(pair[0].instances.end, pair[1].instances.start);
        }
    }

    #[test]
    fn instance_data_follows_batch_order_and_keeps_extraction_order_within_a_mesh() {
        let input = [instance(2, 10.0), instance(1, 20.0), instance(2, 30.0)];
        let (data, batches) = group_into_batches(&input);
        // Mesh 1 sorts first with one instance; mesh 2 follows with its two in extraction order.
        assert_eq!(batches[0].mesh, mesh_id(1));
        assert_eq!(batches[0].instances, 0..1);
        assert_eq!(batches[1].instances, 1..3);
        let translation_x = |i: usize| data[i].world_from_local[3][0];
        assert!((translation_x(0) - 20.0).abs() < f32::EPSILON);
        assert!((translation_x(1) - 10.0).abs() < f32::EPSILON);
        assert!((translation_x(2) - 30.0).abs() < f32::EPSILON);
    }
}

/// Uniform data for one view.
#[derive(Clone, Copy, Debug, ShaderType)]
pub(crate) struct ViewUniform {
    pub(crate) clip_from_world: Mat4,
}

/// Offset of a view's data in the shared dynamic uniform buffer.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct ViewUniformSlot(pub(crate) u32);

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
    views: Query<(Entity, &ExtractedView)>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    uniforms.buffer.clear();
    for (entity, view) in &views {
        let clip_from_world = view
            .clip_from_world
            .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());
        let offset = uniforms.buffer.push(&ViewUniform { clip_from_world });
        commands.entity(entity).insert(ViewUniformSlot(offset));
    }
    uniforms.buffer.write_buffer(&device, &queue);
    uniforms.bind_group = uniforms.buffer.binding().map(|binding| {
        let layout = pipeline_cache.get_bind_group_layout(&pipelines.view_layout);
        device.create_bind_group(
            "ve_view_bind_group",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: binding,
            }],
        )
    });
}

/// Creates (or reuses, via the texture cache) a depth texture for every 3D camera.
pub(crate) fn prepare_depth_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedCamera, &Camera3d, &Msaa)>,
) {
    for (entity, camera, camera_3d, msaa) in &views {
        let Some(size) = camera.physical_target_size else {
            continue;
        };
        let UVec2 { x, y } = size;
        let texture = texture_cache.get(
            &device,
            TextureDescriptor {
                label: Some("ve_view_depth"),
                size: bevy_render::render_resource::Extent3d {
                    width: x,
                    height: y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: msaa.samples(),
                dimension: TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );
        let clear = match camera_3d.depth_load_op {
            Camera3dDepthLoadOp::Clear(value) => Some(value),
            Camera3dDepthLoadOp::Load => None,
        };
        commands
            .entity(entity)
            .insert(ViewDepthStencilTexture::new(texture, clear, None));
    }
}
