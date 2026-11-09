//! Character controller declarations (`controller KIND key=value ...`): how the entities of a kind move from input axes.
use super::GameDef;
use std::{collections::BTreeMap, fmt::Write as _};

/// How a kind of entity is moved by the player's input: real dynamics, gravity and jumping.
#[derive(Clone, Debug, PartialEq)]
pub struct ControllerDef {
    /// The kind of entity this moves.
    pub kind: String,
    /// `walk` (3D ground), `platformer` (confined to a plane) or `top_down` (no gravity).
    pub mode: String,
    /// Fastest horizontal speed, in metres per second.
    pub speed: f64,
    /// How fast the horizontal speed rises toward the input, in metres per second squared.
    pub accel: f64,
    /// How fast it falls toward rest when the input stops, in metres per second squared.
    pub decel: f64,
    /// Share of `accel` and `decel` that works in the air, 0 to 1.
    pub air: f64,
    /// Upward speed of a jump in metres per second (0 turns jumping off).
    pub jump: f64,
    /// Downward acceleration in metres per second squared.
    pub gravity: f64,
    /// Seconds after leaving a ledge during which a jump still works.
    pub coyote: f64,
    /// Seconds a jump pressed too early is remembered, to fire on landing.
    pub buffer: f64,
    /// Jumps allowed in the air after leaving the ground.
    pub extra_jumps: u32,
    /// Share of the upward speed kept when the jump is released early, 0 to 1.
    pub cut: f64,
    /// Turn rate toward the direction of motion, in degrees per second (0 faces the motion at once).
    pub turn: f64,
    /// Highest step it walks up onto without jumping, in metres.
    pub step: f64,
    /// Whether the movement axes are relative to the camera (`camera`) or to the world (`world`).
    pub relative: String,
    /// A horizontal axis (`x` or `z`) it may not move along, or empty.
    pub lock: String,
    /// Below this height (feet, metres) the hero is returned to its last safe ground and a `fell` event is raised; `None` lets it fall forever.
    pub fall_limit: Option<f64>,
    /// Name of the movement input (its axes are `NAME.x` and `NAME.z`).
    pub movement: String,
    /// Name of the jump input.
    pub jump_input: String,
}

const MODES: [&str; 3] = ["walk", "platformer", "top_down"];

fn number(fields: &BTreeMap<String, String>, key: &str, default: f64, zero: bool) -> Result<f64, String> {
    let value = fields.get(key).map_or(Ok(default), |text| text.parse::<f64>()).map_err(|_| format!("{key} must be a finite number"))?;
    if !value.is_finite() || value < 0.0 || (!zero && value == 0.0) {
        return Err(format!("{key} must be finite and {}", if zero { "nonnegative" } else { "positive" }));
    }
    Ok(value)
}

fn word(fields: &BTreeMap<String, String>, key: &str, default: &str, allowed: &[&str]) -> Result<String, String> {
    let value = fields.get(key).map_or(default, String::as_str);
    if allowed.contains(&value) {
        Ok(value.to_owned())
    } else {
        Err(format!("{key} must be one of {}", allowed.join(", ")))
    }
}

pub(super) fn parse_controller(def: &mut GameDef, kind: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let kind = kind.ok_or("controller needs a kind")?;
    let Some(declared) = def.kinds.iter().find(|entry| entry.name == kind) else {
        return Err(format!("unknown controller kind '{kind}'"));
    };
    if def.controllers.iter().any(|entry| entry.kind == kind) {
        return Err(format!("duplicate controller kind '{kind}'"));
    }
    let mode = word(fields, "mode", "walk", &MODES)?;
    let speed = number(fields, "speed", if declared.speed > 0.0 { declared.speed } else { 4.0 }, false)?;
    let accel = number(fields, "accel", speed * 10.0, false)?;
    let platformer = mode == "platformer";
    let top_down = mode == "top_down";
    let lock_default = if platformer { "z" } else { "" };
    let lock = fields.get("lock").map_or(lock_default, String::as_str);
    if !["", "x", "z"].contains(&lock) {
        return Err("lock must be x or z (the axis it may not move along)".into());
    }
    let extra_jumps = number(fields, "extra_jumps", 0.0, true)?;
    if extra_jumps.fract() != 0.0 || extra_jumps > 5.0 {
        return Err("extra_jumps must be a whole number from 0 to 5".into());
    }
    let cut = number(fields, "cut", 0.5, true)?;
    let air = number(fields, "air", 0.6, true)?;
    if cut > 1.0 || air > 1.0 {
        return Err(format!("{} must be between 0 and 1", if cut > 1.0 { "cut" } else { "air" }));
    }
    let controller = ControllerDef {
        kind: kind.to_owned(),
        mode,
        speed,
        accel,
        decel: number(fields, "decel", accel, false)?,
        air,
        jump: if top_down { 0.0 } else { number(fields, "jump", 0.0, true)? },
        gravity: if top_down { 0.0 } else { number(fields, "gravity", 20.0, true)? },
        coyote: number(fields, "coyote", 0.1, true)?,
        buffer: number(fields, "buffer", 0.1, true)?,
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "checked to be a whole number from 0 to 5")]
        extra_jumps: extra_jumps as u32,
        cut,
        turn: number(fields, "turn", 0.0, true)?,
        step: number(fields, "step", 0.3, true)?,
        relative: word(fields, "relative", "world", &["world", "camera"])?,
        lock: lock.to_owned(),
        fall_limit: match fields.get("fall_limit") {
            Some(text) => Some(text.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or("fall_limit must be a finite number (a height in metres)")?),
            None => None,
        },
        movement: fields.get("move").cloned().unwrap_or_else(|| "move".into()),
        jump_input: fields.get("jump_input").cloned().unwrap_or_else(|| "jump".into()),
    };
    def.controllers.push(controller);
    Ok(())
}

pub(super) fn error_column(raw: &str, message: &str) -> Option<usize> {
    if !raw.trim_start().starts_with("controller ") {
        return None;
    }
    raw.split_whitespace().find_map(|token| {
        let key = token.split_once('=').map_or(token, |(key, _)| key);
        (message.contains(key) && key != "controller").then(|| raw.find(token).map(|index| raw[..index].chars().count() + 1)).flatten()
    })
}

impl ControllerDef {
    pub(crate) fn to_text(&self) -> String {
        let mut line = format!(
            "controller {} mode={} speed={} accel={} decel={} air={} jump={} gravity={} coyote={} buffer={} extra_jumps={} cut={} turn={} step={} relative={}",
            self.kind, self.mode, self.speed, self.accel, self.decel, self.air, self.jump, self.gravity, self.coyote, self.buffer, self.extra_jumps, self.cut, self.turn, self.step, self.relative
        );
        if !self.lock.is_empty() {
            let _ = write!(line, " lock={}", self.lock);
        }
        if let Some(limit) = self.fall_limit {
            let _ = write!(line, " fall_limit={limit}");
        }
        if self.movement != "move" {
            let _ = write!(line, " move={}", self.movement);
        }
        if self.jump_input != "jump" {
            let _ = write!(line, " jump_input={}", self.jump_input);
        }
        line
    }
}
