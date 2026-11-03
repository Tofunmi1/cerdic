//! One fixed behaviour step: who fires, whether their needs are met, flag changes, and clip triggers.
use bevy_ecs::{entity::Entity, system::Query};
use ve_modeling::{
    anim::{Action, Trigger, timer_due},
    flags::{self, MAX_FLAGS},
};

use super::{Behaviors, Reactive};

/// Applies a behaviour's flag changes (reversed when a held part is let go); the scene keeps at most `MAX_FLAGS` flags.
fn apply_sets(
    flags: &mut std::collections::BTreeMap<String, bool>,
    sets: &[flags::FlagSet],
    reverse: bool,
) {
    for set in sets {
        let current = flags.get(&set.name).copied().unwrap_or(false);
        let change = if reverse {
            flags::reverse(set.change)
        } else {
            set.change
        };
        if flags.contains_key(&set.name) || flags.len() < MAX_FLAGS {
            flags.insert(set.name.clone(), flags::apply(change, current));
        }
    }
}

/// Starts the clip a fired behaviour runs, if the part has it: it follows what is held or its watched flag, or runs as
/// triggered. Starting a sequence forward rewinds the clips it will chain to, so it can play again.
fn run_own_clip(
    part: &mut Reactive,
    behavior: &ve_modeling::anim::Behavior,
    held: Option<bool>,
    watched_on: bool,
) {
    let Some(reaction) = part
        .clip_mut(&behavior.clip)
        .filter(|reaction| reaction.duration > 0.0)
    else {
        return;
    };
    if let Some(held) = held {
        reaction.follow(held);
    } else if behavior.action == Action::Follow {
        reaction.follow(watched_on);
    } else {
        reaction.trigger();
    }
    if reaction.running && reaction.forward {
        for name in &behavior.then {
            if let Some(later) = part.clip_mut(name) {
                later.reset();
            }
        }
    }
}

/// Moves every clip on by one fixed step, then starts the next clip of any sequence whose clip just finished running forward.
fn advance_clips(parts: &mut Query<(Entity, &mut Reactive)>) {
    let mut chained: Vec<(Entity, String)> = Vec::new();
    for (entity, mut part) in &mut *parts {
        let sequence: Vec<String> = std::iter::once(part.reaction.behavior.clip.clone())
            .chain(part.reaction.behavior.then.clone())
            .collect();
        for (name, reaction) in part.all_mut() {
            let was_running = reaction.running;
            reaction.step();
            if was_running && !reaction.running && reaction.forward && sequence.len() > 1 {
                let next = sequence
                    .iter()
                    .position(|clip| clip == name)
                    .and_then(|at| sequence.get(at + 1));
                chained.extend(next.map(|clip| (entity, clip.clone())));
            }
        }
    }
    for (entity, clip) in chained {
        if let Ok((_, mut part)) = parts.get_mut(entity)
            && let Some(reaction) = part.clip_mut(&clip)
        {
            reaction.follow(true);
        }
    }
}

/// Runs one step. Everything that fired is handled in node-name order, so the same triggers on the same step
/// give the same flags and clip times on every run; flag changes made in this step reach watchers in the next.
pub(super) fn step(behaviors: &mut Behaviors, parts: &mut Query<(Entity, &mut Reactive)>) {
    let pending = std::mem::take(&mut behaviors.pending);
    let clicks = std::mem::take(&mut behaviors.clicks);
    let pressed = std::mem::take(&mut behaviors.pressed);
    let changed = std::mem::take(&mut behaviors.changed);
    let grabs = std::mem::take(&mut behaviors.grabs);
    let releases = std::mem::take(&mut behaviors.releases);
    let docks = std::mem::take(&mut behaviors.docks);
    let collided = std::mem::take(&mut behaviors.collided);
    let before = behaviors.flags.clone();
    let play_step = behaviors.play_steps;

    // Who fires this step; for a `hold` part, whether it was just picked up (true) or let go (false).
    let mut fired: Vec<(String, Entity, Option<bool>)> = parts
        .iter()
        .filter_map(|(entity, part)| {
            let behavior = &part.reaction.behavior;
            let held = match behavior.trigger {
                Trigger::Grab if grabs.contains(&entity) => Some(None),
                Trigger::Release if releases.contains(&entity) => Some(None),
                Trigger::Hold if releases.contains(&entity) => Some(Some(false)),
                Trigger::Hold if grabs.contains(&entity) => Some(Some(true)),
                Trigger::Socket => docks
                    .iter()
                    .find(|(socket, _)| *socket == entity)
                    .map(|(_, docked)| Some(*docked)),
                Trigger::Select if clicks.contains(&entity) => Some(None),
                Trigger::Collide
                    if collided.iter().any(|(me, other)| {
                        *me == part.name
                            && behavior.accepts.as_deref().is_none_or(|want| want == other)
                    }) =>
                {
                    Some(None)
                }
                Trigger::Timer if timer_due(behavior.after, behavior.every, play_step) => {
                    Some(None)
                }
                Trigger::Flag
                    if behavior
                        .watch
                        .as_ref()
                        .is_some_and(|flag| changed.contains(flag)) =>
                {
                    Some(None)
                }
                _ if pending.contains(&entity) || pressed.contains(&part.node) => Some(None),
                _ => None,
            }?;
            Some((part.node.clone(), entity, held))
        })
        .collect();
    fired.sort();
    fired.dedup();

    for (_, source, held) in fired {
        let Ok((_, mut part)) = parts.get_mut(source) else {
            continue;
        };
        let behavior = part.reaction.behavior.clone();
        let met = behavior
            .needs
            .iter()
            .all(|need| behaviors.flags.get(&need.name).copied().unwrap_or(false) == need.value);
        // Letting go always undoes what picking up did, whatever the flags have become since.
        if !met && held != Some(false) {
            continue;
        }
        apply_sets(&mut behaviors.flags, &behavior.sets, held == Some(false));
        let watched_on = behavior
            .watch
            .as_ref()
            .and_then(|flag| behaviors.flags.get(flag))
            .copied()
            .unwrap_or(false);
        run_own_clip(&mut part, &behavior, held, watched_on);
        if behavior.action == Action::Ragdoll
            && held != Some(false)
            && let Some(name) = &behavior.target
        {
            crate::world_api::request_ragdoll(name);
        }
        if let Some(name) = &behavior.target {
            for (_, mut listener) in &mut *parts {
                if &listener.name == name
                    && listener.reaction.behavior.trigger == Trigger::Signal
                    && let Some(reaction) = listener.clip_mut(&behavior.target_clip)
                {
                    reaction.trigger();
                }
            }
        }
    }

    behaviors.changed = behaviors
        .flags
        .iter()
        .filter(|(name, on)| before.get(*name).copied().unwrap_or(false) != **on)
        .map(|(name, _)| name.clone())
        .collect();
    advance_clips(parts);
    behaviors.steps += 1;
    behaviors.play_steps += 1;
}
