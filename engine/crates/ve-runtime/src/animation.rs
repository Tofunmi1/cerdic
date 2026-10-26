//! Playing `anim.track` clips: parts that carry tracks are posed from a shared clock every frame.

mod clock;
pub(crate) use clock::{AnimationClock, advance, apply_requested, control, set_speed};

include!("animation/part_1.rs");
include!("animation/part_2.rs");
include!("animation/secondary.rs");
include!("animation/machine.rs");
