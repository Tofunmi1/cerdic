//! Gizmo projection and drag exports, keeping the root entry point small.

use wasm_bindgen::prelude::*;

use crate::gizmo;

/// Projects selection bounds into axis handle endpoints and three rotation rings for the viewer overlay.
#[wasm_bindgen]
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "flat coordinates keep the wasm-bindgen ABI scalar and stable"
)]
pub fn viewer_gizmo(
    min_x: f32,
    min_y: f32,
    min_z: f32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    width: f32,
    height: f32,
) -> String {
    gizmo::project_bounds(
        bevy_math::Vec3::new(min_x, min_y, min_z),
        bevy_math::Vec3::new(max_x, max_y, max_z),
        width,
        height,
        None,
    )
}

/// Projects selection bounds around an explicit world-space pivot.
#[wasm_bindgen]
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "flat coordinates keep the wasm-bindgen ABI scalar and stable"
)]
pub fn viewer_gizmo_at(
    min_x: f32,
    min_y: f32,
    min_z: f32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    pivot_x: f32,
    pivot_y: f32,
    pivot_z: f32,
    width: f32,
    height: f32,
) -> String {
    gizmo::project_bounds(
        bevy_math::Vec3::new(min_x, min_y, min_z),
        bevy_math::Vec3::new(max_x, max_y, max_z),
        width,
        height,
        Some(bevy_math::Vec3::new(pivot_x, pivot_y, pivot_z)),
    )
}

/// Begins a translate, rotate or scale gizmo drag against the last viewer camera.
#[wasm_bindgen]
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "flat coordinates keep the wasm-bindgen ABI scalar and stable"
)]
pub fn gizmo_begin(
    mode: &str,
    axis: u8,
    px: f32,
    py: f32,
    aspect: f32,
    x: f32,
    y: f32,
    z: f32,
) -> bool {
    gizmo::begin(mode, axis, px, py, aspect, bevy_math::Vec3::new(x, y, z))
}

/// Begins a profile-point drag constrained to the XY plane.
#[wasm_bindgen]
#[must_use]
pub fn gizmo_begin_profile(px: f32, py: f32, aspect: f32, x: f32, y: f32) -> bool {
    gizmo::begin_profile(px, py, aspect, bevy_math::Vec3::new(x, y, 0.0))
}

/// Begins a 3D control-point drag in the camera-facing plane through that point.
#[wasm_bindgen]
#[must_use]
pub fn gizmo_begin_camera_plane(px: f32, py: f32, aspect: f32, x: f32, y: f32, z: f32) -> bool {
    gizmo::begin_camera_plane(px, py, aspect, bevy_math::Vec3::new(x, y, z))
}

/// Updates the active gizmo drag and returns a typed JSON delta, or an empty string for a bad ray.
#[wasm_bindgen]
#[must_use]
pub fn gizmo_update(px: f32, py: f32, aspect: f32, grid: f32, angle: f32, scale: f32) -> String {
    gizmo::update(px, py, aspect, grid, angle, scale)
}

/// Ends the active gizmo drag without changing the document.
#[wasm_bindgen]
pub fn gizmo_cancel() {
    gizmo::cancel();
}
