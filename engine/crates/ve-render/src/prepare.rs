//! Per-frame GPU data: instance buffer, view uniforms and the lights uniform.

use std::ops::Range;

use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Vec3, Vec4};
use bevy_mesh::Mesh;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindingResource, BufferUsages, DynamicUniformBuffer, PipelineCache,
    RawBufferVec, ShaderType, UniformBuffer,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::view::ExtractedView;
use bytemuck::{Pod, Zeroable};

use crate::extract::{ExtractedMesh, ExtractedMeshes};
use crate::light::{ExtractedLights, LightsUniform, pack_lights};
use crate::pipeline::ForwardPipelines;
use crate::quality::RenderQuality;
use crate::shadow::{ShadowMap, ShadowState};
use crate::stereo::StereoViews;

/// Per-instance vertex data: the model matrix (four `vec4` columns) and the material.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct InstanceData {
    pub(crate) world_from_local: [[f32; 4]; 4],
    pub(crate) base_color: [f32; 4],
    pub(crate) params: [f32; 4],
    pub(crate) emissive: [f32; 4],
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
/// instances of one mesh keep their extraction order. Materials ride along as instance data, so
/// only the mesh decides which instances can share a draw call.
pub(crate) fn group_into_batches(meshes: &[ExtractedMesh]) -> (Vec<InstanceData>, Vec<Batch>) {
    let mut sorted = meshes.to_vec();
    sorted.sort_by_key(|m| m.mesh);
    let mut data = Vec::with_capacity(sorted.len());
    let mut batches: Vec<Batch> = Vec::new();
    for mesh in &sorted {
        let index = u32::try_from(data.len()).unwrap_or(u32::MAX);
        data.push(InstanceData {
            world_from_local: mesh.world_from_local.to_cols_array_2d(),
            base_color: mesh.material.base_color,
            params: mesh.material.params,
            emissive: mesh.material.emissive,
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

/// Uniform data for one view.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(crate) struct ViewUniform {
    pub(crate) clip_from_world: Mat4,
    /// xyz: world position of the eye.
    pub(crate) camera_position: Vec4,
    /// x: exposure multiplier applied before tonemapping.
    pub(crate) exposure: Vec4,
}

impl ViewUniform {
    /// The uniform for an eye or camera at `world_from_view` with projection `clip_from_view`.
    pub(crate) fn new(world_from_view: Mat4, clip_from_view: Mat4) -> Self {
        Self {
            clip_from_world: clip_from_view * world_from_view.inverse(),
            camera_position: world_from_view.w_axis.truncate().extend(1.0),
            exposure: Vec4::new(1.0, 0.0, 0.0, 0.0),
        }
    }
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
                .map(|eye| ViewSlot {
                    offset: uniforms
                        .buffer
                        .push(&ViewUniform::new(eye.world_from_view, eye.clip_from_view)),
                    viewport: Some(eye.viewport),
                })
                .collect()
        } else {
            let world_from_view = view.world_from_view.to_matrix();
            let mut uniform = ViewUniform::new(world_from_view, view.clip_from_view);
            if let Some(clip_from_world) = view.clip_from_world {
                uniform.clip_from_world = clip_from_world;
            }
            vec![ViewSlot {
                offset: uniforms.buffer.push(&uniform),
                viewport: None,
            }]
        };
        commands.entity(entity).insert(ViewSlots(slots));
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

/// The lights uniform for this frame plus its bind group.
#[derive(Resource, Default)]
pub(crate) struct LightsGpu {
    pub(crate) buffer: UniformBuffer<LightsUniform>,
    pub(crate) bind_group: Option<BindGroup>,
}

/// Packs the extracted lights (capped by the quality tier) and rebuilds the lights bind group.
pub(crate) fn prepare_lights(
    lights: Res<ExtractedLights>,
    quality: Res<RenderQuality>,
    views: Query<&ExtractedView>,
    shadow: Res<ShadowState>,
    shadow_map: Res<ShadowMap>,
    mut gpu: ResMut<LightsGpu>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let viewer = views
        .iter()
        .next()
        .map_or(Vec3::ZERO, |view| view.world_from_view.translation());
    gpu.buffer.set(pack_lights(
        &lights,
        viewer,
        quality.max_point_lights,
        &shadow.params,
    ));
    gpu.buffer.write_buffer(&device, &queue);
    let (Some(binding), Some(map_view), Some(map_sampler)) = (
        gpu.buffer.binding(),
        shadow_map.sampled_view(),
        shadow_map.sampler(),
    ) else {
        gpu.bind_group = None;
        return;
    };
    let layout = pipeline_cache.get_bind_group_layout(&pipelines.lights_layout);
    gpu.bind_group = Some(device.create_bind_group(
        "ve_lights_bind_group",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: binding,
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(map_view),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(map_sampler),
            },
        ],
    ));
}

#[cfg(test)]
mod tests {
    use bevy_asset::uuid::Uuid;

    use super::*;
    use crate::material::VeMaterial;

    fn mesh_id(n: u128) -> AssetId<Mesh> {
        AssetId::Uuid {
            uuid: Uuid::from_u128(n),
        }
    }

    fn instance(mesh: u128, x: f32) -> ExtractedMesh {
        ExtractedMesh {
            mesh: mesh_id(mesh),
            world_from_local: Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
            material: VeMaterial::default().pack(),
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

    #[test]
    fn view_uniform_inverts_the_pose_and_records_the_eye_position() {
        // Any invertible matrix works as a stand-in projection for checking the pose handling.
        let projection = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let pose = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let uniform = ViewUniform::new(pose, projection);
        // A point at the eye's own position projects like the view-space origin: onto the axis.
        let at_eye = uniform.clip_from_world * Vec3::new(1.0, 2.0, 3.0).extend(1.0);
        let at_origin = projection * Vec3::ZERO.extend(1.0);
        assert!((at_eye - at_origin).abs().max_element() < 1e-5);
        assert!((uniform.camera_position.truncate() - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-6);
        assert!((uniform.camera_position.w - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn different_materials_on_one_mesh_share_a_batch() {
        let mut shiny = instance(1, 0.0);
        shiny.material = VeMaterial {
            metallic: 1.0,
            roughness: 0.1,
            ..VeMaterial::default()
        }
        .pack();
        let (data, batches) = group_into_batches(&[shiny, instance(1, 1.0)]);
        assert_eq!(batches.len(), 1);
        assert!((data[0].params[0] - 1.0).abs() < f32::EPSILON);
        assert!(data[1].params[0].abs() < f32::EPSILON);
    }
}
