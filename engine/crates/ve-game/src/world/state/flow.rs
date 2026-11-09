//! Game flow: named states (a frozen one stops the simulation), levels, checkpoints and restart. See `parse/flow.rs` for the declarations.
//!
//! A frozen state (or any state once the game is won or lost) steps no physics, rules on `tick`, timers or scripts: only the inputs the game
//! declared as keys are heard, so a title, a pause or a game-over screen can be left with a press and nothing moves behind it. Entering a
//! state raises the event `state:NAME` (a rule on it runs once), loading a level raises `level:NAME`. Winning or losing enters the states
//! called `win` and `game_over` when the game has them. `restart` rebuilds the world from its definition and seed.
use std::collections::{BTreeMap, BTreeSet};

use super::{Event, MAX_ENTITIES, World};
use crate::{Effect, GameError, Outcome, Status};

/// The flow's own state, kept in saves and in the hash.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct FlowState {
    /// The current flow state's name (empty when the game declares none).
    pub(super) state: String,
    /// The loaded level's name (empty when the game declares none).
    pub(super) level: String,
    /// The entities that belong to the loaded level; they go when another level loads.
    pub(super) level_entities: BTreeSet<u32>,
    /// Where `checkpoint` stored each entity.
    pub(super) checkpoint: BTreeMap<u32, [f64; 3]>,
    /// The UI button that has the focus (empty until a button is chosen; the first visible button then counts).
    #[serde(default)]
    pub(super) focus: String,
}

impl World {
    /// The current flow state's name (empty when the game declares no states).
    #[must_use]
    pub fn flow_state(&self) -> &str {
        &self.flow.state
    }

    /// The loaded level's name (empty when the game declares no levels).
    #[must_use]
    pub fn level_name(&self) -> &str {
        &self.flow.level
    }

    /// Whether the game declares flow states (a flow game keeps taking steps after it ends, so its win and game-over screens work).
    #[must_use]
    pub fn has_flow(&self) -> bool {
        !self.def.states.is_empty()
    }

    /// Fixed steps taken so far (frozen states do not count).
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Whether the simulation stands still: the game has states and is in a frozen one, or has ended.
    pub(super) fn is_frozen(&self) -> bool {
        !self.def.states.is_empty()
            && (self.status != Status::Playing || self.def.states.iter().any(|state| state.frozen && state.name == self.flow.state))
    }

    /// Puts a new world into its first state and loads its first level.
    pub(super) fn start_flow(&mut self) {
        if let Some(first) = self.def.states.first() {
            self.flow.state.clone_from(&first.name);
        }
        if let Some(first) = self.def.levels.first().map(|level| level.name.clone()) {
            self.load_level(&first, None);
        }
    }

    /// Spawns one `spawn` line (honouring the entity budget); returns the ids it made.
    pub(super) fn spawn_line(&mut self, spawn: &crate::SpawnDef) -> Vec<u32> {
        let first = self.next_id;
        for _ in 0..spawn.count {
            if self.entities.len() > MAX_ENTITIES {
                break;
            }
            let mut position = spawn.position;
            if spawn.spread > 0.0 {
                position[0] += (self.random() - 0.5) * spawn.spread;
                position[2] += (self.random() - 0.5) * spawn.spread;
            }
            if let Some(kind) = self.def.kinds.iter().find(|kind| kind.name == spawn.kind) {
                let props = kind.props.clone();
                self.spawn(&spawn.kind, position, props, spawn.heading);
            }
        }
        (first..self.next_id).collect()
    }

    fn load_level(&mut self, name: &str, queue: Option<&mut Vec<Event>>) {
        let gone = std::mem::take(&mut self.flow.level_entities);
        self.entities.retain(|entity| !gone.contains(&entity.id));
        self.agents.retain(|id, _| !gone.contains(id));
        self.flow.checkpoint.clear();
        let spawns: Vec<crate::SpawnDef> = self.def.spawns.iter().filter(|spawn| spawn.level.as_deref() == Some(name)).cloned().collect();
        for spawn in &spawns {
            let made = self.spawn_line(spawn);
            self.flow.level_entities.extend(made);
        }
        if let Some(start) = self.def.levels.iter().find(|level| level.name == name).and_then(|level| level.start) {
            let heroes: Vec<u32> = self.entities.iter().filter(|e| self.matches("tag:hero", e)).map(|e| e.id).collect();
            for entity in self.entities.iter_mut().filter(|e| heroes.contains(&e.id)) {
                entity.position = start;
                entity.velocity = [0.0; 3];
            }
        }
        name.clone_into(&mut self.flow.level);
        if let Some(queue) = queue {
            queue.push(Event { name: format!("level:{name}"), a: None, b: None, depth: 1 });
        }
    }

