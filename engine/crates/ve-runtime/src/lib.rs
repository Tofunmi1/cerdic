//! The runtime entry point called from the web page.
//!
//! `start(canvas_id, scene)` assembles the engine's plugins into a Bevy app that renders into the
//! canvas. `status()` reports what the page needs to verify the run (adapter, limits, frame count, XR
//! session). `enter_xr` and `exit_xr` move the running app into and out of a headset session.
#![cfg(target_arch = "wasm32")]

mod animation;
mod bake_exports;
pub use bake_exports::bake_maps;
mod app;
mod audio_exports;
pub use audio_exports::{audio_music, audio_sfx, fx_explode, fx_live_particles};
mod behavior;
mod bench_scene;
mod document;
pub mod document_exports;
mod effects;
mod fx;
mod fx_bursts;
mod gallery;
mod game;
mod game_camera;
mod game_ground;
mod game_hands;
mod game_models;
mod game_nav;
mod game_view;
mod gizmo;
mod gizmo_exports;
mod hud;
mod locomotion;
pub use gizmo_exports::{
    gizmo_begin, gizmo_begin_camera_plane, gizmo_begin_profile, gizmo_cancel, gizmo_update,
    viewer_gizmo, viewer_gizmo_at,
};
pub mod grading;
mod instance_exports;
mod instance_probe;
mod interact;
mod measure_exports;
pub use measure_exports::measure_mesh;
mod survey_exports;
pub use survey_exports::survey_plan;
mod water_sim;
mod world_api;
mod world_api_game;
pub use game::game_ui_frame;
pub use world_api::{world_call, world_events_since, world_take};
mod reflect_spike;
pub use reflect_spike::{
    reflect_spike_direct, reflect_spike_get, reflect_spike_set, reflect_spike_setup,
};
mod mock_hmd;
mod perf;
mod projection;
pub use instance_exports::{
    animation_set_bool, animation_set_float, animation_trigger, eval_projection_animation,
    eval_projection_behavior, eval_projection_body, eval_projection_chunk_group,
    eval_projection_chunk_part, eval_projection_cloth, eval_projection_environment,
    eval_projection_fluid, eval_projection_fx, eval_projection_ik, eval_projection_instances,
    eval_projection_lights, eval_projection_machine, eval_projection_maps,
    eval_projection_material_region, eval_projection_parent, eval_projection_ragdoll,
    eval_projection_sim, eval_projection_skin, viewer_animation, viewer_animation_speed,
    viewer_behaviors, viewer_cloth, viewer_flag, viewer_fluid, viewer_physics, viewer_play_mode,
    viewer_press, viewer_sim, viewer_texture_update,
};
pub use tangent_exports::eval_tangents;
mod cloth_live;
mod fluid_live;
mod sim_api;
mod sim_live;
mod physics;
pub mod quality;
mod scene;
mod selection;
mod skinning;
#[cfg(any(feature = "bevy-webgl", feature = "bevy-webgpu"))]
mod skinning_gpu;
mod status;
mod studio;
mod tangent_exports;
mod viewer;
mod viewer_nav;
mod wands;

use std::str::FromStr;

use ve_platform_web::CanvasHandle;
use wasm_bindgen::prelude::*;

/// Starts the engine on the canvas with the given element id, showing a built-in test scene
/// (`"kernel"`, `"materials"`, `"lights"`, `"stereo"` or `"xr"`; an empty string means `"kernel"`).
///
/// # Errors
/// Fails if the canvas cannot be found or the scene name is unknown.
#[wasm_bindgen]
pub fn start(canvas_id: &str, scene: &str) -> Result<(), JsError> {
    // Without a hook a wasm panic only surfaces as "unreachable executed".
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    let scene = scene::SceneKind::from_str(scene)
        .map_err(|()| JsError::new(&format!("unknown scene {scene:?}")))?;
    app::start(canvas_id, scene).map_err(|error| JsError::new(&error.to_string()))
}

/// JSON describing the running engine: `{ "frames", "adapter", "backend", "limits", "errors", "xr" }`.
#[wasm_bindgen]
#[must_use]
pub fn status() -> String {
    status::snapshot()
}

/// Loads and resets the live game from evaluator text; returns an empty string on success.
#[wasm_bindgen]
#[must_use]
pub fn game_load(text: &str) -> String {
    game::load(text)
}

/// JSON snapshot of the current live game, its entities, drawings, and recent effects.
#[wasm_bindgen]
#[must_use]
pub fn game_status() -> String {
    game::status()
}

/// Asks the live game for a save (everything its deterministic state holds). The game writes it on its next frame; read it with
/// `game_save_take`.
#[wasm_bindgen]
pub fn game_save_request() {
    game::request_save();
}

/// The save the game wrote since `game_save_request`, as JSON text; empty while it has not written one, or when no game is running.
#[wasm_bindgen]
#[must_use]
pub fn game_save_take() -> String {
    game::take_save()
}

