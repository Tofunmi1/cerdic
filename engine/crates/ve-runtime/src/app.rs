//! Assembles the plugins into the app that the page runs.

use bevy_app::{App, Startup, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_camera::CameraPlugin;
use bevy_camera::ClearColor;
use bevy_color::Color;
use bevy_diagnostic::FrameCountPlugin;
use bevy_image::ImagePlugin;
use bevy_input::InputPlugin;
use bevy_mesh::MeshPlugin;
use bevy_render::RenderPlugin;
use bevy_time::TimePlugin;
use bevy_transform::TransformPlugin;
use bevy_window::WindowPlugin;
use ve_platform_web::{PlatformError, WebPlatformPlugin};
use ve_render::VeRenderPlugin;

use crate::{scene, status};

/// The colour M3's golden test expects on the canvas (sRGB 26, 51, 102).
const CLEAR: Color = Color::srgb(0.10, 0.20, 0.40);

/// Builds and runs the app. Returns once the browser runner has taken over the frame loop.
pub(crate) fn start(canvas_id: &str) -> Result<(), PlatformError> {
    let platform = WebPlatformPlugin::new(canvas_id)?;
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        TimePlugin,
        FrameCountPlugin,
        InputPlugin,
        WindowPlugin {
            primary_window: None,
            ..WindowPlugin::default()
        },
        // The platform plugin must precede the renderer: the renderer reads the window handle.
        platform,
        AssetPlugin::default(),
        // Asset types the renderer extracts; they must exist before `RenderPlugin` builds.
        ImagePlugin::default(),
        MeshPlugin,
        TransformPlugin,
        CameraPlugin,
        RenderPlugin::default(),
        VeRenderPlugin,
    ));
    app.insert_resource(ClearColor(CLEAR));
    app.add_systems(Startup, scene::spawn_demo_scene);
    app.add_systems(Update, status::publish);
    app.run();
    Ok(())
}
