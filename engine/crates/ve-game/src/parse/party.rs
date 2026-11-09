//! Party combat declarations:
//!
//! - `brain KIND foes=SELECTOR speed=N [leader=SELECTOR] [follow=METRES] [engage=METRES] [weapon=NAME] [keep=METRES]`: entities of the kind
//!   move and fight on their own. With a foe within `engage` they close to the weapon's reach (`keep` for a ranged weapon) and use it; with
//!   none they follow the `leader` to within `follow`.
//! - `targeting leader=SELECTOR foes=SELECTOR [range=METRES] [cone=DEGREES]`: the leader's prop `target` names the foe it faces (sticky until
//!   it dies or leaves `range`); that foe has the prop `targeted` set to 1. The action `target:next` / `target:prev` cycles.
//! - `ability NAME owner=SELECTOR input=NAME cooldown=SECONDS [cost=N] [cost_prop=NAME] [range=METRES] [needs_target=1] [do=ACTIONS]`: pressing
//!   the input runs the actions with `a` the owner and `b` its target, then the ability cools down (the owner's prop `cd_NAME` counts the seconds
//!   left); a press that cannot work raises `refused:NAME`, a use raises `ability:NAME`.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::helpers::{number, selector_exists, validate_action, validate_selector};
use crate::{Diagnostic, GameDef};

/// A kind that moves and fights on its own.
#[derive(Clone, Debug, PartialEq)]
pub struct BrainDef {
    /// The kind.
    pub kind: String,
    /// Which entities it fights.
    pub foes: String,
    /// Whom it follows when there is nothing to fight.
    pub leader: Option<String>,
    /// Metres per second.
    pub speed: f64,
    /// How close it stays to its leader.
    pub follow: f64,
    /// How far away it notices a foe.
    pub engage: f64,
    /// The weapon it fights with.
    pub weapon: Option<String>,
    /// How far from a foe a ranged fighter stands (a melee fighter closes to its weapon's reach).
    pub keep: Option<f64>,
}

/// How the leader's target is chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetingDef {
    /// Whose target it is.
    pub leader: String,
    /// What can be targeted.
    pub foes: String,
    /// How far away a foe can be targeted.
    pub range: f64,
    /// Foes within this many degrees of where the leader faces are preferred.
    pub cone: f64,
}

/// A button-triggered action with a cost and a cooldown.
#[derive(Clone, Debug, PartialEq)]
pub struct AbilityDef {
    /// Unique name (`cd_NAME`, `ability:NAME` and `refused:NAME` use it).
    pub name: String,
    /// Who has it.
    pub owner: String,
    /// The key input that triggers it.
    pub input: String,
    /// Seconds before it can be used again.
    pub cooldown: f64,
    /// What it costs, taken from the owner's prop `cost_prop`.
    pub cost: f64,
    /// The prop that pays.
    pub cost_prop: String,
    /// The farthest target it works on (0 for any).
    pub range: f64,
    /// Whether it needs a target in range.
    pub needs_target: bool,
    /// What it does, as rule actions (`a` is the owner, `b` the target).
    pub actions: Vec<String>,
}

fn num(fields: &BTreeMap<String, String>, key: &str, default: f64) -> Result<f64, String> {
    match fields.get(key) {
        None => Ok(default),
        Some(text) => number(text).ok().filter(|v| v.is_finite() && *v >= 0.0).ok_or_else(|| format!("{key} must be a number that is not negative")),
    }
}

pub(super) fn parse_brain(def: &mut GameDef, kind: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let kind = kind.ok_or("brain needs a kind")?;
    let foes = fields.get("foes").ok_or("a brain needs foes= (a selector such as tag:enemy)")?;
    validate_selector(foes)?;
    let leader = fields.get("leader").cloned();
    if let Some(leader) = &leader {
        validate_selector(leader)?;
    }
    let speed = num(fields, "speed", 3.0)?;
    if speed <= 0.0 {
        return Err("speed must be more than 0".into());
    }
    let brain = BrainDef {
        kind: kind.to_owned(),
        foes: foes.clone(),
        leader,
        speed,
        follow: num(fields, "follow", 3.0)?,
        engage: num(fields, "engage", 15.0)?,
        weapon: fields.get("weapon").cloned(),
        keep: fields.get("keep").map(|_| num(fields, "keep", 6.0)).transpose()?,
    };
    def.brains.retain(|existing| existing.kind != kind);
    def.brains.push(brain);
    Ok(())
}

