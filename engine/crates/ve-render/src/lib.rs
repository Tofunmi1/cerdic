//! Render core for the engine.
//!
//! Built on the vendored Bevy render crates (`third_party/bevy`). Rendering is one ECS schedule per
//! camera, run by a driver inside the `RenderGraph` schedule. The [`Forward3d`] schedule draws
//! instance-batched meshes with a metallic-roughness PBR shader, a sun, point lights, MSAA and
//! tonemapping; shadows and quality tiers come next (M6).

mod ao;
mod ao_depth;
mod counters;
mod extract;
mod placement;
pub use placement::VeInstanceTransform;
mod frame;
mod geometry;
mod grade;
mod lens;
mod light;
mod material;
mod pass;
mod pipeline;
mod prepare;
mod present;
mod quality;
mod readiness;
mod schedule;
#[cfg(test)]
mod shader_tests;
mod shadow;
mod shadow_math;
mod sky;
mod stereo;
mod targets;
mod textures;

use bevy_app::{App, Plugin, SubApp, Update};
use bevy_asset::Assets;
use bevy_camera::{Camera3d, ClearColor};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_render::camera::CameraRenderGraph;
use bevy_render::extract_component::ExtractComponentPlugin;
use bevy_render::extract_resource::ExtractResourcePlugin;
use bevy_render::renderer::{RenderGraph, RenderGraphSystems};
use bevy_render::view::Msaa;
use bevy_render::{ExtractSchedule, Render, RenderApp, RenderSystems};
use bevy_shader::Shader;

pub use counters::{FrameCounts, PassCounts, last_frame_counts};
pub use grade::{
    CONTRAST_RANGE, EXPOSURE_RANGE, GradePreset, Grading, SATURATION_RANGE, TINT_RANGE,
};
pub use lens::{LensMode, LensWarp};
pub use light::{
    MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS, VeAmbientLight, VeDirectionalLight, VePointLight,
    VeSpotLight,
};
pub use material::{ProceduralRecipe, ToonStyle, VeMaterial};
pub use quality::{QualityTier, RenderQuality};
pub use readiness::mesh_ready;
pub use sky::SkyBackground;
pub use textures::{LAYER_SIZE, MAX_LAYERS, MaterialTextures};
/// Per-vertex baked ambient visibility, stored as one float from zero to one.
pub const ATTRIBUTE_OCCLUSION: bevy_mesh::MeshVertexAttribute = bevy_mesh::MeshVertexAttribute::new(
    "Vertex_Occlusion",
    1_948_275_003,
    bevy_render::render_resource::VertexFormat::Float32,
);
pub use schedule::{Forward3d, Forward3dSystems};
pub use stereo::{StereoEye, StereoViews};

/// Registers the engine's render systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeRenderPlugin;

impl Plugin for VeRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClearColor>()
            .init_resource::<VeAmbientLight>()
            .init_resource::<RenderQuality>()
            .init_resource::<Grading>()
            .init_resource::<LensWarp>()
            .init_resource::<MaterialTextures>()
            .init_resource::<SkyBackground>()
            .register_required_components_with::<Camera3d, CameraRenderGraph>(|| {
                CameraRenderGraph::new(Forward3d)
            })
            .register_required_components_with::<Camera3d, Msaa>(|| Msaa::Sample4)
            .add_systems(Update, quality::apply_quality_to_cameras)
            .add_plugins((
                ExtractComponentPlugin::<Camera3d>::default(),
                ExtractComponentPlugin::<StereoViews>::default(),
                ExtractResourcePlugin::<SkyBackground>::default(),
            ));

        let shaders = app
            .world_mut()
            .get_resource_mut::<Assets<Shader>>()
            .map(|mut shaders| {
                (
                    shaders.add(Shader::from_wgsl(
                        pipeline::FORWARD_SHADER,
                        "ve_forward.wgsl",
                    )),
                    shaders.add(Shader::from_wgsl(
                        pipeline::forward_shader_without_uv(),
                        "ve_forward_without_uv.wgsl",
                    )),
                    shaders.add(Shader::from_wgsl(
                        present::PRESENT_SHADER,
                        present::PRESENT_SHADER_NAME,
                    )),
                    shaders.add(Shader::from_wgsl(
                        shadow::SHADOW_SHADER,
                        shadow::SHADOW_SHADER_NAME,
                    )),
                    shaders.add(Shader::from_wgsl(sky::SHADER, "ve_sky.wgsl")),
                    shaders.add(Shader::from_wgsl(
                        ao_depth::SHADER,
                        ao_depth::SHADER_NAME,
                    )),
                    shaders.add(Shader::from_wgsl(ao::SHADER, ao::SHADER_NAME)),
                )
            });
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        if let Some((
            forward,
            forward_without_uv,
            present,
            shadow,
            sky_shader,
            ao_depth_shader,
            ao_shader,
        )) = shaders
        {
            let forward = pipeline::ForwardPipelines::new(forward, forward_without_uv);
            let sky_pipelines = sky::SkyPipelines::new(sky_shader, forward.view_layout.clone());
            render_app
                .insert_resource(shadow::ShadowPipelines::new(
                    shadow,
                    forward.view_layout.clone(),
                ))
                .insert_resource(ao_depth::AoDepthPipelines::new(
                    ao_depth_shader,
                    forward.view_layout.clone(),
                ))
                .insert_resource(ao::AoPipeline::new(ao_shader))
                .insert_resource(forward)
                .insert_resource(sky_pipelines)
                .insert_resource(present::PresentPipelines::new(present));
        }
        add_render_systems(render_app);
    }
}

