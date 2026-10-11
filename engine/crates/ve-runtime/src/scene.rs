//! The M4 demo scene: a cube with a spherical bite taken out of it, built by the C++ geometry
//! kernel, and a camera looking at it. Replaced by authored scenes in later tracks.

use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::Camera3d;
use bevy_ecs::system::{Commands, ResMut};
use bevy_math::Vec3;
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_transform::components::Transform;
use ve_kernel::{KernelError, Solid};

use crate::status;

/// Builds cube-minus-sphere with the kernel and returns it as a render mesh with flat normals.
fn bitten_cube() -> Result<Mesh, KernelError> {
    let cube = Solid::cube([1.0, 1.0, 1.0], true)?;
    let sphere = Solid::sphere(0.65, 48)?;
    let kernel_mesh = cube.difference(&sphere)?.to_mesh()?;

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, kernel_mesh.positions);
    mesh.insert_indices(Indices::U32(kernel_mesh.indices));
    // Flat shading shows the cube's sharp edges; it needs unshared vertices.
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    Ok(mesh)
}

/// Startup system: spawns the demo mesh and a camera.
pub(crate) fn spawn_demo_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    match bitten_cube() {
        Ok(mesh) => {
            commands.spawn((Mesh3d(meshes.add(mesh)), Transform::IDENTITY));
        }
        Err(error) => status::report_error(format!("demo scene failed: {error}")),
    }
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.6, 1.3, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
