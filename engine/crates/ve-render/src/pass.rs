//! The forward main pass: draws every instance batch into the view's output texture.

use bevy_camera::ClearColor;
use bevy_color::LinearRgba;
use bevy_ecs::prelude::*;
use bevy_log::info_once;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    LoadOp, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
};
use bevy_render::renderer::{RenderContext, ViewQuery};
use bevy_render::view::{Msaa, ViewDepthStencilTexture, ViewTarget};

use crate::pipeline::ForwardPipelines;
use crate::prepare::{InstanceBatches, LightsGpu, ViewUniformSlot, ViewUniforms};
use crate::targets::ViewMsaaColor;

/// Chooses the pass's colour attachment on top of the view's `output` attachment.
///
/// With MSAA on, the pass renders into the view's multisampled texture and resolves into the output
/// (the multisampled samples are discarded afterwards, which is cheap on tiled GPUs). Without MSAA it
/// renders straight into the output.
///
/// `output` must come from [`ViewTarget::out_texture_color_attachment`] and nowhere else: that call
/// also records that the output was written, and Bevy only presents outputs marked that way. Building
/// an attachment from the raw output view skips the mark, the frame is never presented, and the
/// screen keeps showing the previous frame.
fn color_attachment<'a>(
    output: RenderPassColorAttachment<'a>,
    msaa_color: Option<&'a ViewMsaaColor>,
    samples: u32,
    clear: LinearRgba,
) -> RenderPassColorAttachment<'a> {
    match msaa_color {
        Some(msaa_color) if samples > 1 => RenderPassColorAttachment {
            view: &msaa_color.0.default_view,
            depth_slice: None,
            resolve_target: Some(output.view),
            ops: Operations {
                load: LoadOp::Clear(clear.into()),
                store: StoreOp::Discard,
            },
        },
        _ => output,
    }
}

/// Clears the view and draws all batches. Skips quietly while pipelines are still compiling.
pub(crate) fn forward_pass(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthStencilTexture,
        &ViewUniformSlot,
        &Msaa,
        Option<&ViewMsaaColor>,
    )>,
    batches: Res<InstanceBatches>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    lights: Res<LightsGpu>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    clear: Res<ClearColor>,
    mut ctx: RenderContext,
) {
    let (target, depth, slot, msaa, msaa_color) = view.into_inner();
    let clear_color = clear.0.to_linear();
    let (Some(output), Some(format)) = (
        target.out_texture_color_attachment(Some(clear_color)),
        target.out_texture_view_format(),
    ) else {
        info_once!("forward pass skipped: the view has no output texture");
        return;
    };
    let (Some(view_bind_group), Some(lights_bind_group)) = (
        view_uniforms.bind_group.as_ref(),
        lights.bind_group.as_ref(),
    ) else {
        info_once!("forward pass skipped: view or lights bind group is not ready");
        return;
    };
    let samples = msaa.samples();
    let color_attachments = [Some(color_attachment(
        output,
        msaa_color,
        samples,
        clear_color,
    ))];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_forward_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let Some(instance_buffer) = batches.buffer.buffer() else {
        info_once!("forward pass: no instance buffer yet (nothing to draw)");
        return;
    };
    for batch in &batches.batches {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            info_once!("forward pass: a mesh is not uploaded to the GPU yet");
            continue;
        };
        let Some(pipeline_id) = pipelines.get(&render_mesh.layout, format, samples) else {
            // Pipelines are queued one frame after a mesh first appears.
            info_once!("forward pass: no pipeline is queued for a mesh layout yet");
            continue;
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
            info_once!("forward pass: a pipeline is not ready (still compiling, or failed)");
            continue;
        };
        let Some(vertices) = allocator.mesh_vertex_slice(&batch.mesh) else {
            info_once!("forward pass: mesh vertices are not allocated yet");
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, view_bind_group, &[slot.0]);
        pass.set_bind_group(1, lights_bind_group, &[]);
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
                pass.draw(
                    vertices.range.start..vertices.range.end,
                    batch.instances.clone(),
                );
            }
        }
    }
}
