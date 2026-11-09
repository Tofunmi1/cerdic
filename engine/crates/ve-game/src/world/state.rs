//! World state and deterministic simulation.
mod actions;
mod agents;
mod commands;
mod combat;
mod controller;
mod controller_anim;
mod events;
mod flow;
mod follow;
mod ui_input;
mod hands;
mod hash;
mod hud;
mod movement;
mod party;
mod queries;
mod raycast;
mod ranged;
mod rolling;
mod script;
mod swing;
pub(crate) use self::follow::matches_selector;
use crate::{GameDef, GameError, GroundField, Inputs, Outcome, Status};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const MAX_ENTITIES: usize = 2048;
const MAX_ACTIONS: usize = 4096;

/// Runtime entity.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Entity {
    /// Stable identifier.
    pub id: u32,
    /// Kind name.
    pub kind: String,
    /// Position in world coordinates.
    pub position: [f64; 3],
    /// Yaw in radians about +Y; zero faces +Z.
    pub heading: f64,
    /// World-space quaternion in x, y, z, w order.
    pub orientation: [f64; 4],
    /// Velocity in units per second.
    pub velocity: [f64; 3],
    /// Numeric properties.
    pub props: BTreeMap<String, f64>,
}

pub use hash::Snapshot;

/// Mutable game simulation.
pub struct World {
    def: GameDef,
    entities: Vec<Entity>,
    vars: BTreeMap<String, f64>,
    tick: u64,
    rng: u64,
    next_id: u32,
    status: Status,
    overlaps: BTreeSet<(u32, u32)>,
    timers: BTreeMap<(u64, String), bool>,
    rule_fires: Vec<u64>,
    hands_state: hands::HandsState,
    motion_history: BTreeMap<u32, Vec<[f64; 3]>>,
    agents: BTreeMap<u32, agents::AgentState>,
    nav: Option<Arc<ve_nav::NavMesh>>,
    ground: Option<Arc<GroundField>>,
    tools: BTreeMap<u32, swing::ToolState>,
    swing_latches: BTreeSet<(u32, u32)>,
    out_balls: BTreeSet<u32>,
    script: Option<script::Handle>,
    script_failure: Option<String>,
    /// How the game began, kept when it has scripts so `game.reset()` can start it over.
    initial: Option<Snapshot>,
    /// The seed the world was made with, so `restart` rebuilds the same game.
    seed: u64,
    /// Flow state, level and checkpoint (see `flow.rs`).
    flow: flow::FlowState,
    /// A restart asked for during this step, with the state to enter afterwards.
    pending_restart: Option<String>,
    /// What the page calls each action on the active device (the host's wording, not game state).
    ui_labels: BTreeMap<String, String>,
    /// Every weapon's magazine and timers (see `combat.rs`).
    combat: combat::CombatState,
    /// The camera axes (`camera.yaw`, `camera.pitch`, `camera.x/y/z`) of the latest step, for aiming a weapon through the crosshair.
    camera_axes: BTreeMap<String, f64>,
    /// Damage numbers and "Dodge!" floating up on screen (cosmetic: not saved or hashed).
    floaters: Vec<crate::ui::layout::Floater>,
}
/// The world's seeded generator (PCG): the next number in [0, 1], advancing the stream. Rules, the world and scripts all draw from one stream.
fn next_random(rng: &mut u64) -> f64 {
    let old_state = *rng;
    *rng = old_state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let shifted = ((old_state >> 18) ^ old_state) >> 27;
    // PCG keeps the low 32 bits; a plain conversion would fail above u32::MAX and give 0.
    let random_bits = u32::try_from(shifted & u64::from(u32::MAX)).unwrap_or_default();
    let rotation = u32::try_from(old_state >> 59).unwrap_or_default();
    f64::from(random_bits.rotate_right(rotation)) / f64::from(u32::MAX)
}

#[derive(Clone)]
struct Event {
    name: String,
    a: Option<u32>,
    b: Option<u32>,
    depth: usize,
}

impl World {
    /// HUD definitions in display order.
    #[must_use]
    pub fn huds(&self) -> &[crate::HudDef] {
        &self.def.huds
    }

