//! Bounded spatial scalar fields and their deterministic kernel tape.
mod expr;
mod model;
mod repeat;
mod tape;

pub use model::{DistanceGuarantee, Field};
/// Maximum number of control points in a tube polyline.
pub const MAX_TUBE_POINTS: usize = 128;
/// Maximum repeated copies on each side of the original field.
pub const MAX_REPEAT_COUNT: u32 = 16;
pub use tape::{MAX_INSTRUCTIONS, compile, evaluate, mesh};
