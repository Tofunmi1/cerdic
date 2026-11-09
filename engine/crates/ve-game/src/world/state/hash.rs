//! Canonical hashing and snapshots.
use super::{Entity, World};
use crate::{GameDef, GameError, Status};
use std::collections::{BTreeMap, BTreeSet};

/// Serializable deterministic state: [`Snapshot::to_json`] and [`Snapshot::from_json`] make a save game of it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub(super) definition: u64,
    pub(super) entities: Vec<Entity>,
    pub(super) vars: BTreeMap<String, f64>,
    pub(super) tick: u64,
    pub(super) rng: u64,
    pub(super) next_id: u32,
    pub(super) status: Status,
    pub(super) overlaps: BTreeSet<(u32, u32)>,
    pub(super) timers: BTreeMap<(u64, String), bool>,
    pub(super) rule_fires: Vec<u64>,
    pub(super) hands_state: super::hands::HandsState,
    pub(super) motion_history: BTreeMap<u32, Vec<[f64; 3]>>,
    pub(super) agents: Option<BTreeMap<u32, super::agents::AgentState>>,
    pub(super) tools: BTreeMap<u32, super::swing::ToolState>,
    pub(super) swing_latches: BTreeSet<(u32, u32)>,
    pub(super) out_balls: BTreeSet<u32>,
    /// The scripts' persistent `state` as JSON (absent when the game has no scripts or the save is older).
    #[serde(default)]
    pub(super) script_state: Option<String>,
    /// Flow state, level and checkpoint (absent in saves from before flow existed).
    #[serde(default)]
    pub(super) flow: super::flow::FlowState,
    /// Weapon magazines and timers (absent in saves from before weapons existed).
    #[serde(default)]
    pub(super) combat: super::combat::CombatState,
}

impl Snapshot {
    /// The save game as JSON text: everything the deterministic state holds (entities, variables, tick, random stream, timers, hands,
    /// agents), and the digest of the game definition it belongs to so a save cannot be loaded into a different game.
    ///
    /// Every field is plain data, so writing cannot fail in practice; if it ever did the text would be `{}`, which `from_json` refuses.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Reads a save written by [`Snapshot::to_json`].
    ///
    /// # Errors
    /// [`GameError::InvalidInput`] when the text is not a save game.
    pub fn from_json(text: &str) -> Result<Self, GameError> {
        serde_json::from_str(text)
            .map_err(|error| GameError::InvalidInput(format!("not a save game: {error}")))
    }
}