    /// Create the initial world from a definition and RNG seed.
    #[must_use]
    pub fn new(def: &GameDef, seed: u64) -> Self {
        let mut world = Self {
            def: def.clone(),
            entities: Vec::new(),
            vars: def.vars.iter().map(|v| (v.name.clone(), v.value)).collect(),
            tick: 0,
            rng: seed,
            next_id: 0,
            status: Status::Playing,
            overlaps: BTreeSet::new(),
            timers: BTreeMap::new(),
            rule_fires: vec![0; def.rules.len()],
            hands_state: hands::HandsState::default(),
            motion_history: BTreeMap::new(),
            agents: BTreeMap::new(),
            nav: None,
            ground: None,
            tools: BTreeMap::new(),
            swing_latches: BTreeSet::new(),
            out_balls: BTreeSet::new(),
            script: None,
            script_failure: None,
            initial: None,
            seed,
            flow: flow::FlowState::default(),
            pending_restart: None,
            ui_labels: BTreeMap::new(),
            combat: combat::CombatState::default(),
            camera_axes: BTreeMap::new(),
            floaters: Vec::new(),
        };
        for spawn in def.spawns.iter().filter(|spawn| spawn.level.is_none()) {
            world.spawn_line(spawn);
        }
        world.start_flow();
        world.start_scripts(seed);
        if world.script.is_some() {
            world.initial = Some(world.snapshot());
        }
        world
    }

    /// All live entities in id order.
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Every kind this world's definition declares, whether or not an entity of it is alive.
    #[must_use]
    pub fn kinds(&self) -> &[crate::KindDef] {
        &self.def.kinds
    }

    /// The `terrain` declaration of the loaded game (which parts make the ground a ball rolls on), for hosts that build the ground field.
    #[must_use]
    pub fn terrain_declaration(&self) -> Option<&crate::TerrainDef> {
        self.def.terrain()
    }

    /// The `surface` declarations in id order (the first is surface 1), for hosts that mark their footprints on the ground field.
    #[must_use]
    pub fn surface_declarations(&self) -> &[crate::SurfaceDef] {
        self.def.surfaces()
    }

    /// The `nav` declaration of the loaded game (which parts are walkable and the agent's size), for hosts that build the walkable grid.
    #[must_use]
    pub fn nav_declaration(&self) -> Option<&crate::NavDef> {
        self.def.nav()
    }

    /// Where the viewer should start: the position of the first entity a `follow=viewer` input targets, as spawned (None without such an input).
    #[must_use]
    pub fn viewer_start(&self) -> Option<[f64; 3]> {
        self.entities
            .iter()
            .find(|entity| {
                self.def.inputs.iter().any(|declaration| {
                    declaration.follow
                        && declaration
                            .target
                            .as_deref()
                            .is_some_and(|selector| matches_selector(&self.def, selector, entity))
                })
            })
            .map(|entity| entity.position)
    }

    /// Find a kind declaration in this world's definition.
    #[must_use]
    pub fn kind(&self, name: &str) -> Option<&crate::KindDef> {
        self.def.kinds.iter().find(|kind| kind.name == name)
    }

    /// Game variables.
    #[must_use]
    pub fn vars(&self) -> &BTreeMap<String, f64> {
        &self.vars
    }

    /// Current terminal state of the simulation.
    #[must_use]
    pub fn status(&self) -> Status {
        self.status
    }

    /// Number of times each rule's action list has run, in definition order.
    #[must_use]
    pub fn rule_fires(&self) -> &[u64] {
        &self.rule_fires
    }

    /// Advance the simulation by one fixed 1/60 second step.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid input, exceeded budgets, or excessive event chains.
    pub fn step(&mut self, input: &Inputs) -> Result<Outcome, GameError> {
        for (name, value) in &input.axes {
            if !value.is_finite() {
                return Err(GameError::InvalidInput(name.clone()));
            }
        }
        if input
            .viewer
            .is_some_and(|position| position.iter().any(|value| !value.is_finite()))
        {
            return Err(GameError::InvalidInput("viewer position".into()));
        }
        if !hands::valid_positions(input) {
            return Err(GameError::InvalidInput("hand position".into()));
        }
        if input
            .hand_rotations
            .iter()
            .any(|hand| hand.rotation.iter().any(|v| !v.is_finite()))
        {
            return Err(GameError::InvalidInput("hand rotation".into()));
        }
        if self.entities.len() > MAX_ENTITIES {
            return Err(GameError::EntityBudget(MAX_ENTITIES));
        }
        // a game with flow states keeps listening after it ends (a win screen is left with a press); one without stops
        if self.status != Status::Playing && self.def.states.is_empty() {
            return Ok(Outcome {
                status: self.status,
                effects: Vec::new(),
                events_count: 0,
            });
        }
        let before = self.snapshot();
        let result = self.step_inner(input);
        if result.is_err() {
            self.restore_unchecked(&before);
        } else if self.script_take_reset()
            && let Some(initial) = self.initial.clone()
        {
            // `game.reset()` from a script: the game starts over exactly as it began (its `start` handlers run again on the next step).
            self.restore_unchecked(&initial);
        }
        result
    }

