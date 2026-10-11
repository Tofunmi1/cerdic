//! Engine status for the page: written by a system each frame, read through `status()`.

use std::cell::RefCell;

use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res};
use bevy_render::renderer::{RenderAdapterInfo, RenderDevice};
use bevy_transform::components::Transform;
use serde_json::json;
use ve_render::{RenderQuality, StereoViews};
use ve_xr_web::{XrCamera, XrInput};

use crate::bench_scene::SceneStats;
use crate::interact::{self, Interactable, InteractionState, Label};
use crate::perf::Perf;
use crate::viewer::ViewerRig;
use crate::wands::Wand;

thread_local! {
    static STATUS: RefCell<String> = RefCell::new(json!({ "frames": 0 }).to_string());
    static ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Records an error so the page can see it in `status()`.
pub(crate) fn report_error(message: String) {
    ERRORS.with(|errors| errors.borrow_mut().push(message));
}

/// What the headset camera currently sees: eye separation and the head position between the eyes.
fn xr_view_info(cameras: &Query<&StereoViews, With<XrCamera>>) -> serde_json::Value {
    let Some(stereo) = cameras.iter().next() else {
        return json!(null);
    };
    let left = stereo.eyes[0].world_from_view.w_axis.truncate();
    let right = stereo.eyes[1].world_from_view.w_axis.truncate();
    let head = (left + right) * 0.5;
    json!({
        "eye_separation_m": left.distance(right),
        "head_position": [head.x, head.y, head.z],
        "viewports": [stereo.eyes[0].viewport, stereo.eyes[1].viewport],
    })
}

/// What the renderer submitted last frame: draw calls and triangles per pass. The main pass counts
/// every view, so a stereo frame shows twice the draws of a mono one; the shadow pass is extra.
fn render_counts() -> serde_json::Value {
    let counts = ve_render::last_frame_counts();
    let pass = |c: ve_render::PassCounts| json!({ "draws": c.draws, "triangles": c.triangles });
    json!({ "main": pass(counts.main), "shadow": pass(counts.shadow) })
}

/// The controllers as the app sees them, with where each hand's wand ended up in the world.
fn controller_info(input: &XrInput, wands: &Query<(&Wand, &Transform)>) -> serde_json::Value {
    input
        .controllers
        .iter()
        .map(|controller| {
            let wand = wands
                .iter()
                .find(|(Wand(hand), _)| *hand == controller.handedness)
                .map(|(_, transform)| transform.translation.to_array());
            json!({
                "hand": format!("{:?}", controller.handedness),
                "tracked": controller.grip.is_some(),
                "trigger": controller.buttons[0],
                "squeeze": controller.buttons[1],
                "axes": controller.axes,
                "wand_position": wand,
            })
        })
        .collect()
}

/// Records adapter information, key limits, XR session state and a frame counter.
pub(crate) fn publish(
    mut frames: Local<u64>,
    adapter: Option<Res<RenderAdapterInfo>>,
    device: Option<Res<RenderDevice>>,
    xr_cameras: Query<&StereoViews, With<XrCamera>>,
    input: Res<XrInput>,
    wands: Query<(&Wand, &Transform)>,
    quality: Res<RenderQuality>,
    perf: Res<Perf>,
    scene_stats: Option<Res<SceneStats>>,
    interaction: Res<InteractionState>,
    viewer: Res<ViewerRig>,
    labelled: Query<(Entity, &Label, &Transform), With<Interactable>>,
) {
    *frames += 1;
    let errors = ERRORS.with(|errors| errors.borrow().clone());
    let mut value = json!({
        "frames": *frames,
        "errors": errors,
        "perf": perf.report(),
        "scene": scene_stats.as_deref().map(|s| json!({
            "instances": s.instances,
            "mesh_assets": s.mesh_assets,
            "triangles": s.triangles,
        })),
        "render": render_counts(),
        "interact": interact::report(&interaction, &labelled),
        "viewer": viewer.report(),
        "quality": {
            "tier": quality.tier.name(),
            "msaa": quality.msaa_samples,
            "shadow_resolution": quality.shadow_resolution,
            "max_point_lights": quality.max_point_lights,
            "resolution_scale": quality.resolution_scale,
        },
        "xr": {
            "state": format!("{:?}", ve_xr_web::session_state()),
            "frames": ve_xr_web::frames_published(),
            "error": ve_xr_web::last_error(),
            "view": xr_view_info(&xr_cameras),
            "controllers": controller_info(&input, &wands),
            "viewport_scale": {
                "requested": ve_xr_web::viewport_scale(),
                "supported": ve_xr_web::viewport_scale_supported(),
            },
            "last_frame": ve_xr_web::last_frame().map(|frame| json!({
                "views": frame.views.len(),
                "framebuffer": [frame.framebuffer_size.0, frame.framebuffer_size.1],
                "viewports": frame.views.iter().map(|v| v.viewport).collect::<Vec<_>>(),
                "projection_0": frame.views.first().map(|v| v.projection),
                "controllers": frame.controllers.len(),
            })),
        },
    });
    if let Some(info) = adapter {
        value["adapter"] = json!(info.name);
        value["backend"] = json!(format!("{:?}", info.backend));
    }
    if let Some(device) = device {
        let limits = device.limits();
        value["limits"] = json!({
            "max_texture_dimension_2d": limits.max_texture_dimension_2d,
            "max_uniform_buffer_binding_size": limits.max_uniform_buffer_binding_size,
            "max_bind_groups": limits.max_bind_groups,
            "max_vertex_buffers": limits.max_vertex_buffers,
            "max_color_attachments": limits.max_color_attachments,
        });
    }
    STATUS.with(|s| *s.borrow_mut() = value.to_string());
}

/// The most recent status JSON.
pub(crate) fn snapshot() -> String {
    STATUS.with(|s| s.borrow().clone())
}
