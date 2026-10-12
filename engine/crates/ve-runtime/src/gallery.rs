//! The V0 comparison scene: the same authored dimensions and poses as the legacy starter scene.
use bevy_asset::Assets;
use bevy_camera::{Camera3d, PerspectiveProjection, Projection};
use bevy_color::Color;
use bevy_ecs::system::Commands;
use bevy_math::{Quat, Vec3};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::Transform;
use ve_kernel::{KernelError, Solid};
use ve_render::{VeDirectionalLight, VeMaterial};

use crate::{interact, scene::render_mesh};

pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>) -> Result<(), KernelError> {
    let objects = [
        (
            Solid::cube([12.0, 0.01, 12.0], true)?,
            "Studio floor",
            [0.0, -0.005, 0.0],
            0.0,
            [218, 225, 231],
        ),
        (
            Solid::cylinder(0.44, 1.3, 1.3, 64, true)?,
            "Pedestal",
            [0.0, 0.22, 0.0],
            0.0,
            [222, 229, 237],
        ),
        (
            Solid::cube([0.85, 1.1, 0.85], true)?,
            "Lavender block",
            [-0.75, 0.98, 0.0],
            0.22,
            [169, 155, 210],
        ),
        (
            Solid::cylinder(1.4, 0.36, 0.36, 64, true)?,
            "Coral column",
            [0.6, 1.12, 0.15],
            0.0,
            [231, 169, 141],
        ),
    ];
    for (solid, name, position, angle, rgb) in objects {
        commands.spawn((
            crate::projection::GalleryObject,
            Mesh3d(meshes.add(render_mesh(&solid, false)?)),
            VeMaterial {
                base_color: Color::srgb_u8(rgb[0], rgb[1], rgb[2]),
                roughness: 0.65,
                ..VeMaterial::default()
            },
            Transform::from_translation(Vec3::from_array(position))
                .with_rotation(Quat::from_rotation_y(angle)),
            interact::tag(&solid, name)?,
        ));
    }
    commands.spawn((
        VeDirectionalLight {
            color: Color::WHITE,
            intensity: 3.0,
        },
        Transform::IDENTITY.looking_to(Vec3::new(-0.4, -0.8, -0.45), Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 40.0_f32.to_radians(),
            ..PerspectiveProjection::default()
        }),
        Transform::from_xyz(6.0, 4.8, 8.0).looking_at(Vec3::new(0.0, 0.75, 0.0), Vec3::Y),
    ));
    Ok(())
}
