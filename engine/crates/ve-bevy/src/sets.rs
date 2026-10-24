//! Instance sets as plain entities. The engine's own renderer draws a `VeInstanceSet` from one retained buffer with chunk culling; Bevy
//! batches and culls entities itself, so each copy becomes a child entity per level of detail, with all children sharing the set's
//! material. Per-copy colour variation of the old path is not carried over yet.

use bevy_app::{App, Plugin, Update};
use bevy_camera::visibility::{VisibilityRange, VisibilityRangePlugin};
use bevy_ecs::prelude::*;
use bevy_mesh::Mesh3d;
use bevy_pbr::{Material, MeshMaterial3d, StandardMaterial};
use bevy_transform::components::Transform;
use ve_render::VeInstanceSet;

use crate::terrain::TerrainMaterial;
use crate::toon::{DetailMaterial, ToonMaterial};
use crate::water::WaterMaterial;

/// Registers the instance set systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevySetsPlugin;

impl Plugin for VeBevySetsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<VisibilityRangePlugin>() {
            app.add_plugins(VisibilityRangePlugin);
        }
        app.add_systems(
            Update,
            (
                map_sets,
                share_set_material::<StandardMaterial>.after(map_sets),
                share_set_material::<ToonMaterial>.after(map_sets),
                share_set_material::<DetailMaterial>.after(map_sets),
                share_set_material::<TerrainMaterial>.after(map_sets),
                share_set_material::<WaterMaterial>.after(map_sets),
            ),
        );
    }
}

/// Replaces the copies of every set that changed: despawns the old children and spawns one per transform and level.
fn map_sets(
    mut commands: Commands,
    sets: Query<(Entity, &VeInstanceSet, Option<&Children>), Changed<VeInstanceSet>>,
) {
    for (entity, set, children) in &sets {
        for child in children.into_iter().flatten() {
            commands.entity(*child).try_despawn();
        }
        for matrix in set.transforms.iter() {
            let mut start = set.min_distance;
            for level in &set.meshes {
                commands.spawn((
                    Mesh3d(level.mesh.clone()),
                    Transform::from_matrix(*matrix),
                    ChildOf(entity),
                    level_range(start, level.max_distance, set.fade),
                ));
                start = level.max_distance;
            }
        }
    }
}

fn level_range(start: f32, end: f32, fade: f32) -> VisibilityRange {
    if fade <= 0.0 {
        return VisibilityRange::abrupt(start, end);
    }
    VisibilityRange {
        start_margin: if start == 0.0 {
            0.0..0.0
        } else {
            (start - fade * 0.5)..(start + fade * 0.5)
        },
        end_margin: if end.is_infinite() {
            0.0..0.0
        } else {
            (end - fade * 0.5)..(end + fade * 0.5)
        },
        use_aabb: false,
    }
}

/// Gives the copies of a set the set's material whenever either changed. The material can be any kind the mapping chooses (standard,
/// toon, relief, terrain or water), so each copy first loses the kinds it does not use.
fn share_set_material<M: Material + Kind>(
    mut commands: Commands,
    sets: Query<
        (&MeshMaterial3d<M>, &Children),
        (
            With<VeInstanceSet>,
            Or<(Changed<MeshMaterial3d<M>>, Changed<Children>)>,
        ),
    >,
) {
    for (material, children) in &sets {
        for child in children {
            commands.entity(*child).try_remove::<OtherKinds<M>>();
            commands.entity(*child).try_insert(material.clone());
        }
    }
}

/// The material components a copy must not keep when it takes a material of kind `M` (all the kinds except `M`).
type OtherKinds<M> = <M as Kind>::Others;

/// Which other material kinds exist, as a removable bundle, for each kind the mapping can choose.
trait Kind {
    type Others: Bundle;
}

macro_rules! kinds {
    ($($kind:ty => ($($other:ty),*)),* $(,)?) => {
        $(impl Kind for $kind { type Others = ($(MeshMaterial3d<$other>,)*); })*
    };
}

