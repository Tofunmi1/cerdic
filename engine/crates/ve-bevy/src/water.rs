//! Water on Bevy's renderer (ADR 0059): Bevy's standard material extended with `shaders/water.wesl`, which draws flowing, depth-tinted, foamy
//! water from the flow, depth and foam the mesh carries in its vertex colour. No scene depth or screen copy is read, so it runs on WebGL2.

use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::{Asset, Assets, Handle, embedded_asset};
use bevy_color::{Color, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_math::Vec4;
use bevy_pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin, StandardMaterial};
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;
use bevy_shader::ShaderRef;
use ve_render::SurfaceWater;

/// The water parameters the shader reads: `shallow` is linear colour and clarity in metres, `deep` linear colour and the opacity of the
/// deepest water, `a` the flow speed multiplier, ripple strength and foam amount.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct WaterExtension {
    #[uniform(100)]
    shallow: Vec4,
    #[uniform(100)]
    deep: Vec4,
    #[uniform(100)]
    a: Vec4,
    #[uniform(100)]
    sim_bounds: Vec4,
    #[uniform(100)]
    spatial: [Vec4; 12],
    #[uniform(100)]
    temporal: [Vec4; 12],
    #[texture(101, filterable = false)]
    simulation: Option<Handle<Image>>,
}

impl MaterialExtension for WaterExtension {
    fn vertex_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/water.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/water.wesl".into()
    }
}

/// The standard material with the water shading.
pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

/// The shader parameters of a water surface; `opacity` is the material's opacity (the deepest water).
#[must_use]
pub fn water_extension(water: &SurfaceWater, opacity: f32) -> WaterExtension {
    let linear = |c: [f32; 3]| -> LinearRgba { Color::srgb(c[0], c[1], c[2]).to_linear() };
    let (shallow, deep) = (linear(water.shallow), linear(water.deep));
    let field = ve_render::water_waves::WaveField::new(
        water.seed,
        water.ripple,
        water.flow,
        water.direction,
    );
    let mut spatial = [Vec4::ZERO; 12];
    let mut temporal = [Vec4::ZERO; 12];
    for ((s, t), wave) in spatial.iter_mut().zip(&mut temporal).zip(field.waves()) {
        let k = std::f32::consts::TAU / wave.wavelength;
        let (z, x) = wave.direction_deg.to_radians().sin_cos();
        *s = Vec4::new(k * x, k * z, wave.amplitude, wave.phase);
        *t = Vec4::new(
            (ve_render::water_waves::GRAVITY * k).sqrt() * water.flow.clamp(0.0, 4.0),
            0.0,
            0.0,
            0.0,
        );
    }
    WaterExtension {
        shallow: Vec4::new(shallow.red, shallow.green, shallow.blue, water.clarity),
        deep: Vec4::new(deep.red, deep.green, deep.blue, opacity),
        a: Vec4::new(water.flow, water.ripple, water.foam, 0.0),
        sim_bounds: Vec4::ZERO,
        spatial,
        temporal,
        simulation: None,
    }
}

/// Adds the water material.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyWaterPlugin;

impl Plugin for VeBevyWaterPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/water.wesl");
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
        app.init_resource::<super::water_images::WaterImages>()
            .add_systems(PostUpdate, sync_water.before(bevy_asset::AssetEventSystems));
    }
}

fn sync_water(
    frames: Res<ve_render::water_sim::WaterSimTextures>,
    mut cache: ResMut<super::water_images::WaterImages>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    parts: Query<(
        &ve_render::VeMaterial,
        &bevy_pbr::MeshMaterial3d<WaterMaterial>,
    )>,
) {
    cache.update(&frames, &mut images);
    for (surface, handle) in &parts {
        let Some(water) = surface.finish.water else {
            continue;
        };
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        material.extension.a.w = surface.water_time;
        material.extension.simulation = cache.get(water.sim_id);
        material.extension.sim_bounds = if material.extension.simulation.is_some() {
            Vec4::from_array(water.sim_bounds)
        } else {
            Vec4::ZERO
        };
    }
}
