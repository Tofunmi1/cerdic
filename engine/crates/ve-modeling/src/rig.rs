//! `rig.bone` and `rig.skin`: skinned meshes that bend with moving bones (ADR 0030).
//!
//! A bone is an ordinary named part (it moves through `anim.track` and `anim.parent`); `rig.bone` records its rest
//! segment. `rig.skin` marks a shape as skinned by a list of bone parts. After evaluation each vertex gets weights
//! from its distance to the bones' segments. Everything is authored at the rest pose in world coordinates, so a
//! bone's rest transform is the identity and the deformation is just `sum(weight * pose * vertex)`.

include!("rig/part_1.rs");
include!("rig/part_2.rs");
include!("rig/part_3.rs");
