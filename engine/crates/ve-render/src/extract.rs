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

/// Replaces [`ExtractedMeshes`] with the visible `Mesh3d` entities of the main world.
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
            }),
    );
}
