//! Bevy's own renderer as the engine's drawing path.
//!
//! The engine keeps its data model (parts, materials, lights, probes, wind) and this crate maps it onto Bevy's components, so the
//! effects Bevy ships (atmosphere, TAA, SSAO, bloom, tone mapping, shadows, irradiance volumes) work on our scenes without being
//! rewritten. See `spikes/bevy-renderer` for what runs on which backend.

mod ao;
mod camera;
mod environment;
mod exposure;
mod ground;
mod lens;
mod map;
mod maps;
mod probes;
mod sets;
mod stats;
mod stereo;
mod terrain;
mod toon;
mod units;
mod water;
mod water_images;

use bevy_anti_alias::AntiAliasPlugin;
use bevy_app::{PluginGroup, PluginGroupBuilder};
use bevy_core_pipeline::CorePipelinePlugin;
use bevy_light::LightPlugin;
use bevy_pbr::PbrPlugin;
use bevy_post_process::PostProcessPlugin;

pub use ao::VeBevyAoPlugin;
pub use camera::VeBevyCameraPlugin;
pub use environment::{VeBevyEnvironmentPlugin, cube_image, sky_cubemap, sky_radiance};
pub use exposure::VeBevyExposurePlugin;
pub use lens::VeBevyLensPlugin;
pub use map::{VeBevyMapPlugin, standard_material, sun_cascades};
pub use maps::{MapImages, map_image};
pub use probes::{VeBevyProbesPlugin, grid_of, volume_image};
pub use sets::VeBevySetsPlugin;
pub use stats::{BevyDrawStats, VeBevyStatsPlugin, latest};
pub use stereo::{EyeProjection, VeBevyStereoPlugin};
pub use terrain::{TerrainExtension, TerrainMaterial, VeBevyTerrainPlugin, terrain_extension};
pub use toon::{OutlineMaterial, ToonExtension, ToonMaterial, VeBevyToonPlugin, toon_extension};
pub use units::{
    EXPOSURE_EV100, MAX_SPOT_HALF_ANGLE, ambient_brightness, point_lumens, scene_unit_cd,
    spot_half_angles, sun_lux,
};
pub use water::{VeBevyWaterPlugin, WaterExtension, WaterMaterial, water_extension};

/// The plugins that give Bevy's renderer its passes, in the order they must be added: after `RenderPlugin`, and after the platform
/// plugin that owns the canvas. They include the mapping from our components.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyRenderer;

impl PluginGroup for VeBevyRenderer {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(LightPlugin)
            .add(CorePipelinePlugin)
            .add(PbrPlugin::default())
            .add(PostProcessPlugin)
            .add(AntiAliasPlugin)
            .add(VeBevyToonPlugin)
            .add(VeBevyTerrainPlugin)
            .add(VeBevyWaterPlugin)
            .add(VeBevyMapPlugin)
            .add(VeBevySetsPlugin)
            .add(VeBevyStatsPlugin)
            .add(VeBevyProbesPlugin)
            .add(VeBevyStereoPlugin)
            .add(VeBevyCameraPlugin)
            .add(VeBevyEnvironmentPlugin)
            .add(ve_render::VeReadinessPlugin)
            // The engine's own occlusion and exposure passes, which need the core pipeline's schedule to exist.
            .add(VeBevyAoPlugin)
            .add(VeBevyExposurePlugin)
            .add(VeBevyLensPlugin)
    }
}
