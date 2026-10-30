//! Where the player is, for the live game: the hero's feet, the viewer's start, and the head pose the HUD hangs from, on the desktop and in a headset.
use bevy_ecs::{query::With, system::Query};
use bevy_math::Vec3;
use ve_game::{Status, World};
use ve_render::StereoViews;
use ve_xr_web::{SessionState, XrCamera, XrRig, session_state};

use crate::viewer::ViewerRig;

/// A standing eye is this high above the floor.
pub(crate) const EYE_HEIGHT: f32 = 1.6;

#[expect(
    clippy::cast_possible_truncation,
    reason = "game coordinates are finite and the renderer stores transforms as f32"
)]
pub(crate) fn render_position(position: [f64; 3]) -> Vec3 {
    Vec3::new(position[0] as f32, position[1] as f32, position[2] as f32)
}

/// Where the hero's feet should be this frame. On the desktop that is the viewer's eye minus the standing height. In a presenting headset it is the
/// head's position over the floor of the tracking space; the first presenting frame after a load also moves the tracking origin so the player
/// stands at the spawn (`start`, taken once).
pub(crate) fn viewer_feet(
    rig: &ViewerRig,
    xr_cameras: &Query<&StereoViews, With<XrCamera>>,
    xr_rig: Option<&mut XrRig>,
    start: &mut Option<Vec3>,
) -> Vec3 {
    if session_state() == SessionState::Presenting
        && let (Some(stereo), Some(xr_rig)) = (xr_cameras.iter().next(), xr_rig)
    {
        let head = (stereo.eyes[0].world_from_view.w_axis.truncate()
            + stereo.eyes[1].world_from_view.w_axis.truncate())
            * 0.5;
        let origin = xr_rig.0.w_axis.truncate();
        if let Some(spawn) = start.take() {
            let delta = Vec3::new(spawn.x - head.x, spawn.y - origin.y, spawn.z - head.z);
            xr_rig.0.w_axis += delta.extend(0.0);
            return spawn;
        }
        return Vec3::new(head.x, origin.y, head.z);
    }
    let eye = crate::viewer::pose().map_or_else(|| rig.position(), |(position, _)| position);
    eye - Vec3::Y * EYE_HEIGHT
}

/// Moves the player's body by `push` (a horizontal vector) when the game held the hero back, so the viewer is shoved out of a shut door instead of
/// standing in it: the tracking origin in a presenting headset, the eye on the desktop.
pub(crate) fn push_player(push: Vec3, rig: &mut ViewerRig, xr_rig: Option<&mut XrRig>) {
    if session_state() == SessionState::Presenting {
        if let Some(xr_rig) = xr_rig {
            xr_rig.0.w_axis += push.extend(0.0);
        }
        return;
    }
    let eye = rig.position() + push;
    rig.place_eye(eye);
}

/// When a game with a followed hero loads, the viewer stands at the hero's spawn (feet on the spawn point) looking toward the first other entity.
pub(crate) fn place_viewer_at_start(world: &World) {
    let Some(feet) = world.viewer_start() else {
        return;
    };
    let eye = render_position(feet) + Vec3::Y * EYE_HEIGHT;
    let toward = world
        .entities()
        .iter()
        .map(|entity| render_position(entity.position))
        .find(|position| (position.x - eye.x).abs() + (position.z - eye.z).abs() > 0.5)
        .map_or(eye - Vec3::Z, |position| {
            Vec3::new(position.x, eye.y, position.z)
        });
    crate::viewer::set_look_at(eye, toward);
}

/// A `message` effect named `goto_X_Y_Z_LX_LZ` moves the player's feet to (X, Y, Z) facing (LX, LZ): how a game puts the player at the next tee.
/// Returns whether the name was one (a headset's tracking origin is not moved by this yet: the desktop view is).
pub(crate) fn goto_effect(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("goto_") else {
        return false;
    };
    let numbers: Vec<f32> = rest
        .split('_')
        .filter_map(|part| part.parse().ok())
        .collect();
    let [x, y, z, look_x, look_z] = numbers[..] else {
        return false;
    };
    let eye = Vec3::new(x, y + EYE_HEIGHT, z);
    crate::viewer::set_look_at(eye, Vec3::new(look_x, eye.y, look_z));
    true
}

/// One panel to draw: where it sits, how big its font pixels are, and the text with variables filled in.
pub(crate) struct DisplayHud {
    pub(crate) anchor: String,
    pub(crate) size: f64,
    pub(crate) text: String,
}

/// The game's HUD panels in order, plus a banner above them once the game is won or lost (every game says so, whatever its panels).
pub(crate) fn display_huds(world: &World) -> Vec<DisplayHud> {
    let mut shown: Vec<DisplayHud> = world
        .huds()
        .iter()
        .zip(world.hud_lines())
        .map(|(hud, text)| DisplayHud {
            anchor: hud.anchor.clone(),
            size: hud.size,
            text,
        })
        .collect();
    let banner = match world.status() {
        Status::Playing => None,
        Status::Won => Some("YOU WON"),
        Status::Lost => Some("YOU LOST"),
    };
    if let Some(text) = banner {
        shown.push(DisplayHud {
            anchor: "top".to_owned(),
            size: 0.01,
            text: text.to_owned(),
        });
    }
    shown
}

/// Where the viewer's head is and which way it faces (yaw about up), in a headset or on the desktop.
pub(crate) fn eye_and_yaw(
    rig: &ViewerRig,
    xr_cameras: &Query<&StereoViews, With<XrCamera>>,
) -> (Vec3, f32) {
    if session_state() == SessionState::Presenting
        && let Some(stereo) = xr_cameras.iter().next()
    {
        let eyes = [
            stereo.eyes[0].world_from_view.w_axis.truncate(),
            stereo.eyes[1].world_from_view.w_axis.truncate(),
        ];
        let forward = stereo.eyes[0]
            .world_from_view
            .transform_vector3(Vec3::NEG_Z);
        return ((eyes[0] + eyes[1]) * 0.5, (-forward.x).atan2(-forward.z));
    }
    crate::viewer::pose().map_or_else(
        || {
            let forward = rig.rotation() * Vec3::NEG_Z;
            (rig.position(), (-forward.x).atan2(-forward.z))
        },
        |(position, rotation)| {
            let forward = rotation * Vec3::NEG_Z;
            (position, (-forward.x).atan2(-forward.z))
        },
    )
}
