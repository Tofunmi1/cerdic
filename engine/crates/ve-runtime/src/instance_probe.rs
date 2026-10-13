//! G4c representation probe: 10,000 selectable entities sharing one mesh asset.
use bevy_asset::Assets;
use bevy_camera::Camera3d;
use bevy_ecs::system::Commands;
use bevy_math::Vec3;
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::Transform;
use ve_kernel::{KernelError, Solid};
use ve_render::VeMaterial;

pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>) -> Result<(), KernelError> {
    let solid = Solid::cube([0.08; 3], true)?;
    let mesh = meshes.add(crate::scene::render_mesh(&solid, false)?);
    let bounds = solid.bounds()?;
    commands.insert_resource(crate::bench_scene::SceneStats {
        instances: 10_000,
        mesh_assets: 1,
        triangles: 120_000,
    });
    for x in 0..100_u16 {
        for z in 0..100_u16 {
            commands.spawn((
                Mesh3d(mesh.clone()),
                VeMaterial::default(),
                Transform::from_xyz(f32::from(x) * 0.12 - 6.0, 0.0, f32::from(z) * 0.12 - 6.0),
                crate::interact::Interactable::selection_only(&bounds),
            ));
        }
    }
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(8.0, 10.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    Ok(())
}
