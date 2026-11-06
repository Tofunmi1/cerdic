//! Mirrors the runtime's render components onto Bevy's: the runtime keeps spawning `VeMaterial` and `Ve*Light` entities, and these
//! systems give each one the Bevy component that draws it.

use crate::maps::MapImages;
use crate::terrain::{TerrainMaterial, terrain_extension};
use crate::toon::{DetailMaterial, ToonMaterial, detail_extension, needs_detail, toon_extension};
use crate::water::{WaterMaterial, water_extension};
use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bevy_asset::{Assets, Handle};
use bevy_color::{Alpha, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_light::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLight, GlobalAmbientLight,
    PointLight, SimulationLightSystems, SpotLight,
};
use bevy_material::AlphaMode;
use bevy_math::Vec3;
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_transform::components::Transform;
use ve_render::{
    MaterialTextures, ProbeVolume, RenderQuality, SkyBackground, VeAmbientLight,
    VeDirectionalLight, VeMaterial, VePointLight, VeSpotLight,
};

use crate::units;

/// Registers the mirroring systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyMapPlugin;

impl Plugin for VeBevyMapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight::NONE)
            .init_resource::<MapImages>()
            .add_systems(Update, refresh_live_maps)
            .add_systems(
                Update,
                (map_materials, map_lights, map_sky_sun, map_ambient)
                    .before(SimulationLightSystems::AssignLightsToClusters),
            );
    }
}

/// Gives the images of live textures (maps replaced in place, see `MaterialTextures::set_layer`) their new pixels each frame.
fn refresh_live_maps(
    textures: Res<MaterialTextures>,
    mut images: ResMut<Assets<Image>>,
    mut maps: ResMut<MapImages>,
) {
    maps.refresh(&textures, &mut images);
}

/// Bevy's material for one of ours. `map` turns a baked map id and whether it holds sRGB colour into its image.
#[must_use]
pub fn standard_material(
    surface: &VeMaterial,
    map: &mut dyn FnMut(i32, bool) -> Option<Handle<Image>>,
) -> StandardMaterial {
    let mut emissive = surface.emissive.to_linear();
    let cd = units::scene_unit_cd();
    let cutout = surface.foliage.alpha_cutoff > 0.0;
    let [albedo, normal, metallic_roughness, emissive_map] = surface.map_layers;
    let emissive_texture = map(emissive_map, true);
    if emissive_texture.is_some() {
        // The map carries the colour; the factor only scales it.
        emissive = LinearRgba::WHITE;
    }
    // A thin body (translucency or thickness) is seen from both sides.
    let thin = surface.finish.translucency > 0.0 || surface.finish.thickness > 0.0;
    StandardMaterial {
        base_color: surface.base_color.with_alpha(surface.opacity),
        base_color_texture: map(albedo, true),
        normal_map_texture: map(normal, false),
        metallic_roughness_texture: map(metallic_roughness, false),
        emissive_texture,
        metallic: surface.metallic,
        perceptual_roughness: surface.roughness.clamp(0.05, 1.0),
        emissive: LinearRgba::new(
            emissive.red * cd,
            emissive.green * cd,
            emissive.blue * cd,
            1.0,
        ),
        // Exposure scales the emissive back down by the same `cd`; with the default weight of 0 it would stay a thousand times too bright.
        emissive_exposure_weight: 1.0,
        // A particle (see `SurfaceFinish::particle`) is flat: its colour and opacity come from the mesh's vertex colours.
        unlit: surface.finish.particle > 0,
        alpha_mode: if surface.finish.particle == 2 {
            AlphaMode::Add
        } else if surface.finish.particle == 1 {
            AlphaMode::Blend
        } else if cutout {
            AlphaMode::Mask(surface.foliage.alpha_cutoff)
        } else if surface.opacity < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        clearcoat: surface.finish.clearcoat,
        clearcoat_perceptual_roughness: surface.finish.clearcoat_roughness.clamp(0.05, 1.0),
        anisotropy_strength: surface.finish.anisotropy,
        // Light passing through a thin body: leaves, wax, ears. It needs a double-sided surface to show on the far side.
        diffuse_transmission: surface.finish.translucency,
        double_sided: cutout || thin || surface.finish.particle > 0,
        cull_mode: if cutout || thin || surface.finish.particle > 0 {
            None
        } else {
            Some(bevy_render::render_resource::Face::Back)
        },
        ..StandardMaterial::default()
    }
}

