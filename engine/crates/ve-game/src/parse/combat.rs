//! Ranged combat and health declarations (F2):
//!
//! - `health KIND hp=N [iframes=SECONDS] [knock=METRES_PER_SECOND] [height=METRES]`: entities of the kind start with `health` and
//!   `max_health` props; after a hurt they are untouchable for `iframes`; a hurt from a source pushes them away at `knock`; `height` is
//!   how tall the hit shape is (default twice the kind's size).
//! - `projectile KIND speed=N life=SECONDS damage=N [gravity=N] [pierce=N] [splash=METRES] [homing=DEGREES_PER_SECOND]`: entities of the
//!   kind are shots that fly, hurt what they meet and are used up.
//! - `weapon NAME mode=projectile|hitscan input=NAME owner=SELECTOR ...`: fires a shot (`shot=KIND`) or a ray (`range`, `damage`) when
//!   the input is pressed or the action `fire:NAME` runs; `ammo` (0 is endless), `reload` seconds, `cooldown` seconds, `spread` degrees,
//!   `aim=heading|camera|SELECTOR`, `muzzle` height, `auto=1` to fire while the input is held, `reload_input`.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::helpers::{number, selector_exists, validate_selector};
use crate::{Diagnostic, GameDef};

/// Hit points of a kind.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthDef {
    /// The kind.
    pub kind: String,
    /// Starting and maximum health.
    pub hp: f64,
    /// Seconds untouchable after a hurt.
    pub iframes: f64,
    /// Push away from the source of a hurt, metres per second.
    pub knock: f64,
    /// Height of the hit shape in metres (`None`: twice the kind's size).
    pub height: Option<f64>,
}

/// A kind that flies and hurts.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectileDef {
    /// The kind (and the name `shot=` uses).
    pub kind: String,
    /// Metres per second.
    pub speed: f64,
    /// Seconds before it expires.
    pub life: f64,
    /// Damage on a direct hit.
    pub damage: f64,
    /// Downward acceleration, metres per second squared.
    pub gravity: f64,
    /// Extra victims it passes through.
    pub pierce: u32,
    /// Radius of the blast on impact (0 for none).
    pub splash: f64,
    /// Degrees per second it may turn toward the nearest victim (0 for none).
    pub homing: f64,
}

/// Something that fires.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponDef {
    /// Unique name; `fire:NAME` and the owner's prop `ammo_NAME` use it.
    pub name: String,
    /// `projectile` or `hitscan`.
    pub mode: String,
    /// The projectile kind (mode `projectile`).
    pub shot: Option<String>,
    /// The key input that fires it (`none` for rule-fired weapons).
    pub input: String,
    /// The key input that reloads it.
    pub reload_input: Option<String>,
    /// Which entities carry it.
    pub owner: String,
    /// Rounds per magazine (0 is endless).
    pub ammo: u32,
    /// Seconds to reload.
    pub reload: f64,
    /// Seconds between shots.
    pub cooldown: f64,
    /// Scatter, degrees either side.
    pub spread: f64,
    /// `heading`, `camera` or a selector to aim at.
    pub aim: String,
    /// Height of the muzzle above the owner's feet.
    pub muzzle: f64,
    /// Reach of a hitscan ray, metres.
    pub range: f64,
    /// Damage of a hitscan ray.
    pub damage: f64,
    /// Whether holding the input keeps firing.
    pub auto: bool,
    /// Who it can hurt, as a selector (empty: any victim but the owner). A party's weapons name the enemy so they never hit each other.
    pub foes: String,
    /// Melee: the width of the swing in degrees, centred on the aim.
    pub arc: f64,
}

fn num(fields: &BTreeMap<String, String>, key: &str, default: f64) -> Result<f64, String> {
    match fields.get(key) {
        None => Ok(default),
        Some(text) => number(text).ok().filter(|v| v.is_finite() && *v >= 0.0).ok_or_else(|| format!("{key} must be a number that is not negative")),
    }
}

