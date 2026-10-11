//! The interaction state machine: turns pointer frames into hover, select and grab events and into
//! the placements of held objects.
//!
//! One pointer hovers at most one target, grabs at most one target and holds it while its grab
//! button stays down. A target may be held by one pointer or two (two hands scale and turn it).
//! Everything is driven from [`Interaction::update`], once per frame, with every pointer that
//! exists that frame; a pointer missing from a frame is gone, and anything it held is let go.

use std::collections::BTreeMap;

use crate::grab::Anchor;
use crate::held::{Held, Holder, anchor_of};
use crate::pointer::{Event, Hit, Output, PointerFrame, PointerId, TargetId, World};

/// A press and release that moved the held object less than this (metres) still counts as a click.
const CLICK_SLOP: f32 = 0.02;

/// Closest a held object may be brought along the ray with `reach`.
const MIN_GRAB_DISTANCE: f32 = 0.05;

/// Which button a grab is held with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    Squeeze,
    Select,
}

/// What a pointer is doing between frames.
#[derive(Debug, Default)]
struct PointerState {
    hover: Option<TargetId>,
    hover_distance: f32,
    pressed_on: Option<TargetId>,
    grab: Option<(TargetId, Via)>,
    grab_distance: f32,
    was_select: bool,
    was_squeeze: bool,
}

/// The state machine. Create one, then call [`Interaction::update`] every frame.
#[derive(Debug)]
pub struct Interaction {
    max_distance: f32,
    pointers: BTreeMap<PointerId, PointerState>,
    held: BTreeMap<TargetId, Held>,
}

impl Interaction {
    /// A state machine whose pointers pick targets up to `max_distance` metres away.
    #[must_use]
    pub fn new(max_distance: f32) -> Self {
        Self {
            max_distance,
            pointers: BTreeMap::new(),
            held: BTreeMap::new(),
        }
    }

    /// What `pointer` currently hovers.
    #[must_use]
    pub fn hover_of(&self, pointer: PointerId) -> Option<TargetId> {
        self.pointers.get(&pointer).and_then(|state| state.hover)
    }

    /// The pointers currently holding `target` (empty when it is not held).
    #[must_use]
    pub fn holders_of(&self, target: TargetId) -> Vec<PointerId> {
        self.held
            .get(&target)
            .map(|held| held.holders.iter().map(|holder| holder.pointer).collect())
            .unwrap_or_default()
    }

    /// Every target that is held right now.
    #[must_use]
    pub fn held_targets(&self) -> Vec<TargetId> {
        self.held.keys().copied().collect()
    }

    /// Runs one frame. `frames` lists every pointer that exists this frame (order does not matter;
    /// if an id repeats the last entry wins).
    pub fn update(&mut self, frames: &[PointerFrame], world: &impl World) -> Output {
        let mut output = Output::default();
        let frames: BTreeMap<PointerId, &PointerFrame> = frames.iter().map(|f| (f.id, f)).collect();

        let gone: Vec<PointerId> = self
            .pointers
            .keys()
            .copied()
            .filter(|id| !frames.contains_key(id))
            .collect();
        for id in gone {
            self.remove_pointer(id, &frames, world, &mut output);
        }
        for (id, frame) in &frames {
            self.update_pointer(*id, frame, &frames, world, &mut output);
        }
        self.place_held(&frames, world, &mut output);
        output
    }

    fn update_pointer(
        &mut self,
        id: PointerId,
        frame: &PointerFrame,
        frames: &BTreeMap<PointerId, &PointerFrame>,
        world: &impl World,
        output: &mut Output,
    ) {
        let max_distance = self.max_distance;
        let state = self.pointers.entry(id).or_default();

        // Hover: what the ray points at, unless a grab pins the hover to the held target.
        if let Some((held, _)) = state.grab {
            state.hover = Some(held);
        } else {
            let hit: Option<Hit> = frame.ray.and_then(|ray| world.pick(&ray, max_distance));
            let target = hit.map(|h| h.target);
            if target != state.hover {
                if let Some(old) = state.hover {
                    output.events.push(Event::HoverEnd {
                        pointer: id,
                        target: old,
                    });
                }
                if let Some(new) = target {
                    output.events.push(Event::HoverStart {
                        pointer: id,
                        target: new,
                    });
                }
                state.hover = target;
            }
            state.hover_distance = hit.map_or(0.0, |h| h.distance);
        }
        let hover = state.hover;

        // Select button.
        if frame.select
            && !state.was_select
            && let Some(target) = hover
        {
            state.pressed_on = Some(target);
            output.events.push(Event::SelectStart {
                pointer: id,
                target,
            });
        }
        if !frame.select
            && state.was_select
            && let Some(target) = state.pressed_on.take()
        {
            let dragged = self
                .held
                .get(&target)
                .is_some_and(|held| held.carried() > CLICK_SLOP);
            output.events.push(Event::SelectEnd {
                pointer: id,
                target,
                click: hover == Some(target) && !dragged,
            });
        }

        // Grab buttons: squeeze always grabs; the primary button grabs for mouse-like pointers.
        let state = self.pointers.entry(id).or_default();
        let squeeze_down = frame.squeeze && !state.was_squeeze;
        let select_down = frame.select_grabs && frame.select && !state.was_select;
        let releasing = match state.grab {
            Some((_, Via::Squeeze)) => !frame.squeeze,
            Some((_, Via::Select)) => !frame.select,
            None => false,
        };
        let (was_select, was_squeeze) = (frame.select, frame.squeeze);
        if releasing {
            self.end_grab(id, frames, world, output);
        }
        let state = self.pointers.entry(id).or_default();
        state.was_select = was_select;
        state.was_squeeze = was_squeeze;
        if state.grab.is_none() && (squeeze_down || select_down) {
            let via = if squeeze_down {
                Via::Squeeze
            } else {
                Via::Select
            };
            self.begin_grab(id, frame, via, frames, world, output);
        } else if let Some((_, _)) = state.grab {
            state.grab_distance = (state.grab_distance + frame.reach).max(MIN_GRAB_DISTANCE);
        }
    }

