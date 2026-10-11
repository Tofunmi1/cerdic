//! The runtime entry point called from the web page.
//!
//! `start(canvas_id)` assembles the engine's plugins into a Bevy app that renders into the canvas.
//! `status()` reports what the page needs to verify the run (adapter, limits, frame count).
#![cfg(target_arch = "wasm32")]

mod app;
mod status;

use wasm_bindgen::prelude::*;

/// Starts the engine on the canvas with the given element id.
///
/// # Errors
/// Fails if the canvas cannot be found.
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsError> {
    // Without a hook a wasm panic only surfaces as "unreachable executed".
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    app::start(canvas_id).map_err(|error| JsError::new(&error.to_string()))
}

/// JSON describing the running engine: `{ "frames", "adapter", "backend", "limits", "errors" }`.
#[wasm_bindgen]
#[must_use]
pub fn status() -> String {
    status::snapshot()
}
