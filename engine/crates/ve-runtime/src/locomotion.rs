//! VR locomotion v1: the left thumbstick walks the player where the head faces, the right thumbstick snap-turns in fixed steps.
//! What moves is the tracking origin (`XrRig`); the player's head follows it. Collision is the game's: a solid that holds the hero back shoves the origin back too.
use bevy_ecs::{
    query::With,
    system::{Local, Query, Res, ResMut},
};
use bevy_math::{Mat4, Vec3};
use bevy_time::{Real, Time};
use ve_render::StereoViews;
use ve_xr_web::frame::Handedness;
use ve_xr_web::{SessionState, XrCamera, XrInput, XrRig, session_state};

/// Walking pace in metres a second.
const WALK_SPEED: f32 = 2.5;
/// A stick pushed less than this counts as resting.
const DEAD_ZONE: f32 = 0.2;
/// One snap turn, in radians (30 degrees).
const SNAP_TURN: f32 = std::f32::consts::FRAC_PI_6;
/// A flick must pass this to turn, and the stick must come back under [`SNAP_RESET`] before it can turn again.
const SNAP_TRIGGER: f32 = 0.7;
const SNAP_RESET: f32 = 0.3;

/// Whether the right stick has turned the player and is still held out.
#[derive(Default)]
pub(crate) struct SnapLatch(bool);

/// The thumbstick of a controller: axes 2 and 3 on the standard mapping.
fn stick(input: &XrInput, hand: Handedness) -> Option<(f32, f32)> {
    let controller = input
        .controllers
        .iter()
        .find(|controller| controller.handedness == hand)?;
    Some((controller.axes[2], controller.axes[3]))
}

/// Applies the sticks to the tracking origin for this frame.
pub(crate) fn update(
    time: Res<Time<Real>>,
    input: Res<XrInput>,
    rig: Option<ResMut<XrRig>>,
    cameras: Query<&StereoViews, With<XrCamera>>,
    mut latch: Local<SnapLatch>,
) {
    let (Some(mut rig), Some(stereo)) = (rig, cameras.iter().next()) else {
        return;
    };
    if session_state() != SessionState::Presenting {
        return;
    }
    let head = (stereo.eyes[0].world_from_view.w_axis.truncate()
        + stereo.eyes[1].world_from_view.w_axis.truncate())
        * 0.5;
    let forward = stereo.eyes[0]
        .world_from_view
        .transform_vector3(Vec3::NEG_Z);
    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let right = Vec3::new(-forward.z, 0.0, forward.x);

    if let Some((x, y)) = stick(&input, Handedness::Left)
        && x.hypot(y) > DEAD_ZONE
    {
        let amount = time.delta_secs() * WALK_SPEED;
        let step = (forward * -y + right * x) * amount;
        rig.0.w_axis += step.extend(0.0);
    }

    let turn = stick(&input, Handedness::Right).map_or(0.0, |(x, _)| x);
    if latch.0 {
        if turn.abs() < SNAP_RESET {
            latch.0 = false;
        }
    } else if turn.abs() > SNAP_TRIGGER {
        latch.0 = true;
        // Turn about the head, so the player stays where they stand. Pushing right turns right (clockwise from above).
        let angle = -turn.signum() * SNAP_TURN;
        rig.0 = Mat4::from_translation(head)
            * Mat4::from_rotation_y(angle)
            * Mat4::from_translation(-head)
            * rig.0;
    }
}
