//! Spawning what interaction needs in a scene: tags that make objects pickable, and the lasers.

use bevy_asset::Assets;
use bevy_color::Color;
use bevy_ecs::system::Commands;
use bevy_math::Vec3;
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::Transform;
use ve_kernel::{KernelError, Solid};
use ve_render::VeMaterial;
use ve_xr_web::frame::Handedness;

use super::{Interactable, Label, Laser};
use crate::scene::render_mesh;

/// The components that make a solid's object pickable and grabbable, named `label`.
///
/// # Errors
/// Fails if the kernel cannot measure the solid.
pub(crate) fn tag(
    solid: &Solid,
    label: &'static str,
) -> Result<(Interactable, Label), KernelError> {
    Ok((
        Interactable::from_bounds(&solid.bounds()?),
        Label(label.to_owned()),
    ))
}

/// Spawns one thin laser bar per hand, hidden until a controller points somewhere.
///
/// # Errors
/// Fails if the kernel cannot build the bar.
pub(crate) fn spawn_lasers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
) -> Result<(), KernelError> {
    let bar = meshes.add(render_mesh(
        &Solid::cube([0.004, 0.004, 1.0], true)?,
        false,
    )?);
    for (hand, color) in [
        (Handedness::Left, Color::srgb(1.0, 0.5, 0.2)),
        (Handedness::Right, Color::srgb(0.25, 0.5, 1.0)),
    ] {
        commands.spawn((
            Laser(hand),
            Mesh3d(bar.clone()),
            VeMaterial {
                base_color: color,
                emissive: color,
                roughness: 1.0,
                ..VeMaterial::default()
            },
            Transform::from_scale(Vec3::ZERO),
        ));
    }
    Ok(())
}