/// Queues a save game (the text `game_save_take` gave) to be loaded into the running game. Returns the reason when the text is not a save
/// game, or an empty string when it was queued; `game_save_result` says on the next frame whether it fitted the game.
#[wasm_bindgen]
#[must_use]
pub fn game_save_load(text: &str) -> String {
    game::request_restore(text)
}

/// How the last `game_save_load` went: empty while pending, `ok`, or the reason the running game refused the save.
#[wasm_bindgen]
#[must_use]
pub fn game_save_result() -> String {
    game::restore_result()
}

/// Sets the desktop hand's grip and trigger for the live game (the page maps keys to them). A headset uses its controllers instead.
#[wasm_bindgen]
pub fn game_desktop_hand(grip: bool, trigger: bool) {
    game_hands::set_desktop(grip, trigger);
}

/// Replaces every hand of the live game with `[{"side": "left"|"right", "position": [x, y, z], "grip": bool, "trigger": bool}]` (for tests and
/// recordings); an empty string goes back to the controllers or the desktop hand. Returns false for text that is not that.
#[wasm_bindgen]
#[must_use]
pub fn game_set_hands(json: &str) -> bool {
    game_hands::set_override(json)
}

/// JSON array of the live game's effects numbered after `after` (pass the last `seq` seen, or 0): `{seq, kind, name, target, position}`.
#[wasm_bindgen]
#[must_use]
pub fn game_effects_since(after: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sequence numbers are small whole numbers passed through a JavaScript number"
    )]
    game::effects_since(after.max(0.0) as u64)
}

/// Turns the automatic quality governor on or off. When on, the quality tier follows measured frame
/// times (down quickly when over budget, up slowly with headroom).
#[wasm_bindgen]
pub fn set_governor(enabled: bool) {
    perf::set_governor(enabled);
}

/// Sets the refresh rate the page's window is judged against, for frame statistics and the governor
/// (default 60). A headset session uses the rate its runtime reports instead.
#[wasm_bindgen]
pub fn set_window_hz(hz: f32) {
    perf::set_window_hz(hz);
}

/// Turns the desktop viewer on or off. When on, the scene's camera becomes a free-look camera driven
/// by `viewer_look` and `viewer_move`. `fov_deg` is the vertical field of view in degrees (0 keeps
/// the scene's own), `stereo` splits the view into the two eyes of a mock headset, and `speed` is
/// metres per second.
#[wasm_bindgen]
pub fn viewer_configure(enabled: bool, fov_deg: f32, stereo: bool, speed: f32) {
    viewer::configure(enabled, fov_deg, stereo, speed);
}

/// Turns the view by a mouse movement, in pixels.
#[wasm_bindgen]
pub fn viewer_look(dx: f32, dy: f32) {
    viewer::add_look(dx, dy);
}

/// Sets the wanted viewer motion: right, up and forward each in -1..=1, and whether to sprint.
#[wasm_bindgen]
pub fn viewer_move(right: f32, up: f32, forward: f32, sprint: bool) {
    viewer::set_motion(right, up, forward, sprint);
}

/// Tells the engine whether the page holds the pointer for mouse look. While it does, the crosshair
/// at the centre of the view hovers, picks and grabs objects.
#[wasm_bindgen]
pub fn viewer_set_locked(locked: bool) {
    viewer::set_locked(locked);
}

/// Chooses the viewer's lens: "off", "fisheye" or "headset". Returns false for any other name.
#[wasm_bindgen]
#[must_use]
pub fn viewer_set_lens(name: &str) -> bool {
    viewer::set_lens(name)
}

/// Jumps the viewer to a position looking at a target (metres), for fixed review views. Returns
/// false for non-finite values or a zero-length view. The viewer must be on.
#[wasm_bindgen]
#[must_use]
pub fn viewer_look_at(px: f32, py: f32, pz: f32, tx: f32, ty: f32, tz: f32) -> bool {
    viewer::set_look_at(
        bevy_math::Vec3::new(px, py, pz),
        bevy_math::Vec3::new(tx, ty, tz),
    )
}

/// Zooms the viewer toward its focus: wheel movement, positive zooms out, about 100 per notch.
#[wasm_bindgen]
pub fn viewer_zoom(amount: f32) {
    viewer_nav::zoom(amount);
}

/// Orbits the viewer around its focus by a drag, in pixels.
#[wasm_bindgen]
pub fn viewer_orbit(dx: f32, dy: f32) {
    viewer_nav::orbit(dx, dy);
}

