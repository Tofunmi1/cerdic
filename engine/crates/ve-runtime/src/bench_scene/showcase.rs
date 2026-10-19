//! The showcase scene: the benchmark room's colourful props (spheres and bitten cubes) under
//! studio lighting: a warm key sun with soft shadows, a cool sky fill, and coloured accent lights.

use bevy_asset::Assets;
use bevy_camera::Camera3d;
use bevy_color::Color;
use bevy_ecs::system::Commands;
use bevy_math::Vec3;
use bevy_mesh::Mesh;
use bevy_transform::components::Transform;
use ve_kernel::KernelError;
use ve_render::{VeAmbientLight, VeDirectionalLight, VePointLight};

use super::{BenchSpec, SceneStats, spawn_props, spawn_room};

/// About 80 props, a few thousand triangles each: smooth enough to show the lighting.
const PROPS: BenchSpec = BenchSpec {
    draws: 80,
    triangles: 240_000,
};

/// A warm key sun, then point lights: a cool overhead fill and four coloured accents low in the corners.
fn spawn_lights(commands: &mut Commands) {
    commands.spawn((
        VeDirectionalLight {
            color: Color::srgb(1.0, 0.93, 0.82),
            intensity: 2.8,
        },
        Transform::IDENTITY.looking_to(Vec3::new(-0.55, -0.7, -0.45), Vec3::Y),
    ));
    let points = [
        (Color::srgb(0.75, 0.85, 1.0), Vec3::new(0.0, 2.8, 0.0), 12.0),
        (
            Color::srgb(0.25, 0.75, 1.0),
            Vec3::new(-5.0, 1.4, -5.0),
            10.0,
        ),
        (
            Color::srgb(1.0, 0.45, 0.25),
            Vec3::new(5.0, 1.4, -5.0),
            10.0,
        ),
        (Color::srgb(0.8, 0.4, 1.0), Vec3::new(-5.0, 1.4, 5.0), 10.0),
        (Color::srgb(1.0, 0.8, 0.4), Vec3::new(5.0, 1.4, 5.0), 10.0),
    ];
    for (color, position, intensity) in points {
        commands.spawn((
            VePointLight {
                color,
                intensity,
                range: 9.0,
            },
            Transform::from_translation(position),
        ));
    }
}

/// Builds the room, the props, the lights and a camera standing at the room's edge.
///
/// # Errors
/// Fails if the kernel cannot build a shape.
pub(crate) fn showcase_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    ambient: &mut VeAmbientLight,
) -> Result<(), KernelError> {
    ambient.color = Color::srgb(0.8, 0.88, 1.0);
    ambient.intensity = 0.3;
    ambient.reflection = 1.0;
    let mut stats = SceneStats::default();
    spawn_room(commands, meshes, &mut stats, 6.0)?;
    spawn_props(commands, meshes, &mut stats, PROPS)?;
    spawn_lights(commands);
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.7, 6.2).looking_at(Vec3::new(0.0, 0.6, 0.0), Vec3::Y),
    ));
    Ok(())
}
