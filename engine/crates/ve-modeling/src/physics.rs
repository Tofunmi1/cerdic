//! `phys.body` (ADR 0035): marks a part as a physics body. In play mode the viewer simulates it (gravity, collisions,
//! being thrown); this module only records what was asked and drops bodies that cannot work.

include!("physics/part_1.rs");
include!("physics/part_2.rs");
