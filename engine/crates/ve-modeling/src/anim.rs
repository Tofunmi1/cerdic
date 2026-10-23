//! `anim.track`: the shape passes through unchanged and carries one keyframed transform track.
//!
//! A track animates a part's position, rotation or scale over time, optionally about a pivot (a door's
//! hinge, a fan's axle). Tracks are plain data: the evaluator only validates and carries them, the viewer
//! plays them and the glTF exporter writes them as animations. Several `anim.track` nodes chained on one
//! shape give it several tracks, which play together.

include!("anim/part_1.rs");
include!("anim/part_2.rs");
include!("anim/drop.rs");
