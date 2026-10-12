//! The runtime entry point called from the web page.
//!
//! `start(canvas_id, scene)` assembles the engine's plugins into a Bevy app that renders into the
//! canvas. `status()` reports what the page needs to verify the run (adapter, limits, frame count,
//! XR session). `enter_xr` and `exit_xr` move the running app into and out of a headset session.
#![cfg(target_arch = "wasm32")]

mod app;
mod bench_scene;
mod document;
mod gallery;
mod interact;
mod mock_hmd;
mod perf;
mod projection;
mod quality;
mod scene;
mod status;
mod viewer;
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

/// Switches the render quality tier (`"low"`, `"medium"` or `"high"`). Takes effect on the next
/// frame, and may be called before `start`.
///
/// # Errors
/// Fails if the tier name is unknown.
#[wasm_bindgen]
pub fn set_quality(tier: &str) -> Result<(), JsError> {
    let tier = tier
        .parse()
        .map_err(|()| JsError::new(&format!("unknown quality tier {tier:?}")))?;
    quality::request(tier);
    Ok(())
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

/// Puts the viewer back where the scene started.
#[wasm_bindgen]
pub fn viewer_reset() {
    viewer::request_reset();
}

/// Opens the document called `name` from local storage (or starts an empty one with `seed`) and
/// returns the state as JSON: `{ id, revision, nodes, can_undo, can_redo, read_only,
/// saved_generation, issues, document }`. A document that cannot be loaded opens read-only with the
/// reasons in `issues`; storage is not modified.
#[wasm_bindgen]
#[must_use]
pub fn doc_open(name: &str, seed: u32) -> String {
    document::open(name, u64::from(seed))
}

/// Starts an empty document called `name`, replacing the session (nothing is saved yet).
#[wasm_bindgen]
#[must_use]
pub fn doc_reset(name: &str, seed: u32) -> String {
    document::reset(name, u64::from(seed))
}

/// The current document and session state, as for `doc_open`.
#[wasm_bindgen]
#[must_use]
pub fn doc_state() -> String {
    document::state()
}

/// Applies a command batch (JSON) and returns the outcome as JSON: `committed`, `rejected`,
/// `conflict` or `already_applied`. Bad input is a `rejected` outcome, never an exception.
#[wasm_bindgen]
#[must_use]
pub fn doc_apply(batch_json: &str) -> String {
    document::apply(batch_json)
}

/// Undoes the last transaction: `{ revision, changed }`, or `null` if there is nothing to undo.
#[wasm_bindgen]
#[must_use]
pub fn doc_undo() -> String {
    document::step(true)
}

/// Redoes the last undone transaction: `{ revision, changed }`, or `null`.
#[wasm_bindgen]
#[must_use]
pub fn doc_redo() -> String {
    document::step(false)
}

/// Saves the document to local storage: `{ generation }`, or `{ error }` with a diagnostic. The
/// previous save stays intact if this one fails.
#[wasm_bindgen]
#[must_use]
pub fn doc_save() -> String {
    document::save()
}

/// Every operator the session knows, as JSON: ports, parameters with bounds, readiness.
#[wasm_bindgen]
#[must_use]
pub fn doc_operators() -> String {
    document::operators()
}

/// Reads the text form into commands: `{ commands, spans }` or `{ diagnostics }` (each with a source
/// span). The page wraps the commands in a batch for the current revision.
#[wasm_bindgen]
#[must_use]
pub fn doc_parse_text(text: &str) -> String {
    document::text::parse(text)
}

/// The working document as canonical text: `{ text }` or `{ error }`.
#[wasm_bindgen]
#[must_use]
pub fn doc_text() -> String {
    document::text::print()
}

/// The Rust-owned working candidate and its dependency hash.
#[wasm_bindgen]
#[must_use]
pub fn doc_candidate_state() -> String {
    document::staging::state()
}

/// Stages a batch without committing its document before geometry succeeds.
#[wasm_bindgen]
#[must_use]
pub fn doc_stage(batch: &str) -> String {
    document::staging::apply(batch)
}

/// Stages an undo for evaluation before it becomes the committed scene.
#[wasm_bindgen]
#[must_use]
pub fn doc_stage_undo() -> String {
    document::staging::step(true)
}

/// Stages a redo for evaluation before it becomes the committed scene.
#[wasm_bindgen]
#[must_use]
pub fn doc_stage_redo() -> String {
    document::staging::step(false)
}

/// Binds the current candidate to one exact worker request token.
#[wasm_bindgen]
#[must_use]
pub fn eval_bind(token: &str) -> bool {
    document::staging::bind(token)
}

/// Discards a failed matching candidate while preserving the committed scene.
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
