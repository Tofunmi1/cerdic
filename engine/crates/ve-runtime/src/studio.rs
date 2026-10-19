//! The studio room grows to hold the model being looked at.
//!
//! The room is 14 m across, which suits a table or a vase. A cottage framed to fill the view puts the
//! camera further back than that, outside the walls, so the picture would be a wall. When the page frames
//! a bigger model the room's floor and walls scale about the origin so the camera stays inside.

use bevy_ecs::component::Component;
use bevy_ecs::system::Query;
use bevy_math::Vec3;
use bevy_transform::components::Transform;

use crate::viewer_nav;

/// One floor or wall of the studio room, with its position at the room's normal size.
#[derive(Component)]
pub(crate) struct StudioRoom {
    pub(crate) base_position: Vec3,
}

/// Keeps every room piece at the current room scale.
pub(crate) fn scale_room(mut pieces: Query<(&StudioRoom, &mut Transform)>) {
    let scale = viewer_nav::room_scale();
    for (piece, mut transform) in &mut pieces {
        if (transform.scale.x - scale).abs() > 1.0e-4 {
            transform.scale = Vec3::splat(scale);
            transform.translation = piece.base_position * scale;
        }
    }
}
