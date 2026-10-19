//! Assembles the plugins into the app that the page runs.

use bevy_app::{App, Startup, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_camera::{CameraPlugin, ClearColor};
use bevy_color::Color;
use bevy_diagnostic::FrameCountPlugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_image::ImagePlugin;
use bevy_input::InputPlugin;
use bevy_log::LogPlugin;
use bevy_mesh::MeshPlugin;
use bevy_render::RenderPlugin;
use bevy_time::TimePlugin;
use bevy_transform::TransformPlugin;
use bevy_window::WindowPlugin;
use ve_platform_web::{PlatformError, WebPlatformPlugin};
use ve_render::VeRenderPlugin;
use ve_xr_web::{XrPlugin, apply_xr_frame};

use crate::scene::{self, SceneKind};
use crate::viewer::ViewerRig;
use crate::{grading, interact, mock_hmd};
use crate::{perf, quality, status, wands};

/// The clear colour the golden tests expect on the canvas (sRGB 26, 51, 102).
const CLEAR: Color = Color::srgb(0.10, 0.20, 0.40);

/// Builds and runs the app. Returns once the browser runner has taken over the frame loop.
pub(crate) fn start(canvas_id: &str, scene: SceneKind) -> Result<(), PlatformError> {
    let platform = WebPlatformPlugin::new(canvas_id)?;
    let mut app = App::new();
    app.add_plugins((
        // Bevy reports pipeline and shader errors only through its logger; without this they are silent.
        LogPlugin::default(),
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
        XrPlugin,
    ));
    app.insert_resource(ClearColor(CLEAR));
    app.insert_resource(bevy_render::render_asset::RenderAssetBytesPerFrame::new(
        8 * 1024 * 1024,
    ));
    app.insert_resource(scene);
    app.init_resource::<perf::Perf>();
    app.init_resource::<ViewerRig>();
    app.init_resource::<scene::SceneAmbient>();
    app.init_resource::<crate::animation::AnimationClock>();
    app.init_resource::<crate::animation::PartPoses>();
    app.init_resource::<crate::physics::Physics>();
    interact::plugin(&mut app);
    app.add_systems(Startup, scene::spawn_scene);
    app.add_systems(Update, crate::fx::step);
    app.add_systems(Startup, scene::remember_ambient.after(scene::spawn_scene));
    // The frame's order: the XR frame is applied first (`apply_xr_frame`), then everything that
    // reads head or controller poses (wands and the interaction chain) sees that same frame, and
    // the status report is written last so it describes what this frame did.
    app.add_systems(
        Update,
        (
            wands::update_wands.after(apply_xr_frame),
            crate::projection::update.before(interact::run_interaction),
            status::publish
                .after(wands::update_wands)
                .after(interact::highlight_hover),
            mock_hmd::update_mock_hmd,
            crate::studio::scale_room,
            quality::apply_requested,
            quality::publish_frame_delta,
            grading::apply_requested,
            quality::sync_xr_scale.after(quality::apply_requested),
            perf::track_frame_time,
            (
                crate::animation::apply_requested,
                crate::animation::advance,
                crate::behavior::xr_toggle,
                crate::behavior::intake,
                crate::behavior::proximity,
                crate::behavior::collect_grabs,
                crate::behavior::sync_grabbable,
                crate::behavior::run,
                crate::physics::run,
                crate::animation::animate,
                crate::animation::apply_glow,
                crate::animation::apply_visibility,
                crate::animation::apply_color,
                crate::animation::animate_lights,
                crate::animation::animate_instances,
                crate::skinning::skin_meshes,
            )
                .chain()
                .after(crate::projection::update),
        ),
    );
    app.run();
    Ok(())
}
