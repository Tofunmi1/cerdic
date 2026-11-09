//! Game cameras: how the view follows a target (`camera NAME mode=... ` statements) and the pure rig that turns a target position and a look
//! direction into an eye, a yaw, a pitch and a field of view.
//!
//! Yaw is a compass angle (the camera looks along `(sin yaw, cos yaw)` on the ground, so 0 looks along +z) and pitch is up-positive.
//! Modes: `first` (eye at the target, the player's look), `third` (orbit on a spring arm that shortens against walls, with a shoulder
//! offset), `top_down`, `side` and `iso` (a fixed angle with smoothing, a dead zone and bounds), `fixed` (stands at a place and watches).
//! Switching cameras blends between the two views.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::GameDef;
use crate::parse::helpers::validate_selector;

const MODES: [&str; 6] = ["first", "third", "top_down", "side", "iso", "fixed"];
/// The spring arm stops this far in front of a wall.
const ARM_MARGIN: f64 = 0.2;
/// The spring arm never gets shorter than this.
const ARM_MINIMUM: f64 = 0.3;
/// How far the player may zoom in and out, as shares of the camera's own distance.
const ZOOM_IN: f64 = 0.3;
const ZOOM_OUT: f64 = 3.0;
/// The highest an isometric view may be tilted: ten degrees below the horizon, in radians.
const MOST_LEVEL: f64 = -0.174_532_925_199_432_96;

/// How one camera follows its target.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraDef {
    /// The camera's name, used to switch to it.
    pub name: String,
    /// `first`, `third`, `top_down`, `side`, `iso` or `fixed`.
    pub mode: String,
    /// Selector of the entity it follows, such as `tag:hero`.
    pub target: String,
    /// Arm length (third), height above the target (top down) or distance back from the target's plane (side, iso), in metres.
    pub distance: f64,
    /// Height of the pivot above the target's feet, in metres (the eye height in first person).
    pub height: f64,
    /// Sideways offset of a third-person camera to the right, in metres.
    pub shoulder: f64,
    /// Fixed pitch in degrees for the fixed-angle modes (negative looks down).
    pub pitch: f64,
    /// Fixed yaw in degrees for the fixed-angle modes.
    pub yaw: f64,
    /// Field of view in degrees.
    pub fov: f64,
    /// Seconds in which the view closes most of a gap to the target (0 follows rigidly).
    pub smoothing: f64,
    /// Whether a third-person arm shortens against walls.
    pub collide: bool,
    /// Half-size of the area around the view in which the target moves without moving the view, in metres.
    pub deadzone: f64,
    /// Where a `fixed` camera stands.
    pub at: Option<[f64; 3]>,
    /// Limits of the view's pivot as `[x_min, x_max, z_min, z_max]`.
    pub bounds: Option<[f64; 4]>,
    /// Lowest third-person pitch in degrees.
    pub min_pitch: f64,
    /// Highest third-person pitch in degrees.
    pub max_pitch: f64,
}

/// What the rig is told each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraInput {
    /// Feet of the followed entity.
    pub target: [f64; 3],
    /// Yaw the followed entity faces, in radians.
    pub heading: f64,
    /// Yaw the player is looking along, in radians (mouse or stick).
    pub orbit_yaw: f64,
    /// Pitch the player is looking at, in radians (up positive).
    pub orbit_pitch: f64,
    /// Seconds since the last pose.
    pub dt: f64,
    /// The player's zoom as a multiplier of the camera's distance (1 leaves it; limited to a third and three times).
    pub zoom: f64,
    /// How far the player has turned a top-down or isometric view about the vertical, in radians (other modes ignore it).
    pub yaw_offset: f64,
    /// How far the player has tilted an isometric view, in radians (up positive; the view never rises above ten degrees below the horizon).
    pub pitch_offset: f64,
}

/// Where the view is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    /// The eye.
    pub eye: [f64; 3],
    /// Compass yaw in radians.
    pub yaw: f64,
    /// Pitch in radians, up positive.
    pub pitch: f64,
    /// Vertical field of view in degrees.
    pub fov: f64,
}

/// The cameras of a game, which one is active and the blend in progress.
#[derive(Clone, Debug)]
pub struct CameraRig {
    cameras: Vec<CameraDef>,
    states: Vec<State>,
    active: usize,
    blend: Option<Blend>,
}

