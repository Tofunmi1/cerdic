//! Line-based game definition parser.
use crate::{Diagnostic, expr::Expr};
use std::{collections::BTreeMap, fmt::Write as _};
pub(crate) mod agent;
pub(crate) mod controller;
mod format;
pub(crate) mod bindings;
pub(crate) mod combat;
pub(crate) mod flow;
pub(crate) mod helpers;
pub(crate) mod party;
mod hud;
mod reader;
pub(crate) mod rolling;
mod script;
use format::{input_text, kind_text, number_text, steps_text};
use helpers::{action_changes, event_raised, expr_names, selector_exists};
pub use script::{LIBRARIES, ScriptDef};
/// Numeric initial variable.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableDef {
    /// Variable name.
    pub name: String,
    /// Initial value.
    pub value: f64,
}
/// Entity kind declaration.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "kind capabilities are independent, composable text flags rather than exclusive states"
)]
pub struct KindDef {
    /// Kind name.
    pub name: String,
    /// Collision radius.
    pub size: f64,
    /// Movement speed.
    pub speed: f64,
    /// Tags.
    pub tags: Vec<String>,
    /// Scene part that draws this kind, when visible.
    pub part: Option<String>,
    /// Axis-aligned box extents that block followed entities, when present.
    pub solid: Option<[f64; 3]>,
    /// Whether entities of this kind can be grabbed by a hand (`grab=1`).
    pub grab: bool,
    /// Whether entities of this kind are affected by gravity.
    pub fall: bool,
    /// Whether this kind rolls on terrain.
    pub roll: bool,
    /// Whether this kind's swept tool head swings into balls.
    pub swing: bool,
    /// Head offset along the hand's local -Z axis, in metres.
    pub reach: f64,
    /// Upward launch angle in degrees.
    pub loft: f64,
    /// Head speed multiplier for ball launches.
    pub power: f64,
    /// Whether this kind captures slow grounded balls.
    pub pocket: bool,
    /// Fraction of vertical speed retained on bounce.
    pub bounce: f64,
    /// Initial numeric properties.
    pub props: BTreeMap<String, f64>,
}
/// Initial entity declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnDef {
    /// Kind name.
    pub kind: String,
    /// Initial position.
    pub position: [f64; 3],
    /// Initial facing direction as a yaw in radians about +Y.
    pub heading: f64,
    /// Number to spawn.
    pub count: usize,
    /// Scatter square width.
    pub spread: f64,
    /// The level this spawn belongs to (`level=NAME`); it appears when that level loads. `None` spawns at the start.
    pub level: Option<String>,
}
/// Declared input axis or button.
#[derive(Clone, Debug, PartialEq)]
pub struct InputDef {
    pub(crate) name: String,
    pub(crate) axes: Vec<String>,
    pub(crate) target: Option<String>,
    pub(crate) key: bool,
    /// Whether matching entities follow the viewer position (`follow=viewer`).
    pub follow: bool,
    /// Hand source for matching entities (`follow=left_hand` or `follow=right_hand`).
    pub(crate) hand_follow: Option<crate::HandSide>,
    /// The keys and gamepad controls that drive it, and the label a prompt uses.
    pub bindings: bindings::Bindings,
}
/// Compiled rule declaration.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Event selector.
    pub event: String,
    /// Condition text.
    pub when: Option<String>,
    /// Action statements.
    pub actions: Vec<String>,
    pub(crate) a: Option<String>,
    pub(crate) b: Option<String>,
    pub(crate) every: Option<u64>,
    /// The flow state the rule is limited to (`state=NAME`).
    pub(crate) state: Option<String>,
    pub(crate) expr: Option<Expr>,
    pub(crate) line: usize,
}
/// Win or loss condition.
#[derive(Clone, Debug)]
pub struct Goal {
    /// Whether this goal wins.
    pub win: bool,
    /// Condition text.
    pub when: String,
    pub(crate) expr: Expr,
}
/// HUD text bound to game variables.
#[derive(Clone, Debug, PartialEq)]
pub struct HudDef {
    /// Text with optional `{variable}` placeholders.
    pub text: String,
    /// Screen anchor: gaze, left, right, top, or bottom.
    pub anchor: String,
    /// Edge length of one pixel, in metres.
    pub size: f64,
}
/// Parsed game data.
#[derive(Clone, Debug)]
pub struct GameDef {
    /// Game name.
    pub name: String,
    /// Initial variables.
    pub vars: Vec<VariableDef>,
    /// Kinds.
    pub kinds: Vec<KindDef>,
    /// Initial entities.
    pub spawns: Vec<SpawnDef>,
    /// Declared player inputs.
    pub inputs: Vec<InputDef>,
    /// Event rules.
    pub rules: Vec<Rule>,
    /// Terminal goals.
    pub goals: Vec<Goal>,
    /// HUD lines in definition order.
    pub huds: Vec<HudDef>,
    /// Ground plane height.
    pub ground: f64,
    /// Autonomous behaviour declarations, in definition order.
    pub agents: Vec<agent::AgentDef>,
    /// Character controller declarations, in definition order.
    pub controllers: Vec<controller::ControllerDef>,
    /// Camera declarations, in definition order (the first starts active).
    pub cameras: Vec<crate::CameraDef>,
    /// Flow states, in definition order (the first is where the game starts).
    pub states: Vec<flow::StateDef>,
    /// Levels, in definition order (the first is loaded at the start).
    pub levels: Vec<flow::LevelDef>,
    /// The declared UI tree's elements, in definition order (ADR 0074).
    pub ui: Vec<crate::UiDef>,
    /// Hit points per kind.
    pub healths: Vec<combat::HealthDef>,
    /// Kinds that fly and hurt.
    pub projectiles: Vec<combat::ProjectileDef>,
    /// Weapons.
    pub weapons: Vec<combat::WeaponDef>,
    /// Kinds that move and fight on their own.
    pub brains: Vec<party::BrainDef>,
    /// How the leader's target is chosen, if the game has targeting.
    pub targeting: Option<party::TargetingDef>,
    /// Button-triggered abilities.
    pub abilities: Vec<party::AbilityDef>,
    pub(crate) navigation: Option<agent::NavDef>,
    pub(crate) surfaces: Vec<rolling::SurfaceDef>,
    pub(crate) terrain: Option<rolling::TerrainDef>,
    /// JavaScript behaviours, in definition order.
    pub scripts: Vec<ScriptDef>,
}
impl GameDef {
    /// Parse a complete line-based definition.
    ///
    /// # Errors
    ///
    /// Returns one or more source-located diagnostics when the text is invalid.
    pub fn parse(text: &str) -> Result<Self, Vec<Diagnostic>> {
        Self::parse_checked(text, true)
    }