/// Resources and systems of the render world: extraction, preparation, queueing and the passes.
fn add_render_systems(render_app: &mut SubApp) {
    render_app
        .init_resource::<extract::ExtractedMeshes>()
        .init_resource::<light::ExtractedLights>()
        .init_resource::<prepare::InstanceBatches>()
        .init_resource::<prepare::ViewUniforms>()
        .init_resource::<prepare::LightsGpu>()
        .init_resource::<quality::RenderQuality>()
        .init_resource::<grade::Grading>()
        .init_resource::<lens::LensWarp>()
        .init_resource::<lens::LensGpu>()
        .init_resource::<shadow::ShadowMap>()
        .init_resource::<shadow::ShadowState>()
        .init_resource::<textures::ExtractedTextures>()
        .init_resource::<textures::TexturesGpu>()
        .init_resource::<schedule::CameraWindows>()
        .add_schedule(Forward3d::base_schedule())
        .add_systems(
            ExtractSchedule,
            (
                extract::extract_meshes,
                light::extract_lights,
                quality::extract_quality,
                grade::extract_grading,
                lens::extract_lens,
                textures::extract_textures,
            ),
        )
        .add_systems(
            Render,
            (
                prepare::prepare_instances,
                readiness::publish,
                prepare::prepare_view_uniforms,
                // The lights uniform includes the sun's shadow matrix and binds the shadow map.
                (
                    shadow::prepare_shadow_map,
                    shadow::prepare_shadow_state,
                    prepare::prepare_lights,
                )
                    .chain(),
                targets::prepare_depth_textures,
                targets::prepare_msaa_targets,
                targets::prepare_offscreen_targets,
                ao_depth::prepare_ao_depth_targets,
                present::prepare_present_sampler,
                textures::prepare_textures,
                lens::prepare_lens,
            )
                .in_set(RenderSystems::PrepareResources),
        )
        .add_systems(
            Render,
            (
                pipeline::queue_forward_pipelines,
                present::queue_present_pipelines,
                shadow::queue_shadow_pipelines,
                ao_depth::queue_ao_depth_pipelines,
                ao::queue_ao_pipeline,
                sky::queue,
            )
                .in_set(RenderSystems::Queue),
        )
        .add_systems(
            Forward3d,
            (
                pass::forward_pass.in_set(Forward3dSystems::MainPass),
                ao_depth::ao_depth_pass.in_set(Forward3dSystems::MainPass),
                ao::ao_pass
                    .in_set(Forward3dSystems::MainPass)
                    .after(ao_depth::ao_depth_pass),
                present::present_pass.in_set(Forward3dSystems::Present),
            ),
        )
        .add_systems(
            RenderGraph,
            (
                // The shadow map must be complete before any camera samples it.
                shadow::shadow_pass.before(schedule::camera_driver),
                schedule::camera_driver,
            )
                .in_set(RenderGraphSystems::Render),
        )
        .add_systems(
            RenderGraph,
            (frame::submit_pending_command_buffers, frame::clear_windows)
                .chain()
                .in_set(RenderGraphSystems::Submit),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_into_an_app_without_a_renderer() {
        let mut app = App::new();
        app.add_plugins(VeRenderPlugin);
        assert!(app.world().contains_resource::<ClearColor>());
        assert!(app.world().contains_resource::<VeAmbientLight>());
        assert!(app.world().contains_resource::<RenderQuality>());
        assert!(app.world().contains_resource::<Grading>());
    }
}
