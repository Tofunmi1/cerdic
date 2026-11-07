//! The studio room grows to hold the model being looked at.
//!
//! The room is 14 m across, which suits a table or a vase. A cottage framed to fill the view puts the
//! camera further back than that, outside the walls, so the picture would be a wall. When the page frames
//! a bigger model the room's floor and walls scale about the origin so the camera stays inside.
//!
//! A game that drives its own camera has a world of its own, so the room is hidden while its scripts hold the view.

use std::cell::Cell;

use bevy_camera::visibility::Visibility;
use bevy_ecs::component::Component;
use bevy_ecs::system::Query;
use bevy_math::Vec3;
use bevy_transform::components::Transform;

use crate::viewer_nav;

thread_local! {
    static HIDDEN: Cell<bool> = const { Cell::new(false) };
}

/// Hides or shows the studio room (a game's own camera hides it).
pub(crate) fn set_hidden(hidden: bool) {
    HIDDEN.with(|flag| flag.set(hidden));
}

/// One floor or wall of the studio room, with its position at the room's normal size.
#[derive(Component)]
pub(crate) struct StudioRoom {
    pub(crate) base_position: Vec3,
}

/// Keeps every room piece at the current room scale and hidden or shown.
pub(crate) fn scale_room(mut pieces: Query<(&StudioRoom, &mut Transform, &mut Visibility)>) {
    let scale = viewer_nav::room_scale();
    let wanted = if HIDDEN.with(Cell::get) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (piece, mut transform, mut visibility) in &mut pieces {
        if (transform.scale.x - scale).abs() > 1.0e-4 {
            transform.scale = Vec3::splat(scale);
            transform.translation = piece.base_position * scale;
        }
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}
