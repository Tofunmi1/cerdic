//! Copying drawable meshes from the main world into the render world each frame.

use bevy_asset::AssetId;
use bevy_camera::visibility::InheritedVisibility;
use bevy_ecs::prelude::*;
use bevy_math::Mat4;
use bevy_mesh::{Mesh, Mesh3d};
use bevy_render::Extract;
use bevy_transform::components::GlobalTransform;

use crate::material::{PackedMaterial, VeMaterial};

/// One mesh instance as the renderer sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ExtractedMesh {
    pub(crate) mesh: AssetId<Mesh>,
    pub(crate) world_from_local: Mat4,
    pub(crate) material: PackedMaterial,
}

/// Every visible mesh instance this frame, rebuilt by [`extract_meshes`].
#[derive(Resource, Default, Debug)]
pub(crate) struct ExtractedMeshes(pub(crate) Vec<ExtractedMesh>);

/// Whether a model matrix can be drawn. It must be finite and keep handedness: back faces are
/// culled by winding, so a mirrored instance (negative determinant) would render inside out, and a
/// collapsed one (zero scale) has no surface to draw. Both are skipped, which makes hiding an
/// object with a zero scale a defined behaviour rather than an accident.
pub(crate) fn drawable(world_from_local: &Mat4) -> bool {
    if !world_from_local.is_finite() {
        return false;
    }
    let determinant = world_from_local.x_axis.truncate().dot(
        world_from_local
            .y_axis
            .truncate()
            .cross(world_from_local.z_axis.truncate()),
    );
    // Not zero, but small enough that a tiny uniform scale (a 1 mm object) still draws.
    determinant > 1e-12
}

/// Replaces [`ExtractedMeshes`] with the visible, drawable `Mesh3d` entities of the main world.
pub(crate) fn extract_meshes(
    mut out: ResMut<ExtractedMeshes>,
    meshes: Extract<
        Query<(
            &Mesh3d,
            &GlobalTransform,
            Option<&VeMaterial>,
            Option<&InheritedVisibility>,
        )>,
    >,
) {
    out.0.clear();
    out.0.extend(
        meshes
            .iter()
            .filter(|(_, _, _, visibility)| visibility.is_none_or(|v| v.get()))
            .map(|(mesh, transform, material, _)| ExtractedMesh {
                mesh: mesh.id(),
                world_from_local: transform.to_matrix(),
                material: material.copied().unwrap_or_default().pack(),
            })
            .filter(|mesh| drawable(&mesh.world_from_local)),
    );
}

#[cfg(test)]
mod tests {
    use bevy_math::{Quat, Vec3};

    use super::*;

    fn model(scale: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            scale,
            Quat::from_rotation_y(0.7),
            Vec3::new(1.0, 2.0, 3.0),
        )
    }

    #[test]
    fn ordinary_and_nonuniform_scales_are_drawn() {
        assert!(drawable(&model(Vec3::ONE)));
        assert!(drawable(&model(Vec3::new(2.0, 1.0, 0.5))));
        assert!(drawable(&model(Vec3::splat(0.001))));
    }

    #[test]
    fn collapsed_mirrored_and_non_finite_instances_are_skipped() {
        assert!(!drawable(&model(Vec3::ZERO)), "zero scale hides an object");
        assert!(!drawable(&model(Vec3::new(1.0, 0.0, 1.0))));
        assert!(!drawable(&model(Vec3::new(-1.0, 1.0, 1.0))), "mirrored");
        assert!(!drawable(&model(Vec3::new(f32::NAN, 1.0, 1.0))));
        assert!(!drawable(&model(Vec3::new(f32::INFINITY, 1.0, 1.0))));
    }

    /// The shader's normal matrix is the cofactor matrix; for drawable transforms it must point the
    /// same way as the inverse transpose, which is what keeps lighting right under non-uniform scale.
    #[test]
    fn the_cofactor_normal_matrix_matches_the_inverse_transpose() {
        let m = model(Vec3::new(2.0, 1.0, 0.5));
        let (c0, c1, c2) = (
            m.x_axis.truncate(),
            m.y_axis.truncate(),
            m.z_axis.truncate(),
        );
        let cofactor = bevy_math::Mat3::from_cols(c1.cross(c2), c2.cross(c0), c0.cross(c1));
        let inverse_transpose = bevy_math::Mat3::from_mat4(m).inverse().transpose();
        for normal in [Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0).normalize()] {
            let a = (cofactor * normal).normalize();
            let b = (inverse_transpose * normal).normalize();
            assert!(a.abs_diff_eq(b, 1e-5), "{a} vs {b}");
        }
        // A surface normal stays perpendicular to the surface when the surface is stretched.
        let tangent = Vec3::new(1.0, -1.0, 0.0);
        let normal = Vec3::new(1.0, 1.0, 0.0);
        let moved_tangent = bevy_math::Mat3::from_mat4(m) * tangent;
        let moved_normal = cofactor * normal;
        assert!(moved_tangent.dot(moved_normal).abs() < 1e-4);
    }
}
