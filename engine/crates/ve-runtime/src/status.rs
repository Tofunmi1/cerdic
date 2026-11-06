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
    let forward = stereo.eyes[0]
        .world_from_view
        .transform_vector3(bevy_math::Vec3::NEG_Z);
    json!({
        "eye_separation_m": left.distance(right),
        "head_position": [head.x, head.y, head.z],
        // Which way the head faces, as a turn about up in radians (0 looks down -z; forward is (-sin, -cos)).
        "head_yaw": (-forward.x).atan2(-forward.z),
        "viewports": [stereo.eyes[0].viewport, stereo.eyes[1].viewport],
    })
}

/// What the renderer submitted last frame: draw calls and triangles per pass. The main pass counts
/// every view, so a stereo frame shows twice the draws of a mono one; the shadow pass and the depth pass (only while
/// screen-space occlusion is on) are extra.
fn render_counts() -> serde_json::Value {
    let counts = ve_render::last_frame_counts();
    let pass = |c: ve_render::PassCounts| json!({ "draws": c.draws, "triangles": c.triangles });
    let sets = ve_render::last_set_stats();
    let mut result = json!({
        "main": pass(counts.main), "shadow": pass(counts.shadow), "depth": pass(counts.depth),
        // Retained instance sets: how many, their copies and chunks, and what the camera's culling kept.
        "sets": {
            "sets": sets.sets, "copies": sets.copies, "chunks": sets.chunks,
            "chunks_visible": sets.chunks_visible, "copies_visible": sets.copies_visible,
        },
    });
    #[cfg(any(feature = "bevy-webgl", feature = "bevy-webgpu"))]
    {
        let stats = ve_bevy::latest();
        result["bevy"] = json!({
            "entities": stats.entities,
            "triangles": stats.triangles,
            "set_entities": stats.sets_entities,
            "frame": stats.frame,
        });
    }
    result
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
    authored: Query<(Entity, &crate::projection::Authored)>,
    textures: Res<ve_render::MaterialTextures>,
) {
    let texture_report = textures.report();
    *frames += 1;
    let errors = ERRORS.with(|errors| errors.borrow().clone());
    let mut value = json!({
        "frames": *frames,
        "authored": crate::projection::selections(&authored),
        "authored_count":authored.iter().len(),
        "authored_sample_limit":128,
        "errors": errors,
        "perf": perf.report(),
        "scene": scene_stats.as_deref().map(|s| json!({
            "instances": s.instances,
            "mesh_assets": s.mesh_assets,
            "triangles": s.triangles,
        })),
        "render": render_counts(),
        // The baked maps: counts and GPU bytes at 1024, 512 and 256 texels, the share of the budget, and what ASTC 6x6 would take.
        "textures": {
            "maps": texture_report.maps,
            "bytes": texture_report.bytes,
            "total_bytes": texture_report.total_bytes,
            "budget_bytes": texture_report.budget_bytes,
            "used_fraction": texture_report.used_fraction,
            "astc_6x6_estimate_bytes": texture_report.astc_6x6_estimate_bytes,
        },
        "interact": interact::report(&interaction, &labelled),
        "viewer": viewer.report(),
        "quality": {
            "tier": quality.tier.name(),
            "msaa": quality.msaa_samples,
            "shadow_resolution": quality.shadow_resolution,
            "max_point_lights": quality.max_point_lights,
            "resolution_scale": quality.resolution_scale,
            "specular_aa": quality.specular_aa_enabled,
            "ssao": quality.ssao_enabled,
            "bloom": quality.bloom_enabled,
            "auto_exposure": quality.auto_exposure_enabled,
            "smaa": quality.smaa_enabled,
            "effects": ve_render::Effects::NAMES.iter().map(|name| ((*name).to_owned(), quality.effects.get(name).unwrap_or(false))).collect::<std::collections::BTreeMap<_, _>>(),
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
