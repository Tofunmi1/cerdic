//! Authored instance matrices are not restricted to TRS decomposition.
use bevy_ecs::component::Component;
use bevy_math::Mat4;

/// Exact authored affine placement, overriding `GlobalTransform` during mesh extraction.
#[derive(Component, Clone, Copy, Debug)]
pub struct VeInstanceTransform(pub Mat4);