/// Moves the viewer so the box from `min` to `max` (metres) fills the view, and makes its centre the
/// point that zoom and orbit work around. Returns false for non-finite or empty boxes.
#[wasm_bindgen]
#[must_use]
pub fn viewer_frame(
    min_x: f32,
    min_y: f32,
    min_z: f32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
) -> bool {
    let (lo, hi) = (
        bevy_math::Vec3::new(min_x, min_y, min_z),
        bevy_math::Vec3::new(max_x, max_y, max_z),
    );
    viewer_nav::frame((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(0.02))
}

/// Projects a world point to canvas pixels as JSON `{x, y, depth}`; `y` is measured from the top.
#[wasm_bindgen]
#[must_use]
pub fn viewer_project(x: f32, y: f32, z: f32, width: f32, height: f32) -> String {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return String::new();
    }
    let Some(point) = viewer::project(bevy_math::Vec3::new(x, y, z), width / height) else {
        return String::new();
    };
    serde_json::json!({
        "x": (point.x + 1.0) * 0.5 * width,
        "y": (1.0 - point.y) * 0.5 * height,
        "depth": point.z,
    })
    .to_string()
}

/// Selects `node` and lights up the viewport parts named in `parts_json` (a JSON array of node
/// ids: the outputs the node builds). Returns false, changing nothing, for input that is not a
/// short list of short ids.
#[wasm_bindgen]
#[must_use]
pub fn selection_set(node: &str, parts_json: &str) -> bool {
    const LONGEST_ID: usize = 128;
    const MOST_PARTS: usize = 1024;
    let Ok(parts) = serde_json::from_str::<Vec<String>>(parts_json) else {
        return false;
    };
    if node.is_empty()
        || node.len() > LONGEST_ID
        || parts.len() > MOST_PARTS
        || parts.iter().any(|part| part.len() > LONGEST_ID)
    {
        return false;
    }
    selection::set(Some(node.to_owned()), parts.into_iter().collect());
    true
}

/// Clears the selection.
#[wasm_bindgen]
pub fn selection_clear() {
    selection::set(None, std::collections::BTreeSet::new());
}

/// The selection as JSON: `{ node, parts, generation }`; `generation` changes whenever it does, so
/// the page can notice a click in the viewport.
#[wasm_bindgen]
#[must_use]
pub fn selection_get() -> String {
    selection::to_json()
}

/// Puts the viewer back where the scene started.
#[wasm_bindgen]
pub fn viewer_reset() {
    viewer::request_reset();
}

/// Binds the current candidate to one exact worker request token.
#[wasm_bindgen]
#[must_use]
pub fn eval_bind(token: &str) -> bool {
    document::staging::bind(token)
}

/// Rejects a failed candidate, except that a Preview triangle budget refusal keeps a staged edit for Final.
#[wasm_bindgen]
#[must_use]
pub fn eval_reject(token: &str, code: &str, message: &str) -> bool {
    document::staging::reject(token, code, message)
}

/// Starts bounded result staging if this token is still current.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_begin(token: &str) -> bool {
    projection::begin(token)
}

/// Sets the surface (`{ color, metallic, roughness, emissive }` as JSON) of the part made by `node`
/// in the scene being staged. Refused when a value is out of range or the token is not current.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_material(token: &str, node: &str, material: &str) -> bool {
    projection::material(token, node, material)
}

/// Queues a small triangle chunk containing interleaved positions and normals.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_chunk(token: &str, node: &str, name: &str, vertices: &[f32]) -> bool {
    projection::chunk(token, node, name, vertices)
}

/// Marks a complete candidate; its document commits when the GPU meshes are ready.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_finish(token: &str) -> bool {
    projection::finish(token)
}

/// Marks the staged scene as a draft: the parts finished so far, shown while the evaluation continues. It replaces the scene on screen
/// when its meshes are ready but does not commit the document; the finished evaluation replaces it.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_finish_draft(token: &str) -> bool {
    projection::finish_draft(token)
}

/// Projection resource and upload counters.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_status() -> String {
    projection::status()
}

/// Whether the browser can present an immersive VR session.
#[wasm_bindgen]
pub async fn xr_supported() -> bool {
    ve_xr_web::is_supported().await
}

/// Enters an immersive VR session on the canvas. On real devices the browser requires this to be
/// called from a user gesture (for example a button click).
///
/// # Errors
/// Fails if the canvas cannot be found, `WebXR` is unavailable, or the browser refuses the session.
#[wasm_bindgen]
pub async fn enter_xr(canvas_id: &str) -> Result<(), JsError> {
    let canvas = CanvasHandle::find(canvas_id).map_err(|error| JsError::new(&error.to_string()))?;
    ve_xr_web::enter(canvas.element(), quality::layer_scale())
        .await
        .map_err(|error| JsError::new(&error.to_string()))
}

/// Ends the running immersive session, if any.
#[wasm_bindgen]
pub async fn exit_xr() {
    ve_xr_web::exit().await;
}

/// Current viewer camera pose as JSON `{position, rotation}` for spatial browser audio.
#[wasm_bindgen]
#[must_use]
pub fn viewer_pose() -> String {
    viewer::pose().map_or_else(String::new, |(position, rotation)| {
        serde_json::json!({ "position": position.to_array(), "rotation": rotation.to_array() })
            .to_string()
    })
}