fn map_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut toon_materials: ResMut<Assets<ToonMaterial>>,
    mut detail_materials: ResMut<Assets<DetailMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    quality: Res<RenderQuality>,
    mut images: ResMut<Assets<Image>>,
    mut maps: ResMut<MapImages>,
    textures: Res<MaterialTextures>,
    mut cache: Local<MaterialCache>,
    changed: Query<(Entity, &VeMaterial), Changed<VeMaterial>>,
    all: Query<(Entity, &VeMaterial)>,
    mut seen_effects: Local<Option<ve_render::Effects>>,
) {
    if textures.is_changed() {
        // The pictures behind the texture ids may have been replaced, so a cached material could point at stale ones.
        *cache = MaterialCache::default();
    }
    // Switching a surface effect on or off in the viewer changes how every surface is drawn, not only the ones that changed.
    let effects_changed = *seen_effects != Some(quality.effects);
    *seen_effects = Some(quality.effects);
    let work: Vec<(Entity, VeMaterial)> = if effects_changed {
        all.iter().map(|(entity, surface)| (entity, *surface)).collect()
    } else {
        changed.iter().map(|(entity, surface)| (entity, *surface)).collect()
    };
    for (entity, authored) in &work {
        let entity = *entity;
        // What is drawn is the authored surface minus the effects the viewer has switched off.
        let mut shown = *authored;
        shown.finish = quality.effects.filter(&shown.finish);
        if !quality.effects.maps {
            shown.map_layers = [-1; 4];
        }
        let surface = &shown;
        // Entities with an identical surface share ONE material asset: Bevy batches copies of a mesh into a single instanced draw only when
        // they also share a material, and a forest of 10,000 trees was 10,000 materials and about as many draw calls.
        // Time changes every frame; it is synchronized onto existing water assets in PostUpdate.
        let key = format!(
            "{:?}",
            VeMaterial {
                water_time: 0.0,
                ..*surface
            }
        );
        let material = standard_material(surface, &mut |id, srgb| {
            maps.get(&textures, &mut images, id, srgb)
        });
        // The entity can be despawned by the time this runs; a plain insert would panic Bevy's default error handler.
        let mut surface_entity = commands.entity(entity);
        // Frustum culling stays on for every part. A deformed part (CPU or GPU skinned) is the exception: its mesh leaves the box Bevy computed
        // when it first appeared, so the runtime marks those `NoFrustumCulling` itself where it makes them. (A blanket mark here made every
        // static tree and prop in a forest draw even when the camera looked at the sky: 35,000 draw calls a frame.)
        // Exactly one material component stays on the entity: every kind is taken away first, then the chosen one is put back (the commands
        // run in this order).
        surface_entity.try_remove::<(
            MeshMaterial3d<StandardMaterial>,
            MeshMaterial3d<ToonMaterial>,
            MeshMaterial3d<DetailMaterial>,
            MeshMaterial3d<TerrainMaterial>,
            MeshMaterial3d<WaterMaterial>,
        )>();
        if let Some(terrain) = surface.finish.terrain {
            let mut extension = terrain_extension(&terrain);
            extension.set_sampling(surface.no_repeat, surface.no_repeat_scale, &quality);
            let terrain = TerrainMaterial {
                base: material,
                extension,
            };
            let handle = cache
                .terrain
                .entry(key)
                .or_insert_with(|| terrain_materials.add(terrain))
                .clone();
            surface_entity.try_insert(MeshMaterial3d(handle));
        } else if let Some(water) = surface.finish.water {
            let water = WaterMaterial {
                extension: water_extension(&water, surface.opacity),
                base: material,
            };
            let handle = cache
                .water
                .entry(key)
                .or_insert_with(|| water_materials.add(water))
                .clone();
            surface_entity.try_insert(MeshMaterial3d(handle));
        } else if surface.toon.bands > 0 {
            let toon = ToonMaterial {
                base: material,
                extension: toon_extension(&surface.toon),
            };
            let handle = cache
                .toon
                .entry(key)
                .or_insert_with(|| toon_materials.add(toon))
                .clone();
            surface_entity.try_insert(MeshMaterial3d(handle));
        } else if needs_detail(&surface.finish) {
            let detail = DetailMaterial {
                base: material,
                extension: detail_extension(&surface.finish),
            };
            let handle = cache
                .detail
                .entry(key)
                .or_insert_with(|| detail_materials.add(detail))
                .clone();
            surface_entity.try_insert(MeshMaterial3d(handle));
        } else {
            let handle = cache
                .standard
                .entry(key)
                .or_insert_with(|| materials.add(material))
                .clone();
            surface_entity.try_insert(MeshMaterial3d(handle));
        }
    }
}

/// One material asset per distinct surface, shared by every entity that has it.
#[derive(Default)]
struct MaterialCache {
    standard: HashMap<String, Handle<StandardMaterial>>,
    toon: HashMap<String, Handle<ToonMaterial>>,
    detail: HashMap<String, Handle<DetailMaterial>>,
    terrain: HashMap<String, Handle<TerrainMaterial>>,
    water: HashMap<String, Handle<WaterMaterial>>,
}

