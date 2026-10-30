//! The terrain surface on Bevy's renderer (ADR 0057): Bevy's standard material extended with `shaders/terrain.wesl`, which blends grass, sand,
//! rock and snow per pixel from the ground's slope and height. It keeps Bevy's lighting, shadows, fog and the AO and exposure passes.

use bevy_app::{App, Plugin, Update};
use bevy_asset::{Asset, Assets, io::embedded::EmbeddedAssetRegistry};
use bevy_color::{Color, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin, StandardMaterial};
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;
use ve_render::{RenderQuality, SurfaceTerrain};

/// The terrain parameters the shader reads, packed as vectors: `grass`, `rock`, `snow` and `sand` are linear colour and roughness; `a` is
/// the rock slope and blend and the snow line and blend; `b` the sand line and blend, the variation and the relief strength; `c` the relief scale.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct TerrainExtension {
    #[uniform(100)]
    grass: Vec4,
    #[uniform(100)]
    rock: Vec4,
    #[uniform(100)]
    snow: Vec4,
    #[uniform(100)]
    sand: Vec4,
    #[uniform(100)]
    a: Vec4,
    #[uniform(100)]
    b: Vec4,
    #[uniform(100)]
    c: Vec4,
}

impl TerrainExtension {
    pub(crate) fn set_sampling(&mut self, enabled: bool, scale: f32, quality: &RenderQuality) {
        self.c.z = if enabled { scale.clamp(0.25, 8.0) } else { 0.0 };
        self.c.w = if quality.stochastic_sampling_enabled() {
            1.0
        } else {
            0.0
        };
    }
}

impl MaterialExtension for TerrainExtension {
    fn fragment_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/terrain.wesl".into()
    }
}

/// The standard material with the terrain blend.
pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExtension>;

/// The shader parameters of a terrain surface.
#[must_use]
pub fn terrain_extension(terrain: &SurfaceTerrain) -> TerrainExtension {
    let layer = |index: usize| {
        let [r, g, b] = terrain.colors[index];
        let linear: LinearRgba = Color::srgb(r, g, b).to_linear();
        Vec4::new(
            linear.red,
            linear.green,
            linear.blue,
            terrain.roughness[index],
        )
    };
    TerrainExtension {
        grass: layer(0),
        rock: layer(1),
        snow: layer(2),
        sand: layer(3),
        a: Vec4::new(
            terrain.rock[0],
            terrain.rock[1],
            terrain.snow[0],
            terrain.snow[1],
        ),
        b: Vec4::new(
            terrain.sand[0],
            terrain.sand[1],
            terrain.variation,
            terrain.relief[0],
        ),
        c: Vec4::new(terrain.relief[1], 1.0, 0.0, 1.0),
    }
}

/// Adds the terrain material.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyTerrainPlugin;

impl Plugin for VeBevyTerrainPlugin {
    fn build(&self, app: &mut App) {
        // The shader cannot compose without `TerrainParams`; `shaders/terrain_params.wgsl` is a copy of the struct in ve-render's
        // `surface_types.wgsl` (that file also needs the grass types, so it cannot be embedded here). tests/terrain_shader.rs checks both.
        let source = concat!(
            include_str!("shaders/terrain.wesl"),
            include_str!("shaders/terrain_params.wgsl"),
            include_str!("../../ve-render/src/shaders/stochastic.wgsl"),
            include_str!("../../ve-render/src/shaders/forward_terrain.wgsl"),
        );
        app.world()
            .resource::<EmbeddedAssetRegistry>()
            .insert_asset(
                std::path::PathBuf::new(),
                std::path::Path::new("ve_bevy/shaders/terrain.wesl"),
                source.as_bytes(),
            );
        app.add_systems(Update, update_sampling_quality);
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
    }
}

fn update_sampling_quality(
    quality: Res<RenderQuality>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    if quality.is_changed() {
        for (_, material) in materials.iter_mut() {
            material.extension.c.w = if quality.stochastic_sampling_enabled() {
                1.0
            } else {
                0.0
            };
        }
    }
}
