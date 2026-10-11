//! The forward schedule and the camera driver that runs it once per camera.
//!
//! Rendering is camera driven: every camera names a schedule (`CameraRenderGraph`) and the driver,
//! which lives in the `RenderGraph` schedule, runs it with `CurrentView` set to that camera. Our
//! cameras use [`Forward3d`]. The shape follows Bevy's own driver so a stereo driver can later run
//! the same schedule once per eye.

use bevy_camera::NormalizedRenderTarget;
use bevy_ecs::entity::EntityHashSet;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ScheduleBuildSettings, ScheduleLabel, SystemSet};
use bevy_ecs::system::SystemState;
use bevy_render::camera::{ExtractedCamera, SortedCameras};
use bevy_render::renderer::CurrentView;
use bevy_render::sync_world::MainEntity;
use bevy_render::view::ExtractedWindow;

/// Schedule label for the forward 3D pipeline.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Forward3d;

/// System sets of the [`Forward3d`] schedule, in execution order.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Forward3dSystems {
    /// Opaque geometry.
    MainPass,
    /// Copies the offscreen result to outputs that cannot be rendered to directly.
    Present,
}

impl Forward3d {
    /// The empty schedule with its sets configured.
    #[must_use]
    pub fn base_schedule() -> Schedule {
        let mut schedule = Schedule::new(Self);
        schedule.set_build_settings(ScheduleBuildSettings {
            auto_insert_apply_deferred: false,
            ..Default::default()
        });
        schedule.configure_sets(Forward3dSystems::Present.after(Forward3dSystems::MainPass));
        schedule
    }
}

/// Windows that at least one camera rendered to this frame.
#[derive(Resource, Default)]
pub(crate) struct CameraWindows(pub(crate) EntityHashSet);

/// Runs each camera's schedule, skipping cameras whose window has no pixels.
pub(crate) fn camera_driver(
    world: &mut World,
    windows: &mut SystemState<Query<(MainEntity, &ExtractedWindow)>>,
) {
    let cameras: Vec<Entity> = world
        .resource::<SortedCameras>()
        .0
        .iter()
        .map(|camera| camera.entity)
        .collect();
    let mut covered = EntityHashSet::default();

    for entity in cameras {
        let Some(camera) = world.get::<ExtractedCamera>(entity) else {
            continue;
        };
        let schedule = camera.schedule;
        let target = camera.target.clone();

        if let Some(NormalizedRenderTarget::Window(window_ref)) = &target {
            let window_entity = window_ref.entity();
            let has_pixels = windows.get(world).is_ok_and(|query| {
                query.iter().any(|(main, window)| {
                    main == window_entity && window.physical_width > 0 && window.physical_height > 0
                })
            });
            if !has_pixels {
                continue;
            }
            covered.insert(window_entity);
        }

        world.insert_resource(CurrentView(entity));
        world.run_schedule(schedule);
    }

    world.remove_resource::<CurrentView>();
    world.insert_resource(CameraWindows(covered));
}
