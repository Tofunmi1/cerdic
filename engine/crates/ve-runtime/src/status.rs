//! Engine status for the page: written by a system each frame, read through `status()`.

use std::cell::RefCell;

use bevy_ecs::system::{Local, Res};
use bevy_render::renderer::{RenderAdapterInfo, RenderDevice};
use serde_json::json;

thread_local! {
    static STATUS: RefCell<String> = RefCell::new(json!({ "frames": 0 }).to_string());
    static ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Records an error so the page can see it in `status()`.
pub(crate) fn report_error(message: String) {
    ERRORS.with(|errors| errors.borrow_mut().push(message));
}

/// Records adapter information, key limits and a frame counter.
pub(crate) fn publish(
    mut frames: Local<u64>,
    adapter: Option<Res<RenderAdapterInfo>>,
    device: Option<Res<RenderDevice>>,
) {
    *frames += 1;
    let errors = ERRORS.with(|errors| errors.borrow().clone());
    let mut value = json!({ "frames": *frames, "errors": errors });
    if let Some(info) = adapter {
        value["adapter"] = json!(info.name);
        value["backend"] = json!(format!("{:?}", info.backend));
    }
    if let Some(device) = device {
        let limits = device.limits();
        value["limits"] = json!({
            "max_texture_dimension_2d": limits.max_texture_dimension_2d,
            "max_uniform_buffer_binding_size": limits.max_uniform_buffer_binding_size,
            "max_bind_groups": limits.max_bind_groups,
            "max_vertex_buffers": limits.max_vertex_buffers,
            "max_color_attachments": limits.max_color_attachments,
        });
    }
    STATUS.with(|s| *s.borrow_mut() = value.to_string());
}

/// The most recent status JSON.
pub(crate) fn snapshot() -> String {
    STATUS.with(|s| s.borrow().clone())
}
