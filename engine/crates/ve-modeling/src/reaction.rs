//! `behavior.on`: a part that reacts (ADR 0027). The node attaches a [`Behavior`] (data); the viewer runs it as a
//! [`Reaction`] in fixed steps, which is pure and deterministic so the same triggers give the same poses.

include!("reaction/part_1.rs");
include!("reaction/part_2.rs");
include!("reaction/part_3.rs");
