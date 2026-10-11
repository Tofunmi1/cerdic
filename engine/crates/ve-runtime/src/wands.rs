//! Controller wands for the XR scene: a small kernel-made bar follows each tracked controller's grip
//! and swells while its trigger is held. It exercises the whole input path (runtime, rig, world).

use bevy_asset::Assets;
use bevy_color::Color;
use bevy_ecs::component::Component;
use bevy_ecs::system::{Commands, Query, Res};
use bevy_math::Vec3;
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::Transform;
use ve_kernel::{KernelError, Solid};
use ve_render::VeMaterial;
use ve_xr_web::frame::Handedness;
use ve_xr_web::{XrInput, XrRig};

use crate::scene::render_mesh;

/// How much a fully pressed trigger enlarges a wand.
const TRIGGER_SWELL: f32 = 0.8;

/// A wand that follows the controller of one hand.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Wand(pub(crate) Handedness);

/// Spawns one wand per hand, left orange and right blue. They start hidden until tracked.
///
/// # Errors
/// Fails if the kernel cannot build the wand's shape.
pub(crate) fn spawn_wands(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
) -> Result<(), KernelError> {
    let bar = meshes.add(render_mesh(&Solid::cube([0.04, 0.04, 0.16], true)?, false)?);
    for (hand, color) in [
        (Handedness::Left, Color::srgb(1.0, 0.5, 0.2)),
        (Handedness::Right, Color::srgb(0.25, 0.5, 1.0)),
    ] {
        commands.spawn((
            Wand(hand),
            Mesh3d(bar.clone()),
            VeMaterial {
                base_color: color,
                roughness: 0.35,
                ..VeMaterial::default()
            },
            Transform::from_scale(Vec3::ZERO),
        ));
    }
    Ok(())
}

/// Places each wand at its controller's grip in the world, scaled by the trigger; a controller that
/// is not tracked hides its wand.
pub(crate) fn update_wands(
    input: Res<XrInput>,
    rig: Res<XrRig>,
    mut wands: Query<(&Wand, &mut Transform)>,
) {
    for (Wand(hand), mut transform) in &mut wands {
        let controller = input
            .controllers
            .iter()
            .find(|controller| controller.handedness == *hand);
        *transform = match controller.and_then(|c| c.grip.map(|grip| (c, grip))) {
            Some((controller, grip)) => {
                let mut placed = Transform::from_matrix(rig.0 * grip);
                placed.scale = Vec3::splat(1.0 + TRIGGER_SWELL * controller.buttons[0]);
                placed
            }
            None => Transform::from_scale(Vec3::ZERO),
        };
    }
}