pub(super) fn parse_targeting(def: &mut GameDef, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let leader = fields.get("leader").ok_or("targeting needs leader= (a selector such as tag:hero)")?;
    let foes = fields.get("foes").ok_or("targeting needs foes= (a selector such as tag:enemy)")?;
    validate_selector(leader)?;
    validate_selector(foes)?;
    let cone = num(fields, "cone", 80.0)?;
    if cone > 360.0 {
        return Err("cone must be at most 360 degrees".into());
    }
    def.targeting = Some(TargetingDef { leader: leader.clone(), foes: foes.clone(), range: num(fields, "range", 25.0)?, cone });
    Ok(())
}

pub(super) fn parse_ability(def: &mut GameDef, name: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let name = name.ok_or("an ability needs a name")?;
    if def.abilities.iter().any(|ability| ability.name == name) {
        return Err(format!("duplicate ability '{name}'"));
    }
    let owner = fields.get("owner").ok_or("an ability needs owner= (a selector such as tag:hero)")?;
    validate_selector(owner)?;
    let input = fields.get("input").ok_or("an ability needs input= (a key input name)")?;
    let actions: Vec<String> = fields.get("do").map(|text| text.split(';').map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
    for action in &actions {
        validate_action(action)?;
    }
    def.abilities.push(AbilityDef {
        name: name.to_owned(),
        owner: owner.clone(),
        input: input.clone(),
        cooldown: num(fields, "cooldown", 0.0)?,
        cost: num(fields, "cost", 0.0)?,
        cost_prop: fields.get("cost_prop").cloned().unwrap_or_else(|| "focus".into()),
        range: num(fields, "range", 0.0)?,
        needs_target: matches!(fields.get("needs_target").map(String::as_str), Some("1" | "true")),
        actions,
    });
    Ok(())
}

impl BrainDef {
    pub(super) fn to_text(&self) -> String {
        let mut line = format!("brain {} foes={} speed={} follow={} engage={}", self.kind, self.foes, self.speed, self.follow, self.engage);
        if let Some(leader) = &self.leader {
            let _ = write!(line, " leader={leader}");
        }
        if let Some(weapon) = &self.weapon {
            let _ = write!(line, " weapon={weapon}");
        }
        if let Some(keep) = self.keep {
            let _ = write!(line, " keep={keep}");
        }
        line
    }
}

impl TargetingDef {
    pub(super) fn to_text(&self) -> String {
        format!("targeting leader={} foes={} range={} cone={}", self.leader, self.foes, self.range, self.cone)
    }
}

impl AbilityDef {
    pub(super) fn to_text(&self) -> String {
        let mut line = format!(
            "ability {} owner={} input={} cooldown={} cost={} cost_prop={} range={} needs_target={}",
            self.name, self.owner, self.input, self.cooldown, self.cost, self.cost_prop, self.range, u8::from(self.needs_target)
        );
        if !self.actions.is_empty() {
            let _ = write!(line, " do={}", self.actions.join(";"));
        }
        line
    }
}

/// References between the party declarations and the rest of the game.
pub(super) fn check(def: &GameDef) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut complain = |message: String, hint: &str| out.push(Diagnostic::new(1, 1, message, hint));
    for brain in &def.brains {
        if !def.kinds.iter().any(|kind| kind.name == brain.kind) {
            complain(format!("brain names the kind '{}', which is not declared", brain.kind), "declare the kind first");
        }
        if let Some(weapon) = &brain.weapon
            && !def.weapons.iter().any(|candidate| candidate.name == *weapon)
        {
            complain(format!("brain {} uses the weapon '{weapon}', which is not declared", brain.kind), "declare it with: weapon NAME ...");
        }
        for selector in [Some(&brain.foes), brain.leader.as_ref()].into_iter().flatten() {
            if !selector_exists(def, selector) {
                complain(format!("brain {} selector '{selector}' cannot match", brain.kind), "declare a kind or tag used by this selector");
            }
        }
    }
    if let Some(targeting) = &def.targeting {
        for selector in [&targeting.leader, &targeting.foes] {
            if !selector_exists(def, selector) {
                complain(format!("targeting selector '{selector}' cannot match"), "declare a kind or tag used by this selector");
            }
        }
    }
    for ability in &def.abilities {
        if !selector_exists(def, &ability.owner) {
            complain(format!("ability {} owner '{}' cannot match", ability.name, ability.owner), "declare a kind or tag used by this selector");
        }
        if !def.inputs.iter().any(|input| input.name == ability.input && input.key) {
            complain(format!("ability {} uses the input '{}', which is not declared with key=press", ability.name, ability.input), "declare: input NAME key=press");
        }
    }
    out
}
