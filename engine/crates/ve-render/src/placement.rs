//! Authored instance matrices are not restricted to TRS decomposition.
use bevy_ecs::component::Component;
use bevy_math::Mat4;

/// Exact authored affine placement and stable source index for per-instance materials.
#[derive(Component, Clone, Copy, Debug)]
pub struct VeInstanceTransform(pub Mat4, pub u32);
