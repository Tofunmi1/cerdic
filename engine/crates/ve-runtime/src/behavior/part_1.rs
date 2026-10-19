mod step;

use std::{cell::RefCell, collections::BTreeMap};

use bevy_camera::Camera3d;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{Or, With},
    resource::Resource,
    system::{Local, Query, Res, ResMut},
};
use bevy_math::Vec3;
use bevy_time::{Real, Time};
use bevy_transform::components::{GlobalTransform, Transform};
use serde_json::json;

use crate::interact::{Interactable, InteractionState};
use ve_modeling::anim::{Action, Behavior, Reaction, STEP, Trigger};

/// Most steps run in one frame, so a stalled tab does not replay minutes of motion at once.
const MAX_STEPS_PER_FRAME: f32 = 5.0;

/// A part that reacts: its clip time is its own, not the shared clock's.
#[derive(Component, Clone, Debug)]
pub(crate) struct Reactive {
    /// The state of the part's main clip (the tracks with no clip name).
    pub(crate) reaction: Reaction,
    /// The state of each named clip (ADR 0031), in name order; each plays on its own.
    pub(crate) clips: Vec<(String, Reaction)>,
    /// Where the part's centre is at rest, for `near` triggers.
    pub(crate) centre: Vec3,
    pub(crate) node: String,
    /// The part's output name, which other parts' `target` refers to.
    pub(crate) name: String,
    inside: bool,
}

impl Reactive {
    /// A part at rest; `durations` is the length in seconds of each clip, by name ("" is the main clip).
    pub(crate) fn new(
        behavior: Behavior,
        durations: &[(String, f32)],
        centre: Vec3,
        node: String,
        name: String,
    ) -> Self {
        let length = |wanted: &str| {
            durations
                .iter()
                .find(|(clip, _)| clip == wanted)
                .map_or(0.0, |(_, seconds)| *seconds)
        };
        let clips = durations
            .iter()
            .filter(|(clip, _)| !clip.is_empty())
            .map(|(clip, seconds)| (clip.clone(), Reaction::new(behavior.clone(), *seconds)))
            .collect();
        Self {
            reaction: Reaction::new(behavior, length("")),
            clips,
            centre,
            node,
            name,
            inside: false,
        }
    }

    /// The state of the clip called `clip` ("" is the main one), if the part has it.
    pub(crate) fn clip_mut(&mut self, clip: &str) -> Option<&mut Reaction> {
        if clip.is_empty() {
            return Some(&mut self.reaction);
        }
        self.clips
            .iter_mut()
            .find(|(name, _)| name == clip)
            .map(|(_, reaction)| reaction)
    }

    /// Seconds into the clip called `clip` (0 for a clip the part does not have).
    pub(crate) fn time_of(&self, clip: &str) -> f32 {
        if clip.is_empty() {
            return self.reaction.time;
        }
        self.clips
            .iter()
            .find(|(name, _)| name == clip)
            .map_or(0.0, |(_, reaction)| reaction.time)
    }

    /// Every clip's state, the main one first.
    pub(crate) fn all_mut(&mut self) -> impl Iterator<Item = (&str, &mut Reaction)> {
        std::iter::once(("", &mut self.reaction)).chain(
            self.clips
                .iter_mut()
                .map(|(name, reaction)| (name.as_str(), reaction)),
        )
    }
}

/// Play mode, the triggers waiting for the next step, and the step clock.
#[derive(Resource, Default)]
pub(crate) struct Behaviors {
    pub(crate) play: bool,
    /// Parts whose camera trigger fired.
    pending: Vec<Entity>,
    /// Parts clicked in play mode.
    clicks: Vec<Entity>,
    /// Parts picked up, and parts let go, in play mode.
    grabs: Vec<Entity>,
    releases: Vec<Entity>,
    /// Parts in a hand right now (physics follows the hand while a body is held).
    held: Vec<Entity>,
    /// Fixed steps run in the last frame (physics takes the same number).
    stepped: u32,
    /// Pairs of part names whose bodies just touched, both ways round, for `collide` triggers in the next step.
    collided: Vec<(String, String)>,
    /// Parts sitting in a socket, as (part, socket).
    docked: Vec<(Entity, Entity)>,
    /// Sockets that something just docked in (true) or left (false), for the next step.
    docks: Vec<(Entity, bool)>,
    pressed: Vec<String>,
    /// Scene flags (ADR 0029): absent means off. Reset when play mode starts or stops.
    flags: BTreeMap<String, bool>,
    /// Flags that changed in the last step (or by the page), which `flag` triggers react to in the next.
    changed: Vec<String>,
    accumulator: f32,
    steps: u64,
    /// Steps run since play mode last started (timers count from here).
    play_steps: u64,
    /// Clicks received in play mode, for the page to tell that one arrived.
    clicks_seen: u64,
}