fn map_lights(
    mut commands: Commands,
    quality: Res<RenderQuality>,
    suns: Query<(Entity, Ref<VeDirectionalLight>)>,
    points: Query<(Entity, &VePointLight), Changed<VePointLight>>,
    spots: Query<(Entity, &VeSpotLight), Changed<VeSpotLight>>,
) {
    for (entity, sun) in &suns {
        if !quality.is_changed() && !sun.is_changed() {
            continue;
        }
        commands.entity(entity).try_insert((
            DirectionalLight {
                color: sun.color,
                illuminance: units::sun_lux(sun.intensity),
                shadow_maps_enabled: true,
                ..DirectionalLight::default()
            },
            sun_cascades(&quality),
        ));
    }
    for (entity, point) in &points {
        commands.entity(entity).try_insert(PointLight {
            color: point.color,
            intensity: units::point_lumens(point.intensity),
            range: point.range,
            ..PointLight::default()
        });
    }
    for (entity, spot) in &spots {
        let (inner_angle, outer_angle) =
            units::spot_half_angles(spot.inner_angle, spot.outer_angle);
        commands.entity(entity).try_insert(SpotLight {
            color: spot.color,
            intensity: units::point_lumens(spot.intensity),
            range: spot.range,
            inner_angle,
            outer_angle,
            ..SpotLight::default()
        });
    }
}

/// Bevy's default reaches 150 m with the first cascade out to 5 m; a bounded range keeps shadows sharp. WebGL2's shaders hold one cascade
/// (`MAX_CASCADES_PER_LIGHT`): asking for more there makes the shader read the wrong cascade and the sun casts no shadows at all.
pub fn sun_cascades(quality: &RenderQuality) -> CascadeShadowConfig {
    let max_cascades = if cfg!(feature = "webgpu") { 4 } else { 1 };
    let num_cascades = quality.shadow_cascades.min(max_cascades) as usize;
    let maximum_distance = if num_cascades == 1 { 30.0 } else { 60.0 };
    CascadeShadowConfigBuilder {
        num_cascades,
        minimum_distance: 0.1,
        maximum_distance,
        first_cascade_far_bound: if num_cascades == 1 {
            maximum_distance
        } else {
            6.0
        },
        overlap_proportion: 0.2,
    }
    .build()
}

/// The sun that belongs to the sky, when the document has none of its own: Bevy's atmosphere takes its sun from a directional light, so a
/// sky with no authored sun light gets one aimed along the sky's own sun direction. A document with a sun light keeps it as the sun.
#[derive(Component)]
struct SkySun;

#[expect(
    clippy::float_cmp,
    reason = "change detection against the value this system wrote, not a computed result"
)]
fn map_sky_sun(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    quality: Res<RenderQuality>,
    authored: Query<(), With<VeDirectionalLight>>,
    mut existing: Query<
        (
            Entity,
            &mut DirectionalLight,
            &mut Transform,
            &mut CascadeShadowConfig,
        ),
        With<SkySun>,
    >,
) {
    let wanted = sky.enabled && authored.is_empty();
    let Ok((entity, mut light, mut transform, mut cascades)) = existing.single_mut() else {
        if wanted {
            commands.spawn((
                SkySun,
                sky_sun_light(&sky, &quality),
                sky_sun_transform(&sky),
                sun_cascades(&quality),
            ));
        }
        return;
    };
    if !wanted {
        commands.entity(entity).despawn();
        return;
    }
    if quality.is_changed() {
        *cascades = sun_cascades(&quality);
    }
    let desired = sky_sun_light(&sky, &quality);
    if light.illuminance != desired.illuminance
        || light.shadow_maps_enabled != desired.shadow_maps_enabled
    {
        *light = desired;
    }
    let aim = sky_sun_transform(&sky);
    if *transform != aim {
        *transform = aim;
    }
}

fn sky_sun_light(sky: &SkyBackground, quality: &RenderQuality) -> DirectionalLight {
    DirectionalLight {
        illuminance: units::sun_lux(sky.sun_intensity),
        shadow_maps_enabled: quality.shadows_enabled(),
        ..DirectionalLight::default()
    }
}

fn sky_sun_transform(sky: &SkyBackground) -> Transform {
    let direction = Vec3::from_array(sky.sun_direction);
    let up = if direction.dot(Vec3::Y).abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Transform::IDENTITY.looking_to(direction, up)
}

fn map_ambient(
    ambient: Res<VeAmbientLight>,
    sky: Res<SkyBackground>,
    probes: Res<ProbeVolume>,
    mut global: ResMut<GlobalAmbientLight>,
) {
    if !ambient.is_changed() && !sky.is_changed() && !probes.is_changed() {
        return;
    }
    // With a sky or baked probes, their light is the ambient: the flat light would count the same sky twice.
    if sky.enabled || probes.is_valid() {
        global.brightness = 0.0;
        return;
    }
    // Bevy's ambient light is flat. Ours is a sky and ground gradient; the average of the two is the flat light with the same total, and
    // the environment light (sky, probes) takes over the gradient.
    let (base, sky, ground) = (
        ambient.color.to_linear(),
        ambient.sky_color.to_linear(),
        ambient.ground_color.to_linear(),
    );
    let mean = |a: f32, b: f32, c: f32| a * (b + c) * 0.5;
    global.color = LinearRgba::rgb(
        mean(base.red, sky.red, ground.red),
        mean(base.green, sky.green, ground.green),
        mean(base.blue, sky.blue, ground.blue),
    )
    .into();
    global.brightness = units::ambient_brightness(ambient.intensity);
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
