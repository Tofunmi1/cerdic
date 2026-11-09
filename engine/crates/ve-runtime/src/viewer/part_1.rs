use std::cell::Cell;
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
use ve_interact::CameraView;
use ve_render::{LensMode, LensWarp, StereoViews};
use ve_xr_math::Pose;
use ve_xr_web::{SessionState, XrCamera, session_state};

use crate::mock_hmd::{MockHead, stereo_views};
use crate::viewer_nav;

/// Radians of turn per pixel of mouse movement.
const SENSITIVITY: f32 = 0.0022;

/// Largest step the movement integrates in one frame, in seconds (a stalled tab must not fling the
/// viewer across the room).
const MAX_STEP: f32 = 0.1;

/// Speed multiplier while sprinting.
const SPRINT: f32 = 3.0;

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
    /// The lens the page asked for; it only applies where it makes sense (see `wanted_lens`).
    lens: LensMode,
    /// A pose to jump to on the next frame: position, yaw and pitch (see `set_look_at`).
    pose: Option<(Vec3, f32, f32)>,
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
            lens: LensMode::Off,
            pose: None,
        }
    }
}

thread_local! {
    static INPUT: RefCell<Input> = RefCell::new(Input::default());
    static CAMERA: Cell<Option<CameraView>> = const { Cell::new(None) };
}

/// The vertical field of view the viewer renders with, in degrees (the page's, or the default of 60).
pub(crate) fn fov_degrees() -> f32 {
    with_input(|input| if input.fov_deg > 0.0 { input.fov_deg } else { 60.0 })
}

/// A desktop viewer camera ray for a pointer at normalized device coordinates.
pub(crate) fn pointer_ray(x: f32, y: f32, aspect: f32) -> Option<ve_interact::Ray> {
    CAMERA
        .with(Cell::get)?
        .ray(bevy_math::Vec2::new(x, y), aspect)
}

/// Screen position and camera depth for a world-space handle point.
pub(crate) fn project(point: Vec3, aspect: f32) -> Option<Vec3> {
    CAMERA.with(Cell::get)?.project(point, aspect)
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

/// Chooses the lens by name: "off", "fisheye" (the whole frame, wants a wide field of view) or
/// "headset" (barrel distortion for the split-screen view). Returns false for any other name.
pub(crate) fn set_lens(name: &str) -> bool {
    let lens = match name {
        "off" => LensMode::Off,
        "fisheye" => LensMode::Fisheye,
        "headset" => LensMode::Headset,
        _ => return false,
    };
    with_input(|input| input.lens = lens);
    true
}

/// The lens that applies: the page's choice where it fits the view, otherwise none. A headset lens
/// needs two eyes to bend and a fisheye needs one wide view, and neither belongs in a real session,
/// where the headset's own optics do the work.
fn wanted_lens(input: &Input, split: bool, presenting: bool) -> LensMode {
    if !input.enabled || presenting {
        return LensMode::Off;
    }
    match (input.lens, split) {
        (LensMode::Headset, true) | (LensMode::Fisheye, false) => input.lens,
        _ => LensMode::Off,
    }
}

/// Jumps the viewer to `position` looking at `target` on the next frame (the fixed review views of
/// `docs/modeling/fixtures.md`). Needs the viewer to be on; non-finite values or a zero-length view
/// are ignored. Returns whether the pose was accepted.
pub(crate) fn set_look_at(position: Vec3, target: Vec3) -> bool {
    let direction = target - position;
    if !(position.is_finite() && target.is_finite()) || direction.length() < 1.0e-6 {
        return false;
    }
    let yaw = (-direction.x).atan2(-direction.z);
    let pitch = (direction.y / direction.length()).asin();
    with_input(|input| input.pose = Some((position, yaw, pitch)));
    true
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
    /// Physics walks the viewer (play mode with a walker): keys no longer fly it, they steer the walker.
    walking: bool,
    /// A game camera owns the view this frame: `Some(true)` when the player's look steers it (first and third person), `Some(false)` when it
    /// holds a fixed angle (so the mouse, wheel and orbit drags are not applied and nothing twitches).
    driven: Option<bool>,
    /// The yaw and pitch a game camera is drawn with this frame (set with the eye); the player's own look stays in `yaw` and `pitch`.
    view: Option<(f32, f32)>,
    /// What the player has done to a game camera: a zoom multiplier (1 is none) and how far the view is turned and tilted, in radians.
    user_zoom: f32,
    user_yaw: f32,
    user_pitch: f32,
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
            walking: false,
            driven: None,
            view: None,
            user_zoom: 1.0,
            user_yaw: 0.0,
            user_pitch: 0.0,
        }
    }
}

impl ViewerRig {
    /// Current eye position in the viewer rig's world space.
    pub(crate) fn position(&self) -> Vec3 {
        self.position
    }

