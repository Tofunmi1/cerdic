//! The desktop viewer: look around any scene with the mouse and keyboard, with an adjustable (wide)
//! field of view and an optional split-screen mock headset. It is for seeing the engine's work on a
//! laptop without a headset. The page owns the DOM side (pointer lock, keys) and feeds it in through
//! the functions below; nothing here runs unless the page enables it, so scenes stay exactly as
//! authored otherwise.

use std::cell::RefCell;
use std::f32::consts::FRAC_PI_2;

use bevy_camera::{Camera3d, PerspectiveProjection, Projection};
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};
use bevy_math::{EulerRot, Quat, Vec2, Vec3};
use bevy_time::{Real, Time};
use bevy_transform::components::Transform;
use bevy_window::{PrimaryWindow, Window};
use ve_render::StereoViews;
use ve_xr_math::{FovTangents, MockHmd, Pose};
use ve_xr_web::{SessionState, XrCamera, session_state};

use crate::mock_hmd::{MockHead, stereo_views};

/// Radians of turn per pixel of mouse movement.
const SENSITIVITY: f32 = 0.0022;

/// Largest step the movement integrates in one frame, in seconds (a stalled tab must not fling the
/// viewer across the room).
const MAX_STEP: f32 = 0.1;

/// Speed multiplier while sprinting.
const SPRINT: f32 = 3.0;

/// Interpupillary distance of the split-screen mock headset, in metres.
const IPD: f32 = 0.064;

/// Near plane of the mock headset's eyes, in metres.
const NEAR: f32 = 0.05;

/// What the page asked for.
#[derive(Clone, Copy, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent on/off switches the page sets separately"
)]
struct Input {
    enabled: bool,
    locked: bool,
    /// Vertical field of view in degrees; zero keeps the scene's own.
    fov_deg: f32,
    stereo: bool,
    speed: f32,
    /// Mouse movement since the last frame, in pixels.
    look: Vec2,
    /// Wanted motion: x right, y up, z forward, each in -1..=1.
    motion: Vec3,
    sprint: bool,
    reset: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            enabled: false,
            locked: false,
            fov_deg: 0.0,
            stereo: false,
            speed: 2.0,
            look: Vec2::ZERO,
            motion: Vec3::ZERO,
            sprint: false,
            reset: false,
        }
    }
}

thread_local! {
    static INPUT: RefCell<Input> = RefCell::new(Input::default());
}

fn with_input<T>(f: impl FnOnce(&mut Input) -> T) -> T {
    INPUT.with(|input| f(&mut input.borrow_mut()))
}

/// Turns the viewer on or off and sets its field of view (degrees, vertical; 0 keeps the scene's),
/// split-screen mock headset and speed (metres per second).
pub(crate) fn configure(enabled: bool, fov_deg: f32, stereo: bool, speed: f32) {
    with_input(|input| {
        input.enabled = enabled;
        input.fov_deg = if fov_deg.is_finite() {
            fov_deg.clamp(0.0, 170.0)
        } else {
            0.0
        };
        input.stereo = stereo;
        input.speed = if speed.is_finite() {
            speed.clamp(0.1, 50.0)
        } else {
            2.0
        };
    });
}

/// Adds mouse movement (pixels) to turn the view by on the next frame.
pub(crate) fn add_look(dx: f32, dy: f32) {
    if dx.is_finite() && dy.is_finite() {
        with_input(|input| input.look += Vec2::new(dx, dy));
    }
}

/// Sets the wanted motion: each axis in -1..=1, and whether to sprint.
pub(crate) fn set_motion(right: f32, up: f32, forward: f32, sprint: bool) {
    let clean = |v: f32| {
        if v.is_finite() {
            v.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    with_input(|input| {
        input.motion = Vec3::new(clean(right), clean(up), clean(forward));
        input.sprint = sprint;
    });
}

/// Records whether the page holds the pointer (mouse look): the crosshair then picks and grabs.
pub(crate) fn set_locked(locked: bool) {
    with_input(|input| input.locked = locked);
}

/// Puts the view back where the scene started.
pub(crate) fn request_reset() {
    with_input(|input| input.reset = true);
}

/// Where the viewer stands and looks.
#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct ViewerRig {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    home: Option<(Vec3, f32, f32)>,
    native_stereo: bool,
    locked: bool,
}

impl Default for ViewerRig {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            home: None,
            native_stereo: false,
            locked: false,
        }
    }
}

impl ViewerRig {
    fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    /// Where the viewer stands and looks, for the status report.
    pub(crate) fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "position": self.position.to_array(),
            "yaw": self.yaw,
            "pitch": self.pitch,
            "locked": self.locked,
            "started": self.home.is_some(),
            "split_screen": self.native_stereo || with_input(|input| input.stereo),
        })
    }

    /// The ray the crosshair points along while the pointer is held, as origin and direction.
    pub(crate) fn crosshair(&self) -> Option<(Vec3, Vec3)> {
        self.locked
            .then(|| (self.position, self.rotation() * Vec3::NEG_Z))
    }
}

