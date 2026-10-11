//! Render core for the engine.
//!
//! Built on the vendored Bevy render crates (`third_party/bevy`). Rendering is one ECS schedule per
//! camera, run by a driver inside the `RenderGraph` schedule. M3 installs the frame plumbing and a
//! clear pass for windows; the camera driver, extraction and pipelines arrive in M4.

mod frame;

use bevy_app::{App, Plugin};
use bevy_camera::ClearColor;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_render::RenderApp;
use bevy_render::renderer::{RenderGraph, RenderGraphSystems};

/// Registers the engine's render systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeRenderPlugin;

impl Plugin for VeRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClearColor>();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(
            RenderGraph,
            (frame::clear_windows, frame::submit_pending_command_buffers)
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
