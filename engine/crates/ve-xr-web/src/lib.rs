//! `WebXR` layer.
//!
//! A small JavaScript bridge (`js/xr.js`) requests the session and reads poses; this crate turns
//! each frame into engine types ([`frame`]), applies it to the headset camera ([`plugin`]) and
//! manages the session ([`session`], browser only). While presenting, the headset's frame loop
//! drives the app instead of `requestAnimationFrame`.

pub mod frame;
mod plugin;
mod pointers;
mod state;

#[cfg(target_arch = "wasm32")]
mod bridge;
#[cfg(target_arch = "wasm32")]
mod layer;
#[cfg(target_arch = "wasm32")]
mod session;

pub use plugin::{XrCamera, XrInput, XrPlugin, XrRig, apply_xr_frame};
pub use pointers::{controller_pointer, pointer_id};
#[cfg(target_arch = "wasm32")]
pub use session::{XrError, enter, exit, is_supported};
pub use state::{
    SessionState, frames_published, last_error, last_frame, publish_frame, refresh_hz,
    session_state, set_last_error, set_refresh_hz, set_session_state, take_latest,
};