/// The horizontal field of view (radians) of one eye that has the given vertical one and aspect.
fn horizontal_fov(vertical: f32, aspect: f32) -> f32 {
    2.0 * ((vertical * 0.5).tan() * aspect).atan()
}

/// Turns the viewer by the mouse movement and moves it by the wanted motion, for `dt` seconds.
fn step(rig: &mut ViewerRig, input: &Input, dt: f32) {
    rig.yaw -= input.look.x * SENSITIVITY;
    rig.pitch = (rig.pitch - input.look.y * SENSITIVITY).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    let wanted =
        rig.rotation() * Vec3::new(input.motion.x, 0.0, -input.motion.z) + Vec3::Y * input.motion.y;
    let boost = if input.sprint { SPRINT } else { 1.0 };
    rig.position += wanted.clamp_length_max(1.0) * input.speed * boost * dt.min(MAX_STEP);
}

/// The vertical field of view to use, in radians: the page's, or a comfortable default.
fn vertical_fov(input: &Input) -> f32 {
    if input.fov_deg > 0.0 {
        input.fov_deg.to_radians()
    } else {
        std::f32::consts::FRAC_PI_3
    }
}

/// The mock headset for the split-screen view of a target `size` pixels across. A scene's own
/// headset keeps its field of view unless the page asked for one.
fn mock_head(pose: Pose, existing: Option<&MockHead>, input: &Input, size: (u32, u32)) -> MockHead {
    if let Some(head) = existing
        && input.fov_deg <= 0.0
    {
        return MockHead {
            pose,
            hmd: head.hmd,
        };
    }
    let vertical = vertical_fov(input);
    #[expect(clippy::cast_precision_loss, reason = "window sizes are small")]
    let aspect = (size.0 as f32 * 0.5) / (size.1.max(1) as f32);
    let fov =
        FovTangents::symmetric(horizontal_fov(vertical, aspect), vertical).unwrap_or(FovTangents {
            left: -1.0,
            right: 1.0,
            down: -1.0,
            up: 1.0,
        });
    MockHead {
        pose,
        hmd: MockHmd::new(IPD, fov),
    }
}

/// Moves and turns the viewer, and applies it to the scene's camera.
pub(crate) fn update_viewer(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut rig: ResMut<ViewerRig>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut MockHead>,
            Option<&mut Projection>,
        ),
        With<Camera3d>,
    >,
) {
    let input = with_input(|input| {
        let snapshot = *input;
        input.look = Vec2::ZERO;
        input.reset = false;
        snapshot
    });
    rig.locked = input.enabled && input.locked;
    if !input.enabled || session_state() == SessionState::Presenting {
        return;
    }
    let Some((entity, mut transform, head, projection)) = cameras.iter_mut().next() else {
        return;
    };
    if rig.home.is_none() {
        // Start from wherever the scene put its camera.
        let (position, rotation) = head.as_ref().map_or_else(
            || (transform.translation, transform.rotation),
            |h| (h.pose.position, h.pose.orientation),
        );
        let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
        rig.home = Some((position, yaw, pitch));
        rig.native_stereo = head.is_some();
        (rig.position, rig.yaw, rig.pitch) = (position, yaw, pitch);
    }
    if input.reset
        && let Some((position, yaw, pitch)) = rig.home
    {
        (rig.position, rig.yaw, rig.pitch) = (position, yaw, pitch);
    }

    step(&mut rig, &input, time.delta_secs());
    let rotation = rig.rotation();
    let vertical = vertical_fov(&input);
    if input.stereo || rig.native_stereo {
        let size = windows.single().map_or((2, 1), |w| {
            (
                w.resolution.physical_width(),
                w.resolution.physical_height(),
            )
        });
        let pose = Pose {
            position: rig.position,
            orientation: rotation,
        };
        let new_head = mock_head(pose, head.as_deref(), &input, size);
        if let Some(mut existing) = head {
            *existing = new_head;
        } else {
            // A scene without a mock headset: split the view, and leave the real headset's camera
            // marker off (it would strip the eyes every frame).
            commands
                .entity(entity)
                .insert((new_head, stereo_views(&new_head, size)))
                .remove::<XrCamera>();
        }
    } else {
        if head.is_some() {
            commands.entity(entity).remove::<(MockHead, StereoViews)>();
        }
        transform.translation = rig.position;
        transform.rotation = rotation;
        if input.fov_deg > 0.0
            && let Some(mut projection) = projection
        {
            *projection = Projection::Perspective(PerspectiveProjection {
                fov: vertical,
                near: NEAR,
                ..PerspectiveProjection::default()
            });
        }
    }
}