    fn enter_state(&mut self, name: &str, queue: &mut Vec<Event>) {
        if self.flow.state != name {
            name.clone_into(&mut self.flow.state);
            queue.push(Event { name: format!("state:{name}"), a: None, b: None, depth: 1 });
        }
    }

    /// Runs the flow actions (`go`, `restart`, `level`, `checkpoint`, `respawn`); returns whether `action` was one of them.
    pub(super) fn flow_action(&mut self, action: &str, event: &Event, queue: &mut Vec<Event>) -> bool {
        if let Some(name) = action.strip_prefix("go:") {
            self.enter_state(name, queue);
        } else if action == "restart" {
            // by default a restart goes to the first state that is not frozen (the game proper), or the first state
            let target = self.def.states.iter().find(|state| !state.frozen).or_else(|| self.def.states.first()).map(|state| state.name.clone());
            self.pending_restart = Some(target.unwrap_or_default());
        } else if let Some(name) = action.strip_prefix("restart:") {
            self.pending_restart = Some(name.to_owned());
        } else if let Some(name) = action.strip_prefix("level:") {
            self.load_level(name, Some(queue));
        } else if action == "checkpoint" || action.starts_with("checkpoint:") {
            let selector = action.strip_prefix("checkpoint:").unwrap_or("tag:hero");
            let ids = self.select_ids(selector, event);
            self.flow.checkpoint = self.entities.iter().filter(|e| ids.contains(&e.id)).map(|e| (e.id, e.position)).collect();
        } else if action == "respawn" || action.starts_with("respawn:") {
            let stored = self.flow.checkpoint.clone();
            for entity in self.entities.iter_mut().filter(|e| stored.contains_key(&e.id)) {
                entity.position = stored[&entity.id];
                entity.velocity = [0.0; 3];
            }
        } else {
            return false;
        }
        true
    }

    /// One step of a frozen game: only the declared key inputs are heard (so a press can leave the state), nothing else advances.
    pub(super) fn step_frozen(&mut self, input: &crate::Inputs) -> Result<Outcome, GameError> {
        let mut effects: Vec<Effect> = Vec::new();
        let mut queue: Vec<Event> = input
            .pressed
            .iter()
            .filter(|name| self.def.inputs.iter().any(|declaration| declaration.name == **name && declaration.key))
            .map(|name| Event { name: format!("input:{name}"), a: None, b: None, depth: 0 })
            .collect();
        self.ui_input(&input.pressed, &mut queue, &mut effects)?;
        let actions = self.process_events(&mut queue, &mut effects)?;
        self.finish_flow(&mut effects)?;
        Ok(Outcome { status: self.status, effects, events_count: actions })
    }

    /// After a step: a pending restart rebuilds the world, and a won or lost game enters its `win` or `game_over` state.
    pub(super) fn finish_flow(&mut self, effects: &mut Vec<Effect>) -> Result<(), GameError> {
        if let Some(target) = self.pending_restart.take() {
            let (def, seed) = (self.def.clone(), self.seed);
            *self = Self::new(&def, seed);
            let mut queue = Vec::new();
            if !target.is_empty() {
                self.enter_state(&target, &mut queue);
            }
            self.process_events(&mut queue, effects)?;
            return Ok(());
        }
        let ending = match self.status {
            Status::Won => "win",
            Status::Lost => "game_over",
            Status::Playing => return Ok(()),
        };
        if self.def.states.iter().any(|state| state.name == ending) && self.flow.state != ending {
            let mut queue = Vec::new();
            self.enter_state(ending, &mut queue);
            self.process_events(&mut queue, effects)?;
        }
        Ok(())
    }
}