#[derive(Clone, Debug, Default)]
struct State {
    /// The smoothed place the view is anchored to.
    anchor: Option<[f64; 3]>,
}

#[derive(Clone, Copy, Debug)]
struct Blend {
    from: usize,
    seconds: f64,
    elapsed: f64,
}

type Collide<'a> = &'a dyn Fn([f64; 3], [f64; 3]) -> Option<f64>;

fn forward(yaw: f64, pitch: f64) -> [f64; 3] {
    [pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()]
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

impl CameraDef {
    fn fixed_angle(&self) -> bool {
        matches!(self.mode.as_str(), "top_down" | "side" | "iso")
    }

    /// The anchor the view follows this step: the target's pivot, smoothed, outside the dead zone, inside the bounds.
    fn anchor(&self, state: &mut State, input: &CameraInput) -> [f64; 3] {
        let mut want = [input.target[0], input.target[1] + self.height, input.target[2]];
        if let Some([x0, x1, z0, z1]) = self.bounds {
            want[0] = want[0].clamp(x0, x1);
            want[2] = want[2].clamp(z0, z1);
        }
        let Some(mut anchor) = state.anchor else {
            state.anchor = Some(want);
            return want;
        };
        if self.deadzone > 0.0 {
            // a side-scroller also lets the hero jump inside a vertical band without the picture bobbing with it
            let axes: &[usize] = if self.mode == "side" { &[0, 1, 2] } else { &[0, 2] };
            for &axis in axes {
                let gap = want[axis] - anchor[axis];
                if gap.abs() > self.deadzone {
                    want[axis] -= gap.signum() * self.deadzone;
                } else {
                    want[axis] = anchor[axis];
                }
            }
        }
        let follow = if self.smoothing > 0.0 { 1.0 - (-input.dt / self.smoothing).exp() } else { 1.0 };
        for axis in 0..3 {
            anchor[axis] = lerp(anchor[axis], want[axis], follow);
        }
        state.anchor = Some(anchor);
        anchor
    }

    fn pose(&self, state: &mut State, input: &CameraInput, collide: Collide) -> CameraPose {
        let anchor = self.anchor(state, input);
        match self.mode.as_str() {
            "first" => CameraPose { eye: anchor, yaw: input.orbit_yaw, pitch: input.orbit_pitch, fov: self.fov },
            "fixed" => {
                let eye = self.at.unwrap_or([0.0; 3]);
                let (dx, dy, dz) = (anchor[0] - eye[0], anchor[1] - eye[1], anchor[2] - eye[2]);
                CameraPose { eye, yaw: dx.atan2(dz), pitch: dy.atan2(dx.hypot(dz)), fov: self.fov }
            }
            _ => {
                let (yaw, pitch) = if self.fixed_angle() {
                    // the player may turn a top-down or isometric view and tilt an isometric one; a side-scroller keeps its plane
                    let turn = if matches!(self.mode.as_str(), "top_down" | "iso") { input.yaw_offset } else { 0.0 };
                    let tilt = if self.mode == "iso" { input.pitch_offset } else { 0.0 };
                    let pitch = self.pitch.to_radians() + tilt;
                    (self.yaw.to_radians() + turn, if self.mode == "iso" { pitch.min(MOST_LEVEL) } else { pitch })
                } else {
                    (input.orbit_yaw, input.orbit_pitch.clamp(self.min_pitch.to_radians(), self.max_pitch.to_radians()))
                };
                let look = forward(yaw, pitch);
                let right = [-yaw.cos(), 0.0, yaw.sin()];
                let mut arm = self.distance * input.zoom.clamp(ZOOM_IN, ZOOM_OUT);
                let shoulder = if self.fixed_angle() { 0.0 } else { self.shoulder };
                if self.collide && !self.fixed_angle() {
                    let wanted = [
                        anchor[0] - look[0] * arm + right[0] * shoulder,
                        anchor[1] - look[1] * arm,
                        anchor[2] - look[2] * arm + right[2] * shoulder,
                    ];
                    if let Some(hit) = collide(anchor, wanted) {
                        arm = (hit - ARM_MARGIN).clamp(ARM_MINIMUM, arm);
                    }
                }
                let eye = [
                    anchor[0] - look[0] * arm + right[0] * shoulder,
                    anchor[1] - look[1] * arm,
                    anchor[2] - look[2] * arm + right[2] * shoulder,
                ];
                CameraPose { eye, yaw, pitch, fov: self.fov }
            }
        }
    }
}

impl CameraRig {
    /// A rig over the declared cameras; the first one starts active.
    #[must_use]
    pub fn new(cameras: Vec<CameraDef>) -> Self {
        let states = vec![State::default(); cameras.len()];
        Self { cameras, states, active: 0, blend: None }
    }

    /// Whether the game declared any camera.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cameras.is_empty()
    }

    /// The name of the active camera (empty when there are none).
    #[must_use]
    pub fn active(&self) -> &str {
        self.cameras.get(self.active).map_or("", |camera| camera.name.as_str())
    }

    /// The active camera's definition.
    #[must_use]
    pub fn active_def(&self) -> Option<&CameraDef> {
        self.cameras.get(self.active)
    }

    /// Makes the named camera active, blending from the current view over `seconds` (0 cuts).
    ///
    /// # Errors
    /// The name is not one of the game's cameras.
    pub fn switch(&mut self, name: &str, seconds: f64) -> Result<(), String> {
        // `next` and `prev` cycle through the cameras in the order they were declared
        let count = self.cameras.len();
        let cycled = match name {
            "next" if count > 0 => Some((self.active + 1) % count),
            "prev" if count > 0 => Some((self.active + count - 1) % count),
            _ => None,
        };
        let Some(next) = cycled.or_else(|| self.cameras.iter().position(|camera| camera.name == name)) else {
            let names: Vec<&str> = self.cameras.iter().map(|camera| camera.name.as_str()).collect();
            return Err(format!("no camera named {name:?} (the cameras are {names:?})"));
        };
        if next != self.active {
            self.blend = (seconds > 0.0).then_some(Blend { from: self.active, seconds, elapsed: 0.0 });
            self.active = next;
        }
        Ok(())
    }

    /// The view for this step.
    pub fn pose(&mut self, input: &CameraInput, collide: Collide) -> CameraPose {
        let Some(camera) = self.cameras.get(self.active) else {
            return CameraPose { eye: [0.0; 3], yaw: 0.0, pitch: 0.0, fov: 60.0 };
        };
        let now = camera.pose(&mut self.states[self.active], input, collide);
        let Some(blend) = self.blend.as_mut() else {
            return now;
        };
        blend.elapsed += input.dt;
        let t = (blend.elapsed / blend.seconds).clamp(0.0, 1.0);
        let (from, done) = (blend.from, t >= 1.0);
        let before = self.cameras[from].pose(&mut self.states[from], input, collide);
        if done {
            self.blend = None;
            return now;
        }
        let eased = t * t * (3.0 - 2.0 * t);
        CameraPose {
            eye: [lerp(before.eye[0], now.eye[0], eased), lerp(before.eye[1], now.eye[1], eased), lerp(before.eye[2], now.eye[2], eased)],
            yaw: before.yaw + wrap(now.yaw - before.yaw) * eased,
            pitch: lerp(before.pitch, now.pitch, eased),
            fov: lerp(before.fov, now.fov, eased),
        }
    }
}