    fn step_inner(&mut self, input: &Inputs) -> Result<Outcome, GameError> {
        if self.is_frozen() {
            return self.step_frozen(input);
        }
        let starting_positions = self
            .entities
            .iter()
            .map(|entity| (entity.id, entity.position))
            .collect::<Vec<_>>();
        self.tick += 1;
        self.script_begin_step(input);
        let mut effects = Vec::new();
        let mut queue = Vec::new();
        if self.tick == 1 {
            queue.push(Event {
                name: "start".into(),
                a: None,
                b: None,
                depth: 0,
            });
            queue.extend(self.entities.iter().map(|entity| Event {
                name: "start".into(),
                a: Some(entity.id),
                b: None,
                depth: 0,
            }));
        }
        queue.push(Event {
            name: "tick".into(),
            a: None,
            b: None,
            depth: 0,
        });
        let expired: Vec<String> = self
            .timers
            .keys()
            .filter(|(tick, _)| *tick <= self.tick)
            .map(|(_, name)| name.clone())
            .collect();
        self.timers.retain(|(tick, _), _| *tick > self.tick);
        for name in expired {
            queue.push(Event {
                name: format!("timer:{name}"),
                a: None,
                b: None,
                depth: 0,
            });
        }
        self.ui_input(&input.pressed, &mut queue, &mut effects)?;
        self.tick_combat();
        self.update_targeting(input);
        self.weapon_step(input, &mut queue);
        self.ability_step(input, &mut queue, &mut effects)?;
        self.advance_positions(input, &mut queue);
        self.advance_brains(&mut queue);
        self.advance_projectiles(&mut queue);
        for name in &input.pressed {
            if self
                .def
                .inputs
                .iter()
                .any(|declaration| declaration.name == *name && declaration.key)
            {
                queue.push(Event {
                    name: format!("input:{name}"),
                    a: None,
                    b: None,
                    depth: 0,
                });
            }
        }
        for host in &input.host {
            queue.push(Event {
                name: format!("host:{}", host.name),
                a: host.a,
                b: host.b,
                depth: 0,
            });
        }
        let actions = self.process_events(&mut queue, &mut effects)?;
        self.freeze_stunned();
        self.update_headings_and_status(&starting_positions);
        self.controller_animation(&mut effects);
        self.finish_flow(&mut effects)?;
        Ok(Outcome {
            status: self.status,
            effects,
            events_count: actions,
        })
    }

    fn advance_positions(&mut self, input: &Inputs, queue: &mut Vec<Event>) {
        self.advance_agents(queue);
        self.integrate(input);
        self.integrate_controllers(input, queue);
        let previous_positions = self
            .entities
            .iter()
            .map(|entity| (entity.id, entity.position))
            .collect::<Vec<_>>();
        self.apply_viewer_follow(input.viewer);
        self.apply_hand_follow(input);
        self.update_hands(&input.hands, queue);
        let released = queue
            .iter()
            .filter(|event| event.name == "release")
            .filter_map(|event| event.b)
            .collect::<BTreeSet<_>>();
        self.swing_balls(input, queue);
        self.integrate_falling(&released);
        self.integrate_rolling(queue);
        self.collect_pockets(queue);
        self.resolve_viewer_solids(&previous_positions);
        self.collect_touches(queue);
    }

    fn spawn(
        &mut self,
        kind: &str,
        position: [f64; 3],
        mut props: BTreeMap<String, f64>,
        heading: f64,
    ) {
        self.give_health(kind, &mut props);
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.entities.push(Entity {
            id,
            kind: kind.into(),
            position,
            heading,
            orientation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            props,
        });
        self.entities.sort_by_key(|entity| entity.id);
        if self.def.agents.iter().any(|agent| agent.kind == kind) {
            self.agents.insert(id, agents::AgentState::default());
        }
        self.give_ammo(id);
    }

    fn random(&mut self) -> f64 {
        next_random(&mut self.rng)
    }

    pub(super) fn restore_unchecked(&mut self, snapshot: &Snapshot) {
        self.entities.clone_from(&snapshot.entities);
        self.vars.clone_from(&snapshot.vars);
        self.tick = snapshot.tick;
        self.rng = snapshot.rng;
        self.next_id = snapshot.next_id;
        self.status = snapshot.status;
        self.overlaps.clone_from(&snapshot.overlaps);
        self.timers.clone_from(&snapshot.timers);
        self.rule_fires.clone_from(&snapshot.rule_fires);
        self.hands_state.clone_from(&snapshot.hands_state);
        self.motion_history.clone_from(&snapshot.motion_history);
        self.agents = snapshot.agents.clone().unwrap_or_default();
        self.tools.clone_from(&snapshot.tools);
        self.swing_latches.clone_from(&snapshot.swing_latches);
        self.out_balls.clone_from(&snapshot.out_balls);
        self.flow.clone_from(&snapshot.flow);
        self.combat.clone_from(&snapshot.combat);
        self.floaters.clear();
        self.pending_restart = None;
        self.script_restore(snapshot.script_state.as_deref());
    }
}
