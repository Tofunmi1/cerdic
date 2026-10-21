//! The window runtime can swap a candidate only after all of its meshes are on the GPU.
use bevy_asset::AssetId;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Res;
use bevy_mesh::Mesh;
use bevy_render::{mesh::RenderMesh, render_asset::RenderAssets};
use std::cell::RefCell;
use std::collections::BTreeSet;

thread_local! { static READY: RefCell<BTreeSet<AssetId<Mesh>>> = const { RefCell::new(BTreeSet::new()) }; }

/// Whether the render world has prepared this mesh. Publication follows asset preparation.
#[must_use]
pub fn mesh_ready(id: AssetId<Mesh>) -> bool {
    READY.with(|set| set.borrow().contains(&id))
}

/// Publishes which meshes the render world has prepared, for a renderer other than the engine's own forward pass (that one registers
/// the same system itself).
#[derive(Debug, Default, Clone, Copy)]
pub struct VeReadinessPlugin;

impl bevy_app::Plugin for VeReadinessPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        if let Some(render_app) = app.get_sub_app_mut(bevy_render::RenderApp) {
            render_app.add_systems(
                bevy_render::Render,
                publish.in_set(bevy_render::RenderSystems::PrepareResources),
            );
        }
    }
}

pub(crate) fn publish(meshes: Res<RenderAssets<RenderMesh>>) {
    READY.with(|set| *set.borrow_mut() = meshes.iter().map(|(id, _)| id).collect());
}