fn number(fields: &BTreeMap<String, String>, key: &str, default: f64, zero: bool) -> Result<f64, String> {
    let value = fields.get(key).map_or(Ok(default), |text| text.parse::<f64>()).map_err(|_| format!("{key} must be a finite number"))?;
    if !value.is_finite() || value < 0.0 || (!zero && value == 0.0) {
        return Err(format!("{key} must be finite and {}", if zero { "nonnegative" } else { "positive" }));
    }
    Ok(value)
}

fn signed(fields: &BTreeMap<String, String>, key: &str, default: f64) -> Result<f64, String> {
    let value = fields.get(key).map_or(Ok(default), |text| text.parse::<f64>()).map_err(|_| format!("{key} must be a finite number"))?;
    if value.is_finite() { Ok(value) } else { Err(format!("{key} must be a finite number")) }
}

fn numbers(text: &str, key: &str, count: usize) -> Result<Vec<f64>, String> {
    let values: Result<Vec<f64>, _> = text.split(',').map(str::parse::<f64>).collect();
    match values {
        Ok(values) if values.len() == count && values.iter().all(|v| v.is_finite()) => Ok(values),
        _ => Err(format!("{key} needs {count} finite numbers separated by commas")),
    }
}

pub(crate) fn parse_camera(def: &mut GameDef, name: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let name = name.ok_or("camera needs a name")?;
    if def.cameras.iter().any(|camera| camera.name == name) {
        return Err(format!("duplicate camera '{name}'"));
    }
    let mode = fields.get("mode").map_or("third", String::as_str);
    if !MODES.contains(&mode) {
        return Err(format!("mode must be one of {}", MODES.join(", ")));
    }
    let target = fields.get("target").cloned().unwrap_or_else(|| "tag:hero".into());
    validate_selector(&target).map_err(|message| format!("target: {message}"))?;
    let (distance, height, pitch, yaw, smoothing) = match mode {
        "first" => (0.0, 1.6, 0.0, 0.0, 0.0),
        "third" => (4.5, 1.5, 0.0, 0.0, 0.08),
        "top_down" => (14.0, 0.0, -89.0, 0.0, 0.1),
        "side" => (10.0, 1.0, 0.0, 0.0, 0.1),
        "iso" => (16.0, 0.0, -35.0, 45.0, 0.1),
        _ => (0.0, 1.0, 0.0, 0.0, 0.0),
    };
    let at = match fields.get("at") {
        Some(text) => {
            let v = numbers(text, "at", 3)?;
            Some([v[0], v[1], v[2]])
        }
        None if mode == "fixed" => return Err("a fixed camera needs at=x,y,z (where it stands)".into()),
        None => None,
    };
    let bounds = match fields.get("bounds") {
        Some(text) => {
            let v = numbers(text, "bounds", 4).map_err(|_| "bounds needs four numbers: x_min,x_max,z_min,z_max".to_owned())?;
            if v[0] > v[1] || v[2] > v[3] {
                return Err("bounds must run from the smaller to the larger value on each axis".into());
            }
            Some([v[0], v[1], v[2], v[3]])
        }
        None => None,
    };
    let (min_pitch, max_pitch) = (signed(fields, "min_pitch", -60.0)?, signed(fields, "max_pitch", 75.0)?);
    if min_pitch >= max_pitch {
        return Err("min_pitch must be below max_pitch".into());
    }
    let collide = match fields.get("collide").map(String::as_str) {
        None | Some("1" | "true") => true,
        Some("0" | "false") => false,
        Some(_) => return Err("collide must be 1 or 0".into()),
    };
    def.cameras.push(CameraDef {
        name: name.to_owned(),
        mode: mode.to_owned(),
        target,
        distance: number(fields, "distance", distance, mode == "first" || mode == "fixed")?,
        height: number(fields, "height", height, true)?,
        shoulder: signed(fields, "shoulder", 0.0)?,
        pitch: signed(fields, "pitch", pitch)?,
        yaw: signed(fields, "yaw", yaw)?,
        fov: number(fields, "fov", 60.0, false)?,
        smoothing: number(fields, "smoothing", smoothing, true)?,
        collide,
        deadzone: number(fields, "deadzone", 0.0, true)?,
        at,
        bounds,
        min_pitch,
        max_pitch,
    });
    Ok(())
}

pub(crate) fn error_column(raw: &str, message: &str) -> Option<usize> {
    if !raw.trim_start().starts_with("camera ") {
        return None;
    }
    raw.split_whitespace().find_map(|token| {
        let key = token.split_once('=').map_or(token, |(key, _)| key);
        (message.contains(key) && key != "camera").then(|| raw.find(token).map(|index| raw[..index].chars().count() + 1)).flatten()
    })
}

impl CameraDef {
    pub(crate) fn to_text(&self) -> String {
        let mut line = format!(
            "camera {} mode={} target={} distance={} height={} shoulder={} pitch={} yaw={} fov={} smoothing={} collide={} deadzone={} min_pitch={} max_pitch={}",
            self.name, self.mode, self.target, self.distance, self.height, self.shoulder, self.pitch, self.yaw, self.fov, self.smoothing, u8::from(self.collide), self.deadzone, self.min_pitch, self.max_pitch
        );
        if let Some([x, y, z]) = self.at {
            let _ = write!(line, " at={x},{y},{z}");
        }
        if let Some([a, b, c, d]) = self.bounds {
            let _ = write!(line, " bounds={a},{b},{c},{d}");
        }
        line
    }
}
