//! The Bevy side: applies each XR frame to the headset camera and exposes controller input.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};
use bevy_math::Mat4;
use ve_render::StereoViews;

use crate::frame::{XrController, stereo_from_frame};
use crate::state::{SessionState, session_state, take_latest};

/// Marks the camera that the headset drives. While a session is presenting it gets a
/// [`StereoViews`] component each frame; when the session ends the component is removed and the
/// camera renders normally again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct XrCamera;

/// Where the tracking space's origin sits in the world. The user's floor-level origin is placed
/// here, so moving or turning the rig teleports or turns the player.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct XrRig(pub Mat4);

impl Default for XrRig {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

/// The latest controller state, in tracking space (apply [`XrRig`] to place it in the world).
#[derive(Resource, Clone, Debug, Default)]
pub struct XrInput {
    /// Tracked controllers this frame; empty when not presenting.
    pub controllers: Vec<XrController>,
    /// Timestamp of the frame these were read in, in milliseconds.
    pub frame_time_ms: f64,
}

/// Applies XR frames to the app.
#[derive(Debug, Default, Clone, Copy)]
pub struct XrPlugin;

impl Plugin for XrPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<XrRig>()
            .init_resource::<XrInput>()
            .add_systems(Update, apply_xr_frame);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(
            Update,
            bevy_ecs::schedule::IntoScheduleConfigs::after(
                crate::layer::bind_layer_framebuffer,
                apply_xr_frame,
            ),
        );
    }
}

/// Moves the newest XR frame into the world: eyes onto the headset camera, controllers into
/// [`XrInput`]. When no session is presenting, the headset camera goes back to normal rendering.
pub fn apply_xr_frame(
    mut commands: Commands,
    rig: Res<XrRig>,
    mut input: ResMut<XrInput>,
    cameras: Query<(Entity, Option<&StereoViews>), With<XrCamera>>,
) {
    if session_state() != SessionState::Presenting {
        input.controllers.clear();
        for (entity, stereo) in &cameras {
            if stereo.is_some() {
                commands.entity(entity).remove::<StereoViews>();
            }
        }
        // Frames from a finished session must not leak into the next one.
        let _stale = take_latest();
        return;
    }
    let Some(frame) = take_latest() else {
        return;
    };
    input.controllers.clone_from(&frame.controllers);
    input.frame_time_ms = frame.time_ms;
    if let Some(stereo) = stereo_from_frame(&frame, rig.0) {
        for (entity, _) in &cameras {
            commands.entity(entity).insert(stereo);
        }
    }
}
