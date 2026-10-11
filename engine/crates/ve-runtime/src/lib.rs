//! The runtime entry point called from the web page.
//!
//! `start(canvas_id, scene)` assembles the engine's plugins into a Bevy app that renders into the
//! canvas. `status()` reports what the page needs to verify the run (adapter, limits, frame count).
#![cfg(target_arch = "wasm32")]

mod app;
mod scene;
mod status;

use std::str::FromStr;

use wasm_bindgen::prelude::*;

/// Starts the engine on the canvas with the given element id, showing a built-in test scene
/// (`"kernel"`, `"materials"` or `"lights"`; an empty string means `"kernel"`).
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

/// JSON describing the running engine: `{ "frames", "adapter", "backend", "limits", "errors" }`.
#[wasm_bindgen]
#[must_use]
pub fn status() -> String {
    status::snapshot()
}