impl Behaviors {
    /// Whether play mode is on.
    pub(crate) fn is_playing(&self) -> bool {
        self.play
    }

    /// How many fixed steps the last frame ran.
    pub(crate) fn stepped(&self) -> u32 {
        self.stepped
    }

    /// Notes that the bodies of parts `a` and `b` started touching (queued for the next step; the list is bounded).
    pub(crate) fn note_collision(&mut self, a: &str, b: &str) {
        if self.play && self.collided.len() < 512 {
            self.collided.push((a.to_owned(), b.to_owned()));
            self.collided.push((b.to_owned(), a.to_owned()));
        }
    }

    /// Whether a hand is holding the part.
    pub(crate) fn is_held(&self, entity: Entity) -> bool {
        self.held.contains(&entity)
    }

    /// A click on `entity` in play mode: it acts if the part reacts to being selected.
    pub(crate) fn clicked(&mut self, entity: Entity) {
        if self.play {
            self.clicks.push(entity);
            self.clicks_seen += 1;
        }
    }
}

enum Request {
    Play(bool),
    Press(String),
    Flag(String, bool),
}

thread_local! {
    static REQUESTS: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
    static STATE: RefCell<String> = RefCell::new(String::from("{\"play\":false,\"steps\":0,\"parts\":[]}"));
}

/// Turns play mode on or off (leaving it puts every behaviour part back at rest).
pub(crate) fn set_play(on: bool) {
    REQUESTS.with(|r| r.borrow_mut().push(Request::Play(on)));
}

/// Triggers the behaviour of the part built by `node`, as if it were clicked (play mode only).
pub(crate) fn press(node: &str) {
    REQUESTS.with(|r| r.borrow_mut().push(Request::Press(node.to_owned())));
}

/// Sets a scene flag from the page (play mode only), as if a behaviour had; watchers react in the next step.
pub(crate) fn set_flag(name: &str, on: bool) {
    REQUESTS.with(|r| r.borrow_mut().push(Request::Flag(name.to_owned(), on)));
}

/// `{ play, steps, clicks, flags, parts: [{ node, time, forward, running }] }` as of the last frame.
pub(crate) fn state() -> String {
    STATE.with(|s| s.borrow().clone())
}

/// Takes the page's requests.
pub(crate) fn intake(
    mut behaviors: ResMut<Behaviors>,
    mut parts: Query<(&mut Reactive, &mut Transform)>,
) {
    let requests = REQUESTS.with(|r| std::mem::take(&mut *r.borrow_mut()));
    for request in requests {
        match request {
            Request::Play(on) => {
                behaviors.play = on;
                if on {
                    // Nothing stays selected (and no selection gizmo catches the clicks) while playing.
                    crate::selection::set(None, std::collections::BTreeSet::new());
                }
                behaviors.pending.clear();
                behaviors.clicks.clear();
                behaviors.grabs.clear();
                behaviors.held.clear();
                behaviors.collided.clear();
                behaviors.docked.clear();
                behaviors.docks.clear();
                behaviors.releases.clear();
                behaviors.pressed.clear();
                behaviors.flags.clear();
                behaviors.changed.clear();
                behaviors.play_steps = 0;
                for (mut part, mut transform) in &mut parts {
                    for (_, reaction) in part.all_mut() {
                        reaction.reset();
                    }
                    part.inside = false;
                    // A part that was carried about goes back where it was authored (animated parts are
                    // re-posed from their clips every frame anyway).
                    *transform = Transform::IDENTITY;
                }
            }
            Request::Press(node) if behaviors.play => behaviors.pressed.push(node),
            Request::Flag(name, on) if behaviors.play => {
                let known = behaviors.flags.contains_key(&name);
                if ve_modeling::flags::name(&name).is_ok()
                    && (known || behaviors.flags.len() < ve_modeling::flags::MAX_FLAGS)
                    && behaviors.flags.insert(name.clone(), on).unwrap_or(false) != on
                {
                    behaviors.changed.push(name);
                }
            }
            Request::Press(_) | Request::Flag(..) => {}
        }
    }
}

/// Index of the B or Y face button in a controller's button list (xr-standard mapping).
const XR_PLAY_BUTTON: usize = 5;

/// In a headset the B or Y button on either controller turns play mode on and off, on the press itself.
pub(crate) fn xr_toggle(
    input: Res<ve_xr_web::XrInput>,
    behaviors: Res<Behaviors>,
    mut was_pressed: Local<bool>,
) {
    let pressed = input
        .controllers
        .iter()
        .any(|controller| controller.buttons[XR_PLAY_BUTTON] > 0.5);
    if pressed && !*was_pressed {
        set_play(!behaviors.play);
    }
    *was_pressed = pressed;
}
