use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::{
    Fail,
    cache::Value,
    fail,
    flags::{self, FlagSet, Need},
};

/// What starts a behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// The part is clicked in play mode.
    Select,
    /// The viewer camera comes within `radius` metres of the part's centre.
    Near,
    /// Nothing in the world starts it; only another part's behaviour that names it as `target` does.
    Signal,
    /// The flag named in `watch` changes.
    Flag,
    /// A hand (or the mouse) picks the part up in play mode. The part becomes grabbable while playing.
    Grab,
    /// The last hand lets the part go in play mode.
    Release,
    /// A socket: the part named in `accepts` is let go within `radius` of this part and snaps onto it (this fires with
    /// `sets` as written and its clip forward); picking it up again fires the reverse (like `hold`).
    Socket,
    /// A timer: fires `after` seconds into play mode, then every `every` seconds if that is set.
    Timer,
    /// The part (which needs phys.body) touches another body: any body, or only the one named in `accepts`. Fires
    /// when the touching starts, one step after the physics step that made contact.
    Collide,
    /// Both: acts when picked up (`sets` as written) and again when let go (`sets` reversed: on becomes off, off
    /// becomes on, toggle stays toggle), and its clip runs forward while held and backward when released.
    Hold,
}

/// What a trigger does to the part's clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Run the tracks once from the start and hold the end.
    Play,
    /// Run forward; the next trigger runs backward, and so on.
    Toggle,
    /// With `trigger="flag"`: run forward while the watched flag is on and backward while it is off.
    Follow,
    /// Makes the character whose `phys.ragdoll` is named by `target` go limp. Plays no clip.
    Ragdoll,
}

/// A response attached to a part: when `trigger` happens, `action` runs the part's own tracks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Behavior {
    /// What starts it.
    pub trigger: Trigger,
    /// The output name of the part whose clip the action runs; `None` runs the part's own clip.
    pub target: Option<String>,
    /// Metres for a `near` trigger.
    pub radius: f32,
    /// What it does.
    pub action: Action,
    /// Flag changes made when it fires.
    #[serde(default)]
    pub sets: Vec<FlagSet>,
    /// Flag states it needs before it will fire.
    #[serde(default)]
    pub needs: Vec<Need>,
    /// The flag a `flag` trigger watches.
    #[serde(default)]
    pub watch: Option<String>,
    /// Seconds into play mode a `timer` trigger first fires.
    #[serde(default)]
    pub after: f32,
    /// Seconds between later firings of a `timer` trigger; 0 fires once.
    #[serde(default)]
    pub every: f32,
    /// The output name of the grabbable part a `socket` accepts.
    #[serde(default)]
    pub accepts: Option<String>,
    /// The part's own clip this behaviour runs ("" is the main clip).
    #[serde(default)]
    pub clip: String,
    /// The clip on the `target` part that it runs ("" is the main clip).
    #[serde(default)]
    pub target_clip: String,
    /// Clips of this part to run one after another, each starting when the one before reaches its end (a sequence
    /// that begins with `clip`).
    #[serde(default)]
    pub then: Vec<String>,
}

/// Longest a timer may wait or repeat, in seconds.
pub(crate) const MAX_TIMER: f32 = 3600.0;
/// Shortest repeat interval, in seconds (a few steps, so a timer cannot fire every frame).
pub(crate) const MIN_EVERY: f32 = 0.1;

/// Whether a timer fires on `play_step` (the number of fixed steps since play mode began, starting at 0): once at
/// `after` seconds, then every `every` seconds when that is above zero. Pure, so every machine agrees.
#[must_use]
pub fn timer_due(after: f32, every: f32, play_step: u64) -> bool {
    let to_steps = |seconds: f32| -> u64 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "bounded seconds, rounded to whole steps"
        )]
        let steps = (seconds / STEP).round() as u64;
        steps
    };
    let first = to_steps(after);
    if play_step < first {
        return false;
    }
    let repeat = if every > 0.0 {
        to_steps(every).max(1)
    } else {
        0
    };
    play_step == first || (repeat > 0 && (play_step - first).is_multiple_of(repeat))
}

/// Seconds per behaviour step (60 steps a second). Clip times only ever move by whole steps.
pub const STEP: f32 = 1.0 / 60.0;

/// The running state of one behaviour: where its clip is, and which way it is going. Pure and deterministic:
/// the same triggers at the same step numbers give the same times everywhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Reaction {
    /// What the part reacts to and how.
    pub behavior: Behavior,
    /// Length of the part's clip in seconds.
    pub duration: f32,
    /// Seconds into the clip (0 is the rest pose).
    pub time: f32,
    /// Whether the clip runs forward (or backward).
    pub forward: bool,
    /// Whether the clip is moving.
    pub running: bool,
}