    fn begin_grab(
        &mut self,
        id: PointerId,
        frame: &PointerFrame,
        via: Via,
        frames: &BTreeMap<PointerId, &PointerFrame>,
        world: &impl World,
        output: &mut Output,
    ) {
        let Some(state) = self.pointers.get_mut(&id) else {
            return;
        };
        let Some(target) = state.hover else {
            return;
        };
        if !world.grabbable(target) {
            return;
        }
        let distance = state.hover_distance;
        let Some(base) = anchor_of(frame, distance) else {
            return;
        };
        match self.held.get_mut(&target) {
            Some(held) if held.holders.len() >= 2 => return,
            Some(held) => {
                // A second hand joins: both hands are measured from now, from where the object is.
                held.baseline = held.current;
                let first = held.holders[0];
                if let (Some(other), Some(other_state)) = (
                    frames.get(&first.pointer),
                    self.pointers.get(&first.pointer),
                ) && let Some(anchor) = anchor_of(other, other_state.grab_distance)
                {
                    held.holders[0].base = anchor;
                }
                held.holders.push(Holder { pointer: id, base });
            }
            None => {
                let Some(placement) = world.placement(target) else {
                    return;
                };
                self.held.insert(target, Held::new(id, base, placement));
            }
        }
        if let Some(state) = self.pointers.get_mut(&id) {
            state.grab = Some((target, via));
            state.grab_distance = distance;
        }
        output.events.push(Event::GrabStart {
            pointer: id,
            target,
        });
    }

    fn end_grab(
        &mut self,
        id: PointerId,
        frames: &BTreeMap<PointerId, &PointerFrame>,
        world: &impl World,
        output: &mut Output,
    ) {
        let Some(state) = self.pointers.get_mut(&id) else {
            return;
        };
        let Some((target, _)) = state.grab.take() else {
            return;
        };
        let Some(held) = self.held.get_mut(&target) else {
            return;
        };
        held.holders.retain(|holder| holder.pointer != id);
        if held.holders.is_empty() {
            let placement = held.current;
            self.held.remove(&target);
            output.placements.push((target, placement));
            output.events.push(Event::GrabEnd {
                pointer: id,
                target,
                placement,
                snapped: world.snap(target, placement),
            });
            return;
        }
        // One hand remains: it carries on from where the object is now.
        held.baseline = held.current;
        let remaining = held.holders[0];
        let distance = self
            .pointers
            .get(&remaining.pointer)
            .map_or(0.0, |s| s.grab_distance);
        if let Some(anchor) = frames
            .get(&remaining.pointer)
            .and_then(|frame| anchor_of(frame, distance))
        {
            held.holders[0].base = anchor;
        }
    }

    /// A pointer that vanished: it stops hovering, its press is cancelled and it lets go.
    fn remove_pointer(
        &mut self,
        id: PointerId,
        frames: &BTreeMap<PointerId, &PointerFrame>,
        world: &impl World,
        output: &mut Output,
    ) {
        self.end_grab(id, frames, world, output);
        if let Some(state) = self.pointers.remove(&id) {
            if let Some(target) = state.pressed_on {
                output.events.push(Event::SelectEnd {
                    pointer: id,
                    target,
                    click: false,
                });
            }
            if let Some(target) = state.hover {
                output.events.push(Event::HoverEnd {
                    pointer: id,
                    target,
                });
            }
        }
    }

    /// Computes where every held target should be this frame.
    fn place_held(
        &mut self,
        frames: &BTreeMap<PointerId, &PointerFrame>,
        world: &impl World,
        output: &mut Output,
    ) {
        // A target that disappeared from the world cannot be held any more.
        let vanished: Vec<TargetId> = self
            .held
            .keys()
            .copied()
            .filter(|target| world.placement(*target).is_none())
            .collect();
        for target in vanished {
            self.release_vanished(target, frames, world, output);
        }
        for (target, held) in &mut self.held {
            let anchors: Vec<Option<Anchor>> = held
                .holders
                .iter()
                .map(|holder| {
                    let distance = self
                        .pointers
                        .get(&holder.pointer)
                        .map_or(0.0, |s| s.grab_distance);
                    frames
                        .get(&holder.pointer)
                        .and_then(|frame| anchor_of(frame, distance))
                })
                .collect();
            held.advance(&anchors);
            output.placements.push((*target, held.current));
        }
    }

    fn release_vanished(
        &mut self,
        target: TargetId,
        frames: &BTreeMap<PointerId, &PointerFrame>,
        world: &impl World,
        output: &mut Output,
    ) {
        let holders = self.holders_of(target);
        for pointer in holders {
            self.end_grab(pointer, frames, world, output);
        }
    }
}
