//! Game cameras in the runtime: the pure rig of `ve-game` (`camera` statements) drives the viewer. The viewer's yaw and pitch are the
//! player's look (the mouse or stick turns them between frames); each frame the rig turns the target's position and that look into a view
//! and writes it back, so the orbit state is the viewer's own and nothing is kept twice.
//!
//! A camera that holds a fixed angle (top down, side, isometric, fixed) takes the view away from the free viewer: its mouse look, wheel and
//! orbit drags are dropped (see `ViewerRig::driven`) and [`apply`] puts the pose on the real camera in the same frame it is computed, so
//! the picture never shows the free viewer's idea of the view for even one frame.
//!
//! The viewer faces -z at yaw 0 and the rig's compass yaw faces +z, so the two differ by half a turn.
use std::f32::consts::PI;

use bevy_camera::Camera3d;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res};
use bevy_math::Vec3;
use bevy_transform::components::Transform;
use ve_game::{CameraInput, CameraRig, Entity, World};

use crate::viewer::ViewerRig;

/// The game's compass yaw for a viewer yaw (what `camera.yaw` and the rig's orbit are measured in).
pub(crate) fn compass_yaw(viewer_yaw: f32) -> f64 {
    f64::from(viewer_yaw - PI)
}

/// The viewer yaw that looks along a compass yaw.
fn viewer_yaw(compass: f64) -> f32 {
    #[expect(clippy::cast_possible_truncation, reason = "an angle in radians, well inside f32")]
    {
        compass as f32 + PI
    }
}

/// The entity a camera follows: the first whose kind has the tag (`tag:hero`) or is the kind (`kind:player`).
fn target<'a>(world: &'a World, selector: &str) -> Option<&'a Entity> {
    let (class, name) = selector.split_once(':')?;
    world.entities().iter().find(|entity| match class {
        "kind" => entity.kind == name,
        "tag" => world.kind(&entity.kind).is_some_and(|kind| kind.tags.iter().any(|tag| tag == name)),
        _ => false,
    })
}

/// Places the viewer where the active camera says, for a step of `dt` seconds. When the game has no camera or its target does not exist
/// (yet) the viewer is given back to the free-look controls.
pub(crate) fn drive(cameras: &mut CameraRig, world: &World, viewer: &mut ViewerRig, dt: f64) {
    let Some(def) = cameras.active_def() else {
        viewer.release();
        return;
    };
    let Some(entity) = target(world, &def.target) else {
        viewer.release();
        return;
    };
    // first and third person are steered by the player's look; every other mode holds its own angle
    let orbit = matches!(def.mode.as_str(), "first" | "third");
    let (yaw, pitch) = viewer.look();
    let (zoom, turn, tilt) = viewer.adjustments();
    let input = CameraInput {
        target: entity.position,
        heading: entity.heading,
        orbit_yaw: compass_yaw(yaw),
        orbit_pitch: f64::from(pitch),
        dt,
        zoom: f64::from(zoom),
        yaw_offset: f64::from(turn),
        pitch_offset: f64::from(tilt),
    };
    // The arm shortens against the game's own solid boxes; the hit distance is measured from the pivot toward the wanted eye.
    let pose = cameras.pose(&input, &|from, to| world.solid_hit(from, to));
    #[expect(clippy::cast_possible_truncation, reason = "metres and radians, well inside f32")]
    {
        viewer.set_view(Vec3::new(pose.eye[0] as f32, pose.eye[1] as f32, pose.eye[2] as f32), viewer_yaw(pose.yaw), pose.pitch as f32, orbit);
    }
}

/// When the player switches into a first- or third-person camera, the look starts behind the hero (along the way it faces), a little
/// downward for third person, instead of however the last fixed-angle camera happened to point.
pub(crate) fn enter(cameras: &CameraRig, world: &World, viewer: &mut ViewerRig) {
    let Some(def) = cameras.active_def() else { return };
    let pitch = match def.mode.as_str() {
        "first" => 0.0,
        "third" => -0.18,
        _ => return,
    };
    if let Some(entity) = target(world, &def.target) {
        viewer.set_look(viewer_yaw(entity.heading), pitch);
    }
}

/// The entity whose own drawing is hidden: the one a first-person camera sits inside.
pub(crate) fn hidden_in_first_person(cameras: &CameraRig, world: &World) -> Option<u32> {
    let def = cameras.active_def().filter(|def| def.mode == "first")?;
    target(world, &def.target).map(|entity| entity.id)
}

/// Puts a game camera's view on the real camera, after the game has stepped this frame.
pub(crate) fn apply(rig: Res<ViewerRig>, mut cameras: Query<&mut Transform, With<Camera3d>>) {
    let Some((eye, rotation)) = rig.driven_pose() else {
        return;
    };
    for mut transform in &mut cameras {
        transform.translation = eye;
        transform.rotation = rotation;
    }
}
