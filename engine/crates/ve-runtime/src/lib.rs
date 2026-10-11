//! The runtime entry point called from the web page.
//!
//! `start(canvas_id, scene)` assembles the engine's plugins into a Bevy app that renders into the
//! canvas. `status()` reports what the page needs to verify the run (adapter, limits, frame count,
//! XR session). `enter_xr` and `exit_xr` move the running app into and out of a headset session.
#![cfg(target_arch = "wasm32")]

mod app;
mod bench_scene;
mod mock_hmd;
mod perf;
mod quality;
mod scene;
mod status;
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
    ve_xr_web::enter(canvas.element(), quality::resolution_scale())
        .await
        .map_err(|error| JsError::new(&error.to_string()))
}

/// Ends the running immersive session, if any.
#[wasm_bindgen]
pub async fn exit_xr() {
    ve_xr_web::exit().await;
}