fn positive(fields: &BTreeMap<String, String>, key: &str, default: f64) -> Result<f64, String> {
    let value = num(fields, key, default)?;
    if value > 0.0 { Ok(value) } else { Err(format!("{key} must be more than 0")) }
}

fn whole(fields: &BTreeMap<String, String>, key: &str) -> Result<u32, String> {
    match fields.get(key) {
        None => Ok(0),
        Some(text) => text.parse().map_err(|_| format!("{key} must be a whole number")),
    }
}

pub(super) fn parse_health(def: &mut GameDef, kind: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let kind = kind.ok_or("health needs a kind")?;
    let health = HealthDef {
        kind: kind.to_owned(),
        hp: positive(fields, "hp", 1.0).map_err(|_| "hp must be more than 0".to_owned())?,
        iframes: num(fields, "iframes", 0.0)?,
        knock: num(fields, "knock", 0.0)?,
        height: fields.get("height").map(|_| positive(fields, "height", 1.0)).transpose()?,
    };
    // a later statement for the same kind replaces the earlier one
    def.healths.retain(|existing| existing.kind != kind);
    def.healths.push(health);
    Ok(())
}

pub(super) fn parse_projectile(def: &mut GameDef, kind: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let kind = kind.ok_or("projectile needs a kind")?;
    let shot = ProjectileDef {
        kind: kind.to_owned(),
        speed: positive(fields, "speed", 20.0)?,
        life: positive(fields, "life", 2.0)?,
        damage: num(fields, "damage", 10.0)?,
        gravity: fields.get("gravity").map_or(Ok(0.0), |g| number(g).ok().filter(|v| v.is_finite()).ok_or("gravity must be a number"))?,
        pierce: whole(fields, "pierce")?,
        splash: num(fields, "splash", 0.0)?,
        homing: num(fields, "homing", 0.0)?,
    };
    def.projectiles.retain(|existing| existing.kind != kind);
    def.projectiles.push(shot);
    Ok(())
}

pub(super) fn parse_weapon(def: &mut GameDef, name: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let name = name.ok_or("a weapon needs a name")?;
    if def.weapons.iter().any(|weapon| weapon.name == name) {
        return Err(format!("duplicate weapon '{name}'"));
    }
    let mode = fields.get("mode").map_or("projectile", String::as_str);
    if !matches!(mode, "projectile" | "hitscan" | "melee") {
        return Err("mode must be projectile, hitscan or melee".into());
    }
    let owner = fields.get("owner").ok_or("a weapon needs owner= (a selector such as tag:hero)")?;
    validate_selector(owner)?;
    let aim = fields.get("aim").map_or("heading", String::as_str);
    if !matches!(aim, "heading" | "camera") {
        validate_selector(aim).map_err(|_| "aim must be heading, camera or a selector such as tag:hero".to_owned())?;
    }
    let range = num(fields, "range", 0.0)?;
    if mode != "projectile" && range <= 0.0 {
        return Err(format!("a {mode} weapon needs range= (metres)"));
    }
    let arc = num(fields, "arc", 120.0)?;
    if !(1.0..=360.0).contains(&arc) {
        return Err("arc must be between 1 and 360 degrees".into());
    }
    let foes = fields.get("foes").cloned().unwrap_or_default();
    if !foes.is_empty() {
        validate_selector(&foes)?;
    }
    def.weapons.push(WeaponDef {
        name: name.to_owned(),
        mode: mode.to_owned(),
        shot: fields.get("shot").cloned(),
        input: fields.get("input").cloned().unwrap_or_else(|| "none".into()),
        reload_input: fields.get("reload_input").cloned(),
        owner: owner.clone(),
        ammo: whole(fields, "ammo")?,
        reload: num(fields, "reload", 1.0)?,
        cooldown: num(fields, "cooldown", 0.2)?,
        spread: num(fields, "spread", 0.0)?,
        aim: aim.to_owned(),
        muzzle: num(fields, "muzzle", 1.0)?,
        range,
        damage: num(fields, "damage", 10.0)?,
        auto: matches!(fields.get("auto").map(String::as_str), Some("1" | "true")),
        foes,
        arc,
    });
    Ok(())
}

