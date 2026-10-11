//! Per-frame plumbing: clearing window swap chains and submitting recorded command buffers.

use bevy_camera::ClearColor;
use bevy_ecs::system::{Query, Res, ResMut};
use bevy_render::render_resource::{
    CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor,
    StoreOp,
};
use bevy_render::renderer::{PendingCommandBuffers, RenderDevice, RenderQueue};
use bevy_render::view::ExtractedWindow;

/// Clears every window that has a swap chain texture to the [`ClearColor`].
///
/// With no cameras this is what puts a colour on the canvas, which proves the whole chain (surface,
/// device, queue, present) works. Once cameras exist, the camera driver clears the windows it
/// renders to and this stays as the fallback for uncovered windows.
pub(crate) fn clear_windows(
    windows: Query<&ExtractedWindow>,
    clear: Res<ClearColor>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let color = clear.0.to_linear();
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("ve_clear_windows"),
    });
    let mut cleared = false;
    for window in &windows {
        let Some(view) = window.swap_chain_texture_view.as_ref() else {
            continue;
        };
        // The pass only needs to exist: beginning it with `Clear` performs the clear.
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

/// Submits every command buffer recorded this frame (by future render systems) to the GPU queue.
pub(crate) fn submit_pending_command_buffers(
    mut pending: ResMut<PendingCommandBuffers>,
    queue: Res<RenderQueue>,
) {
    let mut buffers = pending.finish().peekable();
    if buffers.peek().is_some() {
        queue.submit(buffers);
    }
}
