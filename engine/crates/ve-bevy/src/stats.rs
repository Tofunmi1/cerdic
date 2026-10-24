//! Draw counts for entities visible to the main Bevy camera.

use std::collections::HashMap;
use std::sync::Mutex;

use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_camera::{
    Camera, Camera3d,
    visibility::{VisibilitySystems, VisibleEntities},
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_mesh::{Mesh, Mesh3d};
use ve_render::VeInstanceSet;

/// The latest Bevy renderer counts, also available to code outside the ECS.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BevyDrawStats {
    /// Visible mesh entities in the selected camera view.
    pub entities: u32,
    /// Triangles represented by those mesh entities.
    pub triangles: u64,
    /// Visible mesh entities parented by a `VeInstanceSet`.
    pub sets_entities: u32,
    /// Number of times the counting system has run.
    pub frame: u64,
}

static LATEST: Mutex<BevyDrawStats> = Mutex::new(BevyDrawStats {
    entities: 0,
    triangles: 0,
    sets_entities: 0,
    frame: 0,
});

/// Returns the most recently published Bevy draw counts.
pub fn latest() -> BevyDrawStats {
    *LATEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Resource, Default)]
struct MeshTriangleCache(HashMap<AssetId<Mesh>, u64>);

/// Publishes Bevy's visible mesh counts after camera visibility has been computed.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyStatsPlugin;

impl Plugin for VeBevyStatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BevyDrawStats>()
            .init_resource::<MeshTriangleCache>()
            .add_systems(
                PostUpdate,
                count_visible_meshes.after(VisibilitySystems::CheckVisibility),
            );
    }
}

fn mesh_triangles(mesh: &Mesh) -> u64 {
    mesh.indices()
        .map_or_else(|| mesh.count_vertices(), |indices| indices.len()) as u64
        / 3
}

fn count_visible_meshes(
    cameras: Query<(&Camera, &VisibleEntities), With<Camera3d>>,
    meshes: Query<(Entity, &Mesh3d, Option<&ChildOf>)>,
    parents: Query<(), With<VeInstanceSet>>,
    assets: Res<Assets<Mesh>>,
    mut cache: ResMut<MeshTriangleCache>,
    mut stats: ResMut<BevyDrawStats>,
) {
    if assets.is_changed() {
        cache.0.clear();
    }
    let Some((_, visible)) = cameras
        .iter()
        .filter(|(camera, _)| camera.is_active)
        .min_by_key(|(camera, _)| camera.order)
    else {
        *stats = BevyDrawStats {
            frame: stats.frame + 1,
            ..BevyDrawStats::default()
        };
        publish(*stats);
        return;
    };

    let mut next = BevyDrawStats {
        frame: stats.frame + 1,
        ..BevyDrawStats::default()
    };
    for entity in visible.iter(std::any::TypeId::of::<Mesh3d>()) {
        let Ok((_, handle, child_of)) = meshes.get(*entity) else {
            continue;
        };
        let Some(mesh) = assets.get(&handle.0) else {
            continue;
        };
        next.entities += 1;
        let id = handle.0.id();
        let triangles = *cache.0.entry(id).or_insert_with(|| mesh_triangles(mesh));
        next.triangles += triangles;
        if child_of.is_some_and(|parent| parents.contains(parent.parent())) {
            next.sets_entities += 1;
        }
    }
    *stats = next;
    publish(next);
}

fn publish(stats: BevyDrawStats) {
    *LATEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = stats;
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, Update};
    use bevy_asset::{Assets, Handle};
    use bevy_camera::{
        Camera, Camera3d,
        visibility::{SetViewVisibility, ViewVisibility, VisibleEntities},
    };
    use bevy_ecs::prelude::*;
    use bevy_mesh::{Mesh, Mesh3d, primitives::Meshable};
    use bevy_shape::Cuboid;
    use std::sync::Mutex;
    use ve_render::VeInstanceSet;

    use super::{BevyDrawStats, MeshTriangleCache, count_visible_meshes, latest};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn setup() -> (App, Handle<Mesh>) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(1.0, 1.0, 1.0).mesh());
        app.init_resource::<BevyDrawStats>();
        app.init_resource::<MeshTriangleCache>();
        app.add_systems(Update, count_visible_meshes);
        app.world_mut().spawn((
            Camera3d::default(),
            Camera::default(),
            VisibleEntities::default(),
        ));
        (app, mesh)
    }

    fn visible_entity(app: &mut App, mesh: Handle<Mesh>, visible: bool) -> Entity {
        let entity = app
            .world_mut()
            .spawn((Mesh3d(mesh), ViewVisibility::default()))
            .id();
        if visible {
            let mut query = app.world_mut().query::<&mut ViewVisibility>();
            query
                .get_mut(app.world_mut(), entity)
                .unwrap()
                .set_visible();
            let camera = app
                .world_mut()
                .query_filtered::<Entity, With<Camera3d>>()
                .single(app.world())
                .unwrap();
            app.world_mut()
                .get_mut::<VisibleEntities>(camera)
                .unwrap()
                .push(entity, std::any::TypeId::of::<Mesh3d>());
        }
        entity
    }

    #[test]
    fn counts_visible_meshes_only() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (mut app, mesh) = setup();
        visible_entity(&mut app, mesh.clone(), true);
        visible_entity(&mut app, mesh.clone(), true);
        visible_entity(&mut app, mesh, false);
        app.update();
        let stats = *app.world().resource::<BevyDrawStats>();
        assert_eq!(stats.entities, 2);
        assert_eq!(stats.triangles, 24);
    }

    #[test]
    fn set_copies_are_counted_separately() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (mut app, mesh) = setup();
        let parent = app
            .world_mut()
            .spawn(VeInstanceSet::new(
                mesh.clone(),
                Default::default(),
                Default::default(),
                (Default::default(), 1.0),
            ))
            .id();
        let copy = app.world_mut().spawn((Mesh3d(mesh), ChildOf(parent))).id();
        app.world_mut()
            .entity_mut(copy)
            .insert(ViewVisibility::default());
        {
            let mut query = app.world_mut().query::<&mut ViewVisibility>();
            query.get_mut(app.world_mut(), copy).unwrap().set_visible();
        }
        let camera = app
            .world_mut()
            .query_filtered::<Entity, With<Camera3d>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<VisibleEntities>(camera)
            .unwrap()
            .push(copy, std::any::TypeId::of::<Mesh3d>());
        app.update();
        let stats = *app.world().resource::<BevyDrawStats>();
        assert_eq!(stats.sets_entities, 1);
        assert_eq!(stats.entities, 1);
    }

    #[test]
    fn a_mesh_is_counted_once_per_copy() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (mut app, mesh) = setup();
        visible_entity(&mut app, mesh.clone(), true);
        visible_entity(&mut app, mesh, true);
        app.update();
        assert_eq!(app.world().resource::<BevyDrawStats>().triangles, 24);
    }

    #[test]
    fn latest_returns_what_the_system_wrote() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (mut app, mesh) = setup();
        visible_entity(&mut app, mesh, true);
        app.update();
        assert_eq!(latest(), *app.world().resource::<BevyDrawStats>());
    }
}
