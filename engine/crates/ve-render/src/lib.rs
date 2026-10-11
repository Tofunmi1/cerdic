//! Render core for the engine.
//!
//! Built on the vendored Bevy render crates (`third_party/bevy`). Rendering is one ECS schedule per
//! camera, run by a driver inside the `RenderGraph` schedule. M4 adds the [`Forward3d`] schedule, mesh
//! extraction with instance batching, and a forward pipeline; materials and lights arrive in M5.

mod extract;
mod frame;
mod pass;
mod pipeline;
mod prepare;
mod schedule;

use bevy_app::{App, Plugin};
use bevy_asset::Assets;
use bevy_camera::{Camera3d, ClearColor};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_render::camera::CameraRenderGraph;
use bevy_render::extract_component::ExtractComponentPlugin;
use bevy_render::renderer::{RenderGraph, RenderGraphSystems};
use bevy_render::view::Msaa;
use bevy_render::{ExtractSchedule, Render, RenderApp, RenderSystems};
use bevy_shader::Shader;

pub use schedule::{Forward3d, Forward3dSystems};

/// Registers the engine's render systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeRenderPlugin;

impl Plugin for VeRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClearColor>()
            .register_required_components_with::<Camera3d, CameraRenderGraph>(|| {
                CameraRenderGraph::new(Forward3d)
            })
            // M4 renders straight into the output texture, so no multisampling yet (M5 adds it).
            .register_required_components_with::<Camera3d, Msaa>(|| Msaa::Off)
            .add_plugins(ExtractComponentPlugin::<Camera3d>::default());

        let shader = app
            .world_mut()
            .get_resource_mut::<Assets<Shader>>()
            .map(|mut shaders| {
                shaders.add(Shader::from_wgsl(
                    pipeline::FORWARD_SHADER,
                    "ve_forward.wgsl",
                ))
            });
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        if let Some(shader) = shader {
            render_app.insert_resource(pipeline::ForwardPipelines::new(shader));
        }
        render_app
            .init_resource::<extract::ExtractedMeshes>()
            .init_resource::<prepare::InstanceBatches>()
            .init_resource::<prepare::ViewUniforms>()
            .init_resource::<schedule::CameraWindows>()
            .add_schedule(Forward3d::base_schedule())
            .add_systems(ExtractSchedule, extract::extract_meshes)
            .add_systems(
                Render,
                (
                    prepare::prepare_instances,
                    prepare::prepare_view_uniforms,
                    prepare::prepare_depth_textures,
                )
                    .in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Render,
                pipeline::queue_forward_pipelines.in_set(RenderSystems::Queue),
            )
            .add_systems(
                Forward3d,
                pass::forward_pass.in_set(Forward3dSystems::MainPass),
            )
            .add_systems(
                RenderGraph,
                schedule::camera_driver.in_set(RenderGraphSystems::Render),
            )
            .add_systems(
                RenderGraph,
                (frame::submit_pending_command_buffers, frame::clear_windows)
                    .chain()
                    .in_set(RenderGraphSystems::Submit),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_into_an_app_without_a_renderer() {
        let mut app = App::new();
        app.add_plugins(VeRenderPlugin);
        assert!(app.world().contains_resource::<ClearColor>());
    }
}