    /// Where the player is looking: yaw and pitch in radians (the viewer faces -z at yaw 0).
    pub(crate) fn look(&self) -> (f32, f32) {
        (self.yaw, self.pitch)
    }

    /// Puts the view where a game camera says. The player's look (`yaw`, `pitch`) is NOT overwritten: a blend between cameras passes through
    /// angles the player never chose, and feeding those back as the look made a third-person camera stick at its pitch limit after a blend.
    /// `orbit` says whether the player's look steers this camera; a fixed-angle camera ignores the mouse and wheel.
    pub(crate) fn set_view(&mut self, eye: Vec3, yaw: f32, pitch: f32, orbit: bool) {
        self.position = eye;
        self.view = Some((yaw, pitch));
        self.driven = Some(orbit);
    }

    /// Gives the view back to the free viewer (no game camera, or its target is gone).
    pub(crate) fn release(&mut self) {
        self.driven = None;
        self.view = None;
        (self.user_zoom, self.user_yaw, self.user_pitch) = (1.0, 0.0, 0.0);
    }

    /// Points the player's look somewhere (the viewer faces -z at yaw 0; pitch is up positive).
    pub(crate) fn set_look(&mut self, yaw: f32, pitch: f32) {
        (self.yaw, self.pitch) = (yaw, pitch);
    }

    /// The player's adjustments of the game camera: zoom multiplier, turn and tilt in radians.
    pub(crate) fn adjustments(&self) -> (f32, f32, f32) {
        (self.user_zoom, self.user_yaw, self.user_pitch)
    }

    /// Takes the wheel and drag the page queued and applies them to a game camera. The wheel zooms (a notch is about a tenth). A drag turns
    /// the look of a camera the player steers (first and third person) and the view of a fixed-angle one.
    pub(crate) fn adjust_game_camera(&mut self, zoom: f32, orbit: Vec2) {
        self.user_zoom = (self.user_zoom * (zoom * 0.001).exp()).clamp(0.2, 5.0);
        if self.driven == Some(true) {
            self.yaw -= orbit.x * SENSITIVITY * 2.0;
            self.pitch = (self.pitch - orbit.y * SENSITIVITY * 2.0).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        } else {
            self.user_yaw -= orbit.x * 0.006;
            self.user_pitch = (self.user_pitch - orbit.y * 0.006).clamp(-1.0, 1.0);
        }
    }

    /// The view a game camera set this frame, as an eye and a rotation, or `None` when the viewer is free.
    pub(crate) fn driven_pose(&self) -> Option<(Vec3, Quat)> {
        self.driven.map(|_| (self.position, self.rotation()))
    }
}

/// What a walking viewer is asked to do this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WalkInput {
    /// Where the eye is now.
    pub(crate) eye: Vec3,
    /// World-space direction the viewer wants to walk, length 0 to 1 (forward, back, left and right from the keys).
    pub(crate) direction: Vec3,
    /// Whether a jump is asked for (the up key).
    pub(crate) jump: bool,
    /// Whether the run key is down.
    pub(crate) sprint: bool,
}

impl ViewerRig {
    /// Lets physics walk the viewer (true) or gives the keys back to the flying camera (false).
    pub(crate) fn set_walking(&mut self, on: bool) {
        self.walking = on;
    }

    /// The keys' wishes for a walking viewer; `None` while the desktop viewer is off or a headset is presenting.
    pub(crate) fn walk_input(&self) -> Option<WalkInput> {
        let input = with_input(|input| *input);
        if !input.enabled || session_state() == SessionState::Presenting {
            return None;
        }
        let turn = Quat::from_rotation_y(self.yaw);
        let direction =
            (turn * Vec3::new(input.motion.x, 0.0, -input.motion.z)).clamp_length_max(1.0);
        Some(WalkInput {
            eye: self.position,
            direction,
            jump: input.motion.y > 0.5,
            sprint: input.sprint,
        })
    }

    /// Puts the eye where the walker's body says it is.
    pub(crate) fn place_eye(&mut self, eye: Vec3) {
        self.position = eye;
    }
}

impl ViewerRig {
    /// The rotation the camera is drawn with: the game camera's view while one has it (which may be mid-blend), otherwise the player's look.
    pub(crate) fn rotation(&self) -> Quat {
        let (yaw, pitch) = self.view.unwrap_or((self.yaw, self.pitch));
        Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
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
            "lens": format!("{:?}", with_input(|input| input.lens)).to_lowercase(),
        })
    }

    /// The ray the crosshair points along while the pointer is held, as origin and direction.
    pub(crate) fn crosshair(&self) -> Option<(Vec3, Vec3)> {
        self.locked
            .then(|| (self.position, self.rotation() * Vec3::NEG_Z))
    }
}

/// Latest published desktop camera pose, when the free-look viewer is active.
pub(crate) fn pose() -> Option<(Vec3, Quat)> {
    CAMERA
        .with(Cell::get)
        .map(|camera| (camera.position, camera.rotation))
}
