//! Per-frame plumbing: clearing uncovered window swap chains and submitting recorded command buffers.

use bevy_camera::ClearColor;
use bevy_ecs::entity::ContainsEntity;
use bevy_ecs::system::{Query, Res, ResMut};
use bevy_render::render_resource::{
    CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor,
    StoreOp,
};
use bevy_render::renderer::{PendingCommandBuffers, RenderDevice, RenderQueue};
use bevy_render::sync_world::MainEntity;
use bevy_render::view::ExtractedWindow;

use crate::schedule::CameraWindows;

/// Clears every window that has a swap chain texture but no camera drawing into it.
///
/// A window without a camera would otherwise show whatever the surface held before. Covered
/// windows are cleared by their camera's pass instead.
pub(crate) fn clear_windows(
    windows: Query<(MainEntity, &ExtractedWindow)>,
    covered: Res<CameraWindows>,
    clear: Res<ClearColor>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let color = clear.0.to_linear();
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("ve_clear_windows"),
    });
    let mut cleared = false;
    for (main, window) in &windows {
        if covered.0.contains(&main.entity()) {
            continue;
        }
        let Some(view) = window.swap_chain_texture_view.as_ref() else {
            continue;
        };
        // Beginning the pass with `Clear` performs the clear; nothing is drawn.
        let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("ve_clear_window"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(color.into()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        cleared = true;
    }
    if cleared {
        queue.submit([encoder.finish()]);
    }
}

/// Submits every command buffer recorded this frame by the camera passes to the GPU queue.
pub(crate) fn submit_pending_command_buffers(
    mut pending: ResMut<PendingCommandBuffers>,
    queue: Res<RenderQueue>,
) {
    let mut buffers = pending.finish().peekable();
    if buffers.peek().is_some() {
        queue.submit(buffers);
    }
}