impl Reaction {
    /// A part at rest.
    #[must_use]
    pub fn new(behavior: Behavior, duration: f32) -> Self {
        Self {
            behavior,
            duration,
            time: 0.0,
            forward: false,
            running: false,
        }
    }

    /// The trigger happened: `play` starts the clip over (ignored while it runs); `toggle` sets off the other way.
    pub fn trigger(&mut self) {
        match self.behavior.action {
            Action::Play => {
                if !self.running {
                    self.time = 0.0;
                    self.forward = true;
                    self.running = true;
                }
            }
            Action::Toggle | Action::Follow => {
                self.forward = !self.forward;
                self.running = true;
            }
            Action::Ragdoll => {}
        }
    }

    /// The watched flag now has `value`: head for the end of the clip when on, the start when off.
    pub fn follow(&mut self, value: bool) {
        let at_end = if value {
            self.time >= self.duration
        } else {
            self.time <= 0.0
        };
        if self.forward != value || !at_end {
            self.forward = value;
            self.running = !at_end;
        }
    }

    /// Moves on by one fixed step.
    pub fn step(&mut self) {
        if !self.running {
            return;
        }
        self.time += if self.forward { STEP } else { -STEP };
        if self.time >= self.duration && self.forward {
            self.time = self.duration;
            self.running = false;
        } else if self.time <= 0.0 && !self.forward {
            self.time = 0.0;
            self.running = false;
        }
    }

    /// Back to rest.
    pub fn reset(&mut self) {
        *self = Self::new(self.behavior.clone(), self.duration);
    }
}

/// Most parts that may carry a behaviour at once.
pub const MAX_REACTIVE_PARTS: usize = 64;

/// Smallest and largest `near` radius, in metres.
pub const RADIUS_RANGE: (f32, f32) = (0.1, 100.0);

/// The part a socket accepts: required by `socket`, and meaningless without it.
fn accepted_part(node: &Node, trigger: Trigger) -> Result<Option<String>, Fail> {
    let name = match node.params.get("accepts") {
        Some(Param::Str(text)) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        _ => None,
    };
    match (trigger, &name) {
        (Trigger::Socket, None) => Err(fail(
            "behavior.accepts",
            "trigger=\"socket\" needs `accepts`: the output name of the part that fits in this socket",
        )),
        (Trigger::Socket | Trigger::Collide, _) => Ok(name),
        (_, Some(_)) => Err(fail(
            "behavior.accepts",
            "`accepts` only goes with trigger=\"socket\" (the part that fits) or \"collide\" (the part to hit)",
        )),
        _ => Ok(name),
    }
}

/// A clip name written in a parameter: empty, or lowercase letters, digits and underscores starting with a letter.
fn clip_text(text: &str) -> Result<String, Fail> {
    let text = text.trim();
    let mut chars = text.chars();
    let valid = text.is_empty()
        || (chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && text.len() <= crate::anim::MAX_CLIP_NAME);
    if valid {
        Ok(text.to_owned())
    } else {
        Err(fail(
            "behavior.clip",
            format!(
                "clip name \"{text}\" must be lowercase letters, digits and underscores, starting with a letter"
            ),
        ))
    }
}

/// Reads the part's own clip and the clip it chains to, and checks they exist among the part's tracks. A listener
/// (`signal`) is told which of its clips to run by whoever sends, so it needs none of its own.
fn own_clips(node: &Node, input: &Value, trigger: Trigger) -> Result<(String, Vec<String>), Fail> {
    let text = |key: &str| match node.params.get(key) {
        Some(Param::Str(text)) => text.as_str(),
        _ => "",
    };
    let clip = clip_text(text("clip"))?;
    let has = |name: &str| input.animation.iter().any(|track| track.clip == name);
    if trigger != Trigger::Signal && !clip.is_empty() && !has(&clip) {
        return Err(fail(
            "behavior.clip",
            format!(
                "this part has no clip named \"{clip}\"; give its anim.track nodes clip=\"{clip}\" before behavior.on"
            ),
        ));
    }
    let mut then: Vec<String> = Vec::new();
    for name in text("then")
        .split([';', ','])
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let name = clip_text(name)?;
        if !has(&name) || name == clip || then.contains(&name) {
            return Err(fail(
                "behavior.then",
                format!(
                    "`then` lists clips of this part that have tracks, each once and none the first clip (\"{name}\" is not allowed)"
                ),
            ));
        }
        then.push(name);
    }
    if then.len() > 8 {
        return Err(fail(
            "behavior.then",
            "a sequence may chain at most 8 clips after the first",
        ));
    }
    Ok((clip, then))
}