kinds! {
    StandardMaterial => (ToonMaterial, DetailMaterial, TerrainMaterial, WaterMaterial),
    ToonMaterial => (StandardMaterial, DetailMaterial, TerrainMaterial, WaterMaterial),
    DetailMaterial => (StandardMaterial, ToonMaterial, TerrainMaterial, WaterMaterial),
    TerrainMaterial => (StandardMaterial, ToonMaterial, DetailMaterial, WaterMaterial),
    WaterMaterial => (StandardMaterial, ToonMaterial, DetailMaterial, TerrainMaterial),
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, Update};
    use bevy_asset::Handle;
    use bevy_camera::visibility::VisibilityRange;
    use bevy_ecs::prelude::*;
    use bevy_math::Mat4;
    use bevy_mesh::{Mesh, Mesh3d};
    use bevy_transform::components::Transform;
    use ve_render::{SetMesh, VeInstanceSet};

    use super::map_sets;

    fn set(copies: usize, distances: &[f32]) -> VeInstanceSet {
        VeInstanceSet {
            min_distance: 0.0,
            fade: 0.0,
            meshes: distances
                .iter()
                .map(|&max_distance| SetMesh {
                    mesh: Default::default(),
                    max_distance,
                })
                .collect(),
            transforms: vec![Mat4::IDENTITY; copies].into(),
            indices: vec![0; copies].into(),
            bounds_centre: Default::default(),
            bounds_radius: 1.0,
        }
    }

    fn app_with_set(set: VeInstanceSet) -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, map_sets);
        let entity = app.world_mut().spawn(set).id();
        app.update();
        (app, entity)
    }

    fn children(app: &mut App, parent: Entity) -> Vec<Entity> {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ChildOf)>();
        query
            .iter(world)
            .filter_map(|(entity, child_of)| (child_of.parent() == parent).then_some(entity))
            .collect()
    }

    #[test]
    fn three_levels_make_three_children_per_copy() {
        let (mut app, parent) = app_with_set(set(2, &[20.0, 80.0, f32::INFINITY]));
        assert_eq!(children(&mut app, parent).len(), 6);
    }

    #[test]
    fn levels_cover_consecutive_distance_bands() {
        let (mut app, parent) = app_with_set(set(1, &[20.0, 80.0, f32::INFINITY]));
        let world = app.world_mut();
        let mut query = world.query::<(&ChildOf, &VisibilityRange)>();
        let mut bands = query
            .iter(world)
            .filter(|(child_of, _)| child_of.parent() == parent)
            .map(|(_, range)| (range.start_margin.start, range.end_margin.start))
            .collect::<Vec<_>>();
        bands.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(
            bands,
            vec![(0.0, 20.0), (20.0, 80.0), (80.0, f32::INFINITY)]
        );
    }

    #[test]
    fn finite_level_edges_cross_fade_symmetrically() {
        let mut set = set(1, &[40.0, 130.0]);
        set.fade = 10.0;
        let (mut app, parent) = app_with_set(set);
        let world = app.world_mut();
        let mut query = world.query::<(&ChildOf, &VisibilityRange)>();
        let mut ranges = query
            .iter(world)
            .filter(|(child_of, _)| child_of.parent() == parent)
            .map(|(_, range)| {
                (
                    range.start_margin.start,
                    range.start_margin.end,
                    range.end_margin.start,
                    range.end_margin.end,
                )
            })
            .collect::<Vec<_>>();
        ranges.sort_by(|a, b| a.2.total_cmp(&b.2));
        assert_eq!(
            ranges,
            vec![(0.0, 0.0, 35.0, 45.0), (35.0, 45.0, 125.0, 135.0)]
        );
    }

    #[test]
    fn zero_fade_keeps_abrupt_ranges() {
        let (mut app, parent) = app_with_set(set(1, &[40.0, 130.0]));
        let world = app.world_mut();
        let mut query = world.query::<(&ChildOf, &VisibilityRange)>();
        let mut ranges = query
            .iter(world)
            .filter(|(child_of, _)| child_of.parent() == parent)
            .map(|(_, range)| {
                (
                    range.start_margin.start,
                    range.start_margin.end,
                    range.end_margin.start,
                    range.end_margin.end,
                )
            })
            .collect::<Vec<_>>();
        ranges.sort_by(|a, b| a.2.total_cmp(&b.2));
        assert_eq!(
            ranges,
            vec![(0.0, 0.0, 40.0, 40.0), (40.0, 40.0, 130.0, 130.0)]
        );
    }

    #[test]
    fn a_set_starts_at_its_min_distance() {
        let mut one = set(1, &[140.0]);
        one.min_distance = 45.0;
        let (mut app, parent) = app_with_set(one);
        let world = app.world_mut();
        let mut query = world.query::<(&ChildOf, &VisibilityRange)>();
        let bands = query
            .iter(world)
            .filter(|(child_of, _)| child_of.parent() == parent)
            .map(|(_, range)| (range.start_margin.start, range.end_margin.start))
            .collect::<Vec<_>>();
        assert_eq!(bands, vec![(45.0, 140.0)]);

        let mut two = set(1, &[20.0, 80.0]);
        two.min_distance = 5.0;
        let (mut app, parent) = app_with_set(two);
        let world = app.world_mut();
        let mut query = world.query::<(&ChildOf, &VisibilityRange)>();
        let mut bands = query
            .iter(world)
            .filter(|(child_of, _)| child_of.parent() == parent)
            .map(|(_, range)| (range.start_margin.start, range.end_margin.start))
            .collect::<Vec<_>>();
        bands.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(bands, vec![(5.0, 20.0), (20.0, 80.0)]);
    }

    #[test]
    fn a_single_level_set_is_unchanged() {
        let (mut app, parent) = app_with_set(set(5, &[f32::INFINITY]));
        let world = app.world_mut();
        let mut query = world.query::<(&ChildOf, &Mesh3d, &Transform, &VisibilityRange)>();
        let copies = query
            .iter(world)
            .filter(|(child_of, _, _, _)| child_of.parent() == parent)
            .collect::<Vec<_>>();
        assert_eq!(copies.len(), 5);
        assert!(copies.iter().all(|(_, mesh, _, range)| {
            mesh.0 == Handle::<Mesh>::default()
                && range.start_margin.start == 0.0
                && range.end_margin.start == f32::INFINITY
        }));
    }

    #[test]
    fn replacing_the_set_replaces_the_children() {
        let (mut app, parent) = app_with_set(set(2, &[20.0, 80.0, f32::INFINITY]));
        let old = children(&mut app, parent);
        *app.world_mut()
            .entity_mut(parent)
            .get_mut::<VeInstanceSet>()
            .unwrap() = set(1, &[20.0, 80.0]);
        app.update();
        let current = children(&mut app, parent);
        assert_eq!(current.len(), 2);
        assert!(
            old.iter()
                .all(|entity| !app.world().entities().contains(*entity))
        );
    }
}
