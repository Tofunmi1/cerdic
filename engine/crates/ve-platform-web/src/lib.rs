//! Browser platform layer.
//!
//! Replaces `winit` for the web: a canvas becomes the primary `Window`, a `requestAnimationFrame`
//! loop drives the Bevy `App`, and pointer events become Bevy input messages. The `WebXR` frame
//! loop (a separate runner driven by `XRSession.requestAnimationFrame`) lives in `ve-xr-web`.
//!
//! The crate is empty on native targets so the workspace still builds and lints everywhere.
#![cfg(target_arch = "wasm32")]
#![expect(
    unsafe_code,
    reason = "canvas raw-window-handle bridging needs two audited unsafe items"
)]

#[cfg(target_feature = "atomics")]
compile_error!(
    "ve-platform-web assumes single-threaded wasm; the canvas handle is not thread safe"
);

mod canvas;
mod error;
mod input;
mod plugin;
mod runner;

pub use canvas::CanvasHandle;
pub use error::PlatformError;
pub use plugin::{PlatformStartupError, WebPlatformPlugin};
pub use runner::{pump_frame, set_externally_driven};