    /// Parse a fragment of a game: every statement is read and checked on its own, but names that other statements declare (a state in
    /// `show=`, a kind in a `health` line, a weapon in a `brain`) are not looked up yet. `game.collect` reads each group this way so a large
    /// game can be gathered in parts; [`GameDef::validate`] makes those checks on the whole.
    ///
    /// # Errors
    ///
    /// Returns one or more source-located diagnostics when a statement is invalid.
    pub fn parse_partial(text: &str) -> Result<Self, Vec<Diagnostic>> {
        Self::parse_checked(text, false)
    }

    /// The checks that a statement's names exist among the other statements.
    fn cross_checks(&self) -> Vec<Diagnostic> {
        let mut found = flow::check(self);
        found.extend(crate::ui::tree::check(self));
        found.extend(combat::check(self));
        found.extend(party::check(self));
        found
    }

    fn parse_checked(text: &str, cross: bool) -> Result<Self, Vec<Diagnostic>> {
        let mut d = Self {
            name: "game".into(),
            agents: vec![],
            controllers: vec![],
            cameras: vec![],
            states: vec![],
            levels: vec![],
            ui: vec![],
            healths: vec![],
            projectiles: vec![],
            weapons: vec![],
            brains: vec![],
            targeting: None,
            abilities: vec![],
            navigation: None,
            surfaces: vec![],
            terrain: None,
            scripts: vec![],
            vars: vec![],
            kinds: vec![],
            spawns: vec![],
            inputs: vec![],
            rules: vec![],
            goals: vec![],
            huds: vec![],
            ground: 0.0,
        };
        let mut errors = reader::parse_lines(text, &mut d);
        if cross {
            errors.extend(d.cross_checks());
        }
        if d.kinds.len() > 64 {
            errors.push(Diagnostic::new(
                1,
                1,
                "more than 64 kinds",
                "remove unused kinds",
            ));
        }
        if d.rules.len() > 256 {
            errors.push(Diagnostic::new(
                1,
                1,
                "more than 256 rules",
                "remove unused rules",
            ));
        }
        if errors.is_empty() {
            Ok(d)
        } else {
            Err(errors)
        }
    }
    /// Print a canonical line-based definition accepted by [`GameDef::parse`].
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut lines = vec![format!("game name={}", self.name)];
        if !self.vars.is_empty() {
            lines.push(format!("var {}", {
                self.vars
                    .iter()
                    .map(|var| format!("{}={}", var.name, number_text(var.value)))
                    .collect::<Vec<_>>()
                    .join(" ")
            }));
        }
        lines.extend(self.states.iter().map(flow::StateDef::to_text));
        lines.extend(self.levels.iter().map(flow::LevelDef::to_text));
        lines.extend(self.kinds.iter().map(kind_text));
        lines.extend(self.healths.iter().map(combat::HealthDef::to_text));
        lines.extend(self.projectiles.iter().map(combat::ProjectileDef::to_text));
        lines.extend(self.weapons.iter().map(combat::WeaponDef::to_text));
        lines.extend(self.brains.iter().map(party::BrainDef::to_text));
        lines.extend(self.targeting.iter().map(party::TargetingDef::to_text));
        lines.extend(self.abilities.iter().map(party::AbilityDef::to_text));
        lines.extend(self.surfaces.iter().map(rolling::SurfaceDef::to_text));
        if let Some(terrain) = self.terrain() {
            lines.push(terrain.to_text());
        }
        lines.extend(self.agents.iter().map(agent::AgentDef::to_text));
        lines.extend(self.controllers.iter().map(controller::ControllerDef::to_text));
        lines.extend(self.cameras.iter().map(crate::CameraDef::to_text));
        if let Some(nav) = self.nav() {
            lines.push(nav.to_text());
        }
        if self.ground != 0.0 {
            lines.push(format!("ground y={}", number_text(self.ground)));
        }
        lines.extend(self.spawns.iter().map(|spawn| {
            let mut line = format!(
                "spawn {} at={},{},{}",
                spawn.kind,
                number_text(spawn.position[0]),
                number_text(spawn.position[1]),
                number_text(spawn.position[2])
            );
            if spawn.count != 1 {
                let _ = write!(line, " count={}", spawn.count);
            }
            if spawn.spread != 0.0 {
                let _ = write!(line, " spread={}", number_text(spawn.spread));
            }
            if spawn.heading != 0.0 {
                let _ = write!(line, " heading={}", number_text(spawn.heading.to_degrees()));
            }
            if let Some(level) = &spawn.level {
                let _ = write!(line, " level={level}");
            }
            line
        }));
        lines.extend(self.inputs.iter().map(input_text));
        lines.extend(self.ui.iter().map(crate::UiDef::to_text));
        lines.extend(self.rules.iter().map(|rule| {
            let mut line = format!("rule on={}", rule.event);
            if let Some(value) = &rule.when {
                let _ = write!(line, " when={value}");
            }
            if let Some(value) = &rule.a {
                let _ = write!(line, " a={value}");
            }
            if let Some(value) = &rule.b {
                let _ = write!(line, " b={value}");
            }
            if let Some(value) = rule.every {
                let _ = write!(line, " every={}", steps_text(value));
            }
            if let Some(value) = &rule.state {
                let _ = write!(line, " state={value}");
            }
            if !rule.actions.is_empty() {
                let _ = write!(line, " do={}", rule.actions.join(";"));
            }
            line
        }));
        lines.extend(self.scripts.iter().map(ScriptDef::to_text));
        lines.extend(self.goals.iter().map(|goal| {
            format!(
                "goal {} when={}",
                if goal.win { "win" } else { "lose" },
                goal.when
            )
        }));
        lines.extend(self.huds.iter().map(|hud| {
            format!(
                "hud text=\"{}\" anchor={} size={}",
                hud.text.replace('\\', "\\\\").replace('"', "\\\""),
                hud.anchor,
                number_text(hud.size)
            )
        }));
        format!("{}\n", lines.join("\n"))
    }
    /// Canonical, parseable model-friendly description.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut canonical = self.clone();
        canonical.vars.sort_by(|a, b| a.name.cmp(&b.name));
        canonical.to_text()
    }
    /// Static checks for rule references and obviously unreachable goals.
    #[must_use]
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut out = self.cross_checks();
        for r in &self.rules {
            if !event_raised(self, &r.event) {
                out.push(Diagnostic::new(
                    r.line,
                    1,
                    format!("event '{}' is never raised", r.event),
                    "add an input or action that raises this event",
                ));
            }
            for selector in [r.a.as_deref(), r.b.as_deref()].into_iter().flatten() {
                if !selector_exists(self, selector) {
                    out.push(Diagnostic::new(
                        r.line,
                        1,
                        format!("rule selector '{selector}' cannot match"),
                        "declare a kind or tag used by this selector",
                    ));
                }
            }
            for a in &r.actions {
                if let Some(k) = a.strip_prefix("spawn:").and_then(|s| s.split('@').next())
                    && !self.kinds.iter().any(|v| v.name == k)
                {
                    out.push(Diagnostic::new(
                        r.line,
                        1,
                        format!("unknown kind '{k}'"),
                        "declare this kind before using it",
                    ));
                }
            }
        }
        if self.spawns.iter().map(|spawn| spawn.count).sum::<usize>() > 2048 {
            out.push(Diagnostic::new(
                1,
                1,
                "initial entity count exceeds 2048",
                "reduce the number of spawned entities",
            ));
        }
        for goal in &self.goals {
            for name in expr_names(&goal.expr) {
                if self.vars.iter().any(|var| var.name == name)
                    && !self.rules.iter().any(|rule| {
                        rule.actions
                            .iter()
                            .any(|action| action_changes(action, &name))
                    })
                {
                    out.push(Diagnostic::new(
                        1,
                        1,
                        format!("goal variable '{name}' is never changed"),
                        "add a rule that changes this variable",
                    ));
                }
            }
        }
        out
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event
            && self.when == other.when
            && self.actions == other.actions
            && self.a == other.a
            && self.b == other.b
            && self.every == other.every
            && self.state == other.state
    }
}
impl PartialEq for Goal {
    fn eq(&self, other: &Self) -> bool {
        self.win == other.win && self.when == other.when
    }
}
impl PartialEq for GameDef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.vars == other.vars
            && self.kinds == other.kinds
            && self.spawns == other.spawns
            && self.inputs == other.inputs
            && self.states == other.states
            && self.levels == other.levels
            && self.ui == other.ui
            && self.healths == other.healths
            && self.projectiles == other.projectiles
            && self.weapons == other.weapons
            && self.brains == other.brains
            && self.targeting == other.targeting
            && self.abilities == other.abilities
            && self.rules == other.rules
            && self.goals == other.goals
            && self.huds == other.huds
            && self.agents == other.agents
            && self.navigation == other.navigation
            && self.surfaces == other.surfaces
            && self.terrain == other.terrain
    }
}