impl HealthDef {
    pub(super) fn to_text(&self) -> String {
        let mut line = format!("health {} hp={} iframes={} knock={}", self.kind, self.hp, self.iframes, self.knock);
        if let Some(height) = self.height {
            let _ = write!(line, " height={height}");
        }
        line
    }
}

impl ProjectileDef {
    pub(super) fn to_text(&self) -> String {
        format!(
            "projectile {} speed={} life={} damage={} gravity={} pierce={} splash={} homing={}",
            self.kind, self.speed, self.life, self.damage, self.gravity, self.pierce, self.splash, self.homing
        )
    }
}

impl WeaponDef {
    pub(super) fn to_text(&self) -> String {
        let mut line = format!(
            "weapon {} mode={} input={} owner={} ammo={} reload={} cooldown={} spread={} aim={} muzzle={} range={} damage={} auto={}",
            self.name, self.mode, self.input, self.owner, self.ammo, self.reload, self.cooldown, self.spread, self.aim, self.muzzle, self.range, self.damage, u8::from(self.auto)
        );
        let _ = write!(line, " arc={}", self.arc);
        if !self.foes.is_empty() {
            let _ = write!(line, " foes={}", self.foes);
        }
        if let Some(shot) = &self.shot {
            let _ = write!(line, " shot={shot}");
        }
        if let Some(input) = &self.reload_input {
            let _ = write!(line, " reload_input={input}");
        }
        line
    }
}

/// References between the declarations, and the weapon names used by actions.
pub(super) fn check(def: &GameDef) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let kind_ok = |name: &str| def.kinds.iter().any(|kind| kind.name == name);
    let mut complain = |line: usize, message: String, hint: &str| out.push(Diagnostic::new(line, 1, message, hint));
    for health in &def.healths {
        if !kind_ok(&health.kind) {
            complain(1, format!("health names the kind '{}', which is not declared", health.kind), "declare the kind first");
        }
    }
    for shot in &def.projectiles {
        if !kind_ok(&shot.kind) {
            complain(1, format!("projectile names the kind '{}', which is not declared", shot.kind), "declare the kind first");
        }
    }
    for weapon in &def.weapons {
        if let Some(shot) = &weapon.shot
            && !def.projectiles.iter().any(|p| p.kind == *shot)
        {
            complain(1, format!("weapon {} fires '{shot}', which has no projectile statement", weapon.name), "add: projectile KIND speed=20 ...");
        }
        if weapon.mode == "projectile" && weapon.shot.is_none() {
            complain(1, format!("weapon {} is a projectile weapon and needs shot=KIND", weapon.name), "name the projectile kind");
        }
        if !selector_exists(def, &weapon.owner) {
            complain(1, format!("weapon {} owner '{}' cannot match", weapon.name, weapon.owner), "declare a kind or tag used by this selector");
        }
        for input in [weapon.input.as_str()].into_iter().chain(weapon.reload_input.as_deref()).filter(|name| *name != "none") {
            if !def.inputs.iter().any(|declaration| declaration.name == input && declaration.key) {
                complain(1, format!("weapon {} uses the input '{input}', which is not declared with key=press", weapon.name), "declare: input NAME key=press");
            }
        }
    }
    for rule in &def.rules {
        for action in &rule.actions {
            if let Some(body) = action.strip_prefix("fire:").or_else(|| action.strip_prefix("reload:")) {
                let name = body.split('@').next().unwrap_or("");
                if !def.weapons.iter().any(|weapon| weapon.name == name) {
                    complain(rule.line, format!("no weapon named '{name}' for {action}"), "declare it with: weapon NAME ...");
                }
            }
        }
    }
    out
}
