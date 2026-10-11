//! The forward main pass: draws every instance batch into the view's output texture.

use std::ops::Range;

use bevy_camera::ClearColor;
use bevy_color::LinearRgba;
use bevy_ecs::prelude::*;
use bevy_log::info_once;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_phase::TrackedRenderPass;
use bevy_render::render_resource::{
    BindGroup, Buffer, LoadOp, Operations, PipelineCache, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, TextureFormat, WgpuTextureView,
};
use bevy_render::renderer::{RenderContext, ViewQuery};
use bevy_render::view::{Msaa, ViewDepthStencilTexture, ViewTarget};

use crate::pipeline::ForwardPipelines;
use crate::prepare::{Batch, InstanceBatches, LightsGpu, ViewSlot, ViewSlots, ViewUniforms};
use crate::targets::{ViewMsaaColor, ViewOffscreen, needs_blit, render_format};

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
    destination: Destination<'a>,
    msaa_color: Option<&'a ViewMsaaColor>,
    samples: u32,
    clear: LinearRgba,
) -> RenderPassColorAttachment<'a> {
    let (final_view, direct) = match destination {
        Destination::Output(output) => (output.view, Some(output)),
        Destination::Offscreen(view) => (view, None),
    };
    match msaa_color {
        Some(msaa_color) if samples > 1 => RenderPassColorAttachment {
            view: &msaa_color.0.default_view,
            depth_slice: None,
            resolve_target: Some(final_view),
            ops: Operations {
                load: LoadOp::Clear(clear.into()),
                store: StoreOp::Discard,
            },
        },
        _ => direct.unwrap_or(RenderPassColorAttachment {
            view: final_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(clear.into()),
                store: StoreOp::Store,
            },
        }),
    }
}

/// Where the forward pass's colour ends up.
enum Destination<'a> {
    /// The view's output attachment (marked as written; see above).
    Output(RenderPassColorAttachment<'a>),
    /// The view's offscreen texture; the present pass copies it to the output afterwards.
    Offscreen(&'a WgpuTextureView),
}

/// What to draw for one batch, independent of which view it is drawn from.
enum DrawCall {
    Indexed {
        indices: Range<u32>,
        base_vertex: i32,
    },
    Direct {
        vertices: Range<u32>,
    },
}

impl DrawCall {
    fn issue(&self, pass: &mut TrackedRenderPass<'_>, instances: Range<u32>) {
        match self {
            Self::Indexed {
                indices,
                base_vertex,
            } => pass.draw_indexed(indices.clone(), *base_vertex, instances),
            Self::Direct { vertices } => pass.draw(vertices.clone(), instances),
        }
    }
}

/// Points the pass at one view slot: its uniform, and (for stereo) its part of the target.
fn bind_slot<'a>(pass: &mut TrackedRenderPass<'a>, bind_group: &'a BindGroup, slot: &ViewSlot) {
    pass.set_bind_group(0, bind_group, &[slot.offset]);
    if let Some([x, y, width, height]) = slot.viewport {
        #[expect(
            clippy::cast_precision_loss,
            reason = "viewport coordinates are far below 2^24"
        )]
        pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
        // No scissor on purpose: the viewport already confines rendering, and on GL a leftover
        // scissor rectangle also limits the MSAA resolve blit, so only the last eye would resolve.
    }
}

/// Clears the view and draws all batches from every slot of the view (one for a normal camera, two
/// for stereo). Skips quietly while pipelines are still compiling.
pub(crate) fn forward_pass(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthStencilTexture,
        &ViewSlots,
        &Msaa,
        Option<&ViewMsaaColor>,
        Option<&ViewOffscreen>,
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
    let (target, depth, slots, msaa, msaa_color, offscreen) = view.into_inner();
    let clear_color = clear.0.to_linear();
    let Some(output_format) = target.out_texture_view_format() else {
        info_once!("forward pass skipped: the view has no output texture");
        return;
    };
    let format = render_format(output_format);
    // Views that need the present pass render offscreen; the present pass takes the output
    // attachment (and so marks it written). Others take it here.
    let destination = if needs_blit(output_format) {
        let Some(offscreen) = offscreen else {
            info_once!("forward pass skipped: the offscreen target is not ready");
            return;
        };
        Destination::Offscreen(&offscreen.0.default_view)
    } else {
        let Some(output) = target.out_texture_color_attachment(Some(clear_color)) else {
            return;
        };
        Destination::Output(output)
    };
    let (Some(view_bind_group), Some(lights_bind_group), Some(instance_buffer)) = (
        view_uniforms.bind_group.as_ref(),
        lights.bind_group.as_ref(),
        batches.buffer.buffer(),
    ) else {
        info_once!("forward pass skipped: view, lights or instance data is not ready");
        return;
    };
    let samples = msaa.samples();
    let color_attachments = [Some(color_attachment(
        destination,
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
    pass.set_bind_group(1, lights_bind_group, &[]);

    let gpu = BatchGpu {
        pipelines: &pipelines,
        pipeline_cache: &pipeline_cache,
        render_meshes: &render_meshes,
        allocator: &allocator,
        view_bind_group,
        instance_buffer,
        format,
        samples,
    };
    for batch in &batches.batches {
        draw_batch(&mut pass, &gpu, batch, slots);
    }
}

/// Everything `draw_batch` reads that is the same for all batches of a view.
struct BatchGpu<'a> {
    pipelines: &'a ForwardPipelines,
    pipeline_cache: &'a PipelineCache,
    render_meshes: &'a RenderAssets<RenderMesh>,
    allocator: &'a MeshAllocator,
    view_bind_group: &'a BindGroup,
    instance_buffer: &'a Buffer,
    format: TextureFormat,
    samples: u32,
}

/// Draws one instance batch from every slot of the view.
fn draw_batch<'a>(
    pass: &mut TrackedRenderPass<'a>,
    gpu: &BatchGpu<'a>,
    batch: &Batch,
    slots: &ViewSlots,
) {
    let (Some(render_mesh), Some(vertices)) = (
        gpu.render_meshes.get(batch.mesh),
        gpu.allocator.mesh_vertex_slice(&batch.mesh),
    ) else {
        info_once!("forward pass: a mesh is not uploaded to the GPU yet");
        return;
    };
    // Pipelines are queued one frame after a mesh first appears.
    let Some(pipeline) = gpu
        .pipelines
        .get(&render_mesh.layout, gpu.format, gpu.samples)
        .and_then(|id| gpu.pipeline_cache.get_render_pipeline(id))
    else {
        info_once!("forward pass: no ready pipeline for a mesh layout yet");
        return;
    };
    pass.set_render_pipeline(pipeline);
    pass.set_vertex_buffer(0, vertices.buffer.slice(..));
    pass.set_vertex_buffer(1, gpu.instance_buffer.slice(..));
    let call = match &render_mesh.buffer_info {
        RenderMeshBufferInfo::Indexed {
            count,
            index_format,
        } => {
            let Some(indices) = gpu.allocator.mesh_index_slice(&batch.mesh) else {
                return;
            };
            pass.set_index_buffer(indices.buffer.slice(..), *index_format);
            DrawCall::Indexed {
                indices: indices.range.start..indices.range.start + count,
                base_vertex: i32::try_from(vertices.range.start).unwrap_or(0),
            }
        }
        RenderMeshBufferInfo::NonIndexed => DrawCall::Direct {
            vertices: vertices.range.start..vertices.range.end,
        },
    };
    for slot in &slots.0 {
        bind_slot(pass, gpu.view_bind_group, slot);
        call.issue(pass, batch.instances.clone());
    }
}
