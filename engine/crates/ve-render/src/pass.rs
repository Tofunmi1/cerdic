//! The forward main pass: draws every instance batch into the view's output texture.

use bevy_camera::ClearColor;
use bevy_ecs::prelude::*;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{PipelineCache, RenderPassDescriptor, StoreOp};
use bevy_render::renderer::{RenderContext, ViewQuery};
use bevy_render::view::{ViewDepthStencilTexture, ViewTarget};

use crate::pipeline::ForwardPipelines;
use crate::prepare::{InstanceBatches, ViewUniformSlot, ViewUniforms};

/// Clears the view and draws all batches. Skips quietly while pipelines are still compiling.
pub(crate) fn forward_pass(
    view: ViewQuery<(&ViewTarget, &ViewDepthStencilTexture, &ViewUniformSlot)>,
    batches: Res<InstanceBatches>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    clear: Res<ClearColor>,
    mut ctx: RenderContext,
) {
    let (target, depth, slot) = view.into_inner();
    let Some(color) = target.out_texture_color_attachment(Some(clear.0.to_linear())) else {
        return;
    };
    let Some(format) = target.out_texture_view_format() else {
        return;
    };
    let Some(view_bind_group) = view_uniforms.bind_group.as_ref() else {
        return;
    };

    let color_attachments = [Some(color)];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_forward_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let Some(instance_buffer) = batches.buffer.buffer() else {
        return;
    };
    for batch in &batches.batches {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        let Some(pipeline_id) = pipelines.get(&render_mesh.layout, format) else {
            continue;
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
            continue;
        };
        let Some(vertices) = allocator.mesh_vertex_slice(&batch.mesh) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, view_bind_group, &[slot.0]);
        pass.set_vertex_buffer(0, vertices.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        match &render_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                count,
                index_format,
            } => {
                let Some(indices) = allocator.mesh_index_slice(&batch.mesh) else {
                    continue;
                };
                pass.set_index_buffer(indices.buffer.slice(..), *index_format);
                let first = indices.range.start;
                let base_vertex = i32::try_from(vertices.range.start).unwrap_or(0);
                pass.draw_indexed(first..first + count, base_vertex, batch.instances.clone());
            }
            RenderMeshBufferInfo::NonIndexed => {
                let range = vertices.range.start..vertices.range.end;
                pass.draw(range, batch.instances.clone());
            }
        }
    }
}
