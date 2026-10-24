//! The ground beyond the scene: with a sky, a flat plane out to the horizon in the sky's ground colour, so land that the author never
//! modelled (the edge of a terrain tile, the end of a street) fades into haze like a real horizon instead of showing the atmosphere's
//! bare planet. It sits a little below zero so terrain and floors always cover it.

use bevy_asset::Assets;
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_light::NotShadowCaster;
use bevy_mesh::{Mesh, Mesh3d, Meshable};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_shape::Plane3d;
use bevy_transform::components::Transform;
use ve_render::SkyBackground;

/// Marks the horizon plane.
#[derive(Component)]
pub(crate) struct SkyGround;

/// Half the plane's side, in metres: far enough that fog and the sky's haze hide its edge.
const EXTENT: f32 = 60_000.0;

/// Keeps the horizon plane in step with the sky: spawned when a sky is on, recoloured when it changes, removed when it goes.
pub(crate) fn map_ground(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<SkyGround>>,
) {
    if !sky.is_changed() {
        return;
    }
    if !sky.enabled {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    }
    let [r, g, b] = sky.ground;
    let colour = Color::srgb(r, g, b);
    if let Some((_, material)) = existing.iter().next() {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.base_color = colour;
        }
        return;
    }
    let mesh = meshes.add(Plane3d::default().mesh().size(EXTENT * 2.0, EXTENT * 2.0));
    let material = materials.add(StandardMaterial {
        base_color: colour,
        perceptual_roughness: 1.0,
        reflectance: 0.04,
        ..StandardMaterial::default()
    });
    commands.spawn((
        SkyGround,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, -1.5, 0.0),
        NotShadowCaster,
    ));
}