impl World {
    /// Stable FNV-1a digest over canonical state bytes.
    #[must_use]
    pub fn hash(&self) -> u64 {
        let mut bytes = Vec::new();
        if has_rolling(&self.def) {
            self.append_rolling_hash(&mut bytes);
        }
        if !self.def.agents.is_empty() {
            for (id, state) in &self.agents {
                bytes.extend_from_slice(&id.to_le_bytes());
                state.append_hash(&mut bytes);
            }
        }
        bytes.extend_from_slice(&self.tick.to_le_bytes());
        bytes.extend_from_slice(&self.rng.to_le_bytes());
        bytes.extend_from_slice(&self.next_id.to_le_bytes());
        bytes.push(match self.status {
            Status::Playing => 0,
            Status::Won => 1,
            Status::Lost => 2,
        });
        for entity in &self.entities {
            bytes.extend_from_slice(&entity.id.to_le_bytes());
            append_string(&mut bytes, &entity.kind);
            for value in entity.position.into_iter().chain(entity.velocity) {
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            for (name, value) in &entity.props {
                append_string(&mut bytes, name);
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        if self.def.kinds.iter().any(|kind| kind.fall) {
            for (id, samples) in &self.motion_history {
                bytes.extend_from_slice(&id.to_le_bytes());
                for p in samples {
                    for v in p {
                        bytes.extend_from_slice(&v.to_bits().to_le_bytes());
                    }
                }
            }
        }
        for (name, value) in &self.vars {
            append_string(&mut bytes, name);
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        for pair in &self.overlaps {
            bytes.extend_from_slice(&pair.0.to_le_bytes());
            bytes.extend_from_slice(&pair.1.to_le_bytes());
        }
        for (tick, name) in self.timers.keys() {
            bytes.extend_from_slice(&tick.to_le_bytes());
            append_string(&mut bytes, name);
        }
        for fires in &self.rule_fires {
            bytes.extend_from_slice(&fires.to_le_bytes());
        }
        for side in &self.hands_state.sides {
            bytes.push(u8::from(side.grip));
            bytes.push(u8::from(side.trigger));
            if let Some(held) = &side.held {
                bytes.push(1);
                bytes.extend_from_slice(&held.id.to_le_bytes());
                for value in held.offset {
                    bytes.extend_from_slice(&value.to_bits().to_le_bytes());
                }
            } else {
                bytes.push(0);
            }
        }
        if let Some(state) = self.script_state() {
            append_string(&mut bytes, &state);
        }
        for (key, state) in &self.combat.weapons {
            append_string(&mut bytes, key);
            for value in [state.ammo, state.cooldown, state.reloading] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        if !self.def.states.is_empty() || !self.def.levels.is_empty() || !self.def.ui.is_empty() {
            append_string(&mut bytes, &self.flow.state);
            append_string(&mut bytes, &self.flow.level);
            append_string(&mut bytes, &self.flow.focus);
            for (id, position) in &self.flow.checkpoint {
                bytes.extend_from_slice(&id.to_le_bytes());
                for value in position {
                    bytes.extend_from_slice(&value.to_bits().to_le_bytes());
                }
            }
        }
        fnv(&bytes)
    }

    /// Capture state for rollback.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            definition: definition_hash(&self.def),
            entities: self.entities.clone(),
            vars: self.vars.clone(),
            tick: self.tick,
            rng: self.rng,
            next_id: self.next_id,
            status: self.status,
            overlaps: self.overlaps.clone(),
            timers: self.timers.clone(),
            rule_fires: self.rule_fires.clone(),
            hands_state: self.hands_state.clone(),
            motion_history: self.motion_history.clone(),
            agents: (!self.def.agents.is_empty()).then(|| self.agents.clone()),
            tools: self.tools.clone(),
            swing_latches: self.swing_latches.clone(),
            out_balls: self.out_balls.clone(),
            script_state: self.script_state(),
            flow: self.flow.clone(),
            combat: self.combat.clone(),
        }
    }

    /// Restore a compatible snapshot.
    ///
    /// # Errors
    ///
    /// Returns `SnapshotMismatch` when the snapshot came from another definition.
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<(), GameError> {
        if snapshot.definition != definition_hash(&self.def) {
            return Err(GameError::SnapshotMismatch);
        }
        self.restore_unchecked(snapshot);
        Ok(())
    }

    fn append_rolling_hash(&self, bytes: &mut Vec<u8>) {
        for entity in &self.entities {
            for value in entity.orientation {
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        append_count(bytes, self.tools.len());
        for (id, state) in &self.tools {
            bytes.extend_from_slice(&id.to_le_bytes());
            for value in state.head {
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            append_count(bytes, state.history.len());
            for position in &state.history {
                for value in position {
                    bytes.extend_from_slice(&value.to_bits().to_le_bytes());
                }
            }
        }
        append_count(bytes, self.swing_latches.len());
        for (tool, ball) in &self.swing_latches {
            bytes.extend_from_slice(&tool.to_le_bytes());
            bytes.extend_from_slice(&ball.to_le_bytes());
        }
        append_count(bytes, self.out_balls.len());
        for id in &self.out_balls {
            bytes.extend_from_slice(&id.to_le_bytes());
        }
    }
}

fn has_rolling(def: &GameDef) -> bool {
    def.kinds
        .iter()
        .any(|kind| kind.roll || kind.swing || kind.pocket)
}
fn append_count(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(&u64::try_from(count).unwrap_or(u64::MAX).to_le_bytes());
}

fn definition_hash(def: &GameDef) -> u64 {
    let mut bytes = Vec::new();
    if has_rolling(def) {
        append_string(&mut bytes, &def.describe());
    }
    append_string(&mut bytes, &def.name);
    if !def.agents.is_empty() {
        for agent in &def.agents {
            append_string(&mut bytes, &agent.to_text());
        }
    }
    bytes.extend_from_slice(&def.ground.to_bits().to_le_bytes());
    for var in &def.vars {
        append_string(&mut bytes, &var.name);
        bytes.extend_from_slice(&var.value.to_bits().to_le_bytes());
    }
    for kind in &def.kinds {
        append_string(&mut bytes, &kind.name);
        bytes.extend_from_slice(&kind.size.to_bits().to_le_bytes());
        bytes.extend_from_slice(&kind.speed.to_bits().to_le_bytes());
        bytes.push(u8::from(kind.grab));
        bytes.push(u8::from(kind.fall));
        bytes.extend_from_slice(&kind.bounce.to_bits().to_le_bytes());
        for tag in &kind.tags {
            append_string(&mut bytes, tag);
        }
        if let Some(part) = &kind.part {
            append_string(&mut bytes, part);
        }
        for (name, value) in &kind.props {
            append_string(&mut bytes, name);
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    for spawn in &def.spawns {
        append_string(&mut bytes, &spawn.kind);
        for value in spawn.position {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&spawn.count.to_le_bytes());
        bytes.extend_from_slice(&spawn.spread.to_bits().to_le_bytes());
    }
    for input in &def.inputs {
        append_string(&mut bytes, &input.name);
        for axis in &input.axes {
            append_string(&mut bytes, axis);
        }
        if let Some(target) = &input.target {
            append_string(&mut bytes, target);
        }
        bytes.push(u8::from(input.key));
        bytes.push(u8::from(input.follow));
        bytes.push(match input.hand_follow {
            Some(crate::HandSide::Left) => 1,
            Some(crate::HandSide::Right) => 2,
            None => 0,
        });
    }
    for rule in &def.rules {
        append_string(&mut bytes, &rule.event);
        for action in &rule.actions {
            append_string(&mut bytes, action);
        }
        if let Some(when) = &rule.when {
            append_string(&mut bytes, when);
        }
        if let Some(a) = &rule.a {
            append_string(&mut bytes, a);
        }
        if let Some(b) = &rule.b {
            append_string(&mut bytes, b);
        }
        if let Some(every) = rule.every {
            bytes.extend_from_slice(&every.to_le_bytes());
        }
    }
    for goal in &def.goals {
        bytes.push(u8::from(goal.win));
        append_string(&mut bytes, &goal.when);
    }
    fnv(&bytes)
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
