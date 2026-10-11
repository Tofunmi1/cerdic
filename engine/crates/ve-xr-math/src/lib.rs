//! Pure maths for stereo and XR views.
//!
//! Conventions match the renderer: right-handed, X right, Y up, the camera looks down -Z, and
//! projections are reverse-z with an infinite far plane (depth 1 at the near plane, 0 at infinity).
//! Nothing here depends on the engine, the GPU or the browser, so all of it is tested natively.

mod fov;
mod layout;
mod pose;

pub use fov::{FovError, FovTangents};
pub use layout::{Viewport, side_by_side};
pub use pose::{EyeView, MockHmd, Pose, eye_pose};
