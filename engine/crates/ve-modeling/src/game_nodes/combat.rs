//! Health, projectile and weapon fragments (F2): each writes its game text line from the node's parameters.
use super::{GameFragment, Param, Section, item, number_or, text};
use crate::Fail;
use std::fmt::Write as _;
use ve_graph::Node;

/// `health KIND hp=N iframes=S knock=N [height=N]`.
pub(super) fn health(node: &Node) -> Result<GameFragment, Fail> {
    let kind = text(node, "kind").unwrap_or("");
    let mut line = format!("health {kind} hp={} iframes={} knock={}", number_or(node, "hp", 1.0), number_or(node, "iframes", 0.0), number_or(node, "knock", 0.0));
    let height = number_or(node, "height", -1.0);
    if height > 0.0 {
        let _ = write!(line, " height={height}");
    }
    Ok(item(node, Section::Health, line, Some(kind.to_owned())))
}

/// `projectile KIND speed=N life=S damage=N gravity=N pierce=N splash=N homing=N`.
pub(super) fn projectile(node: &Node) -> Result<GameFragment, Fail> {
    let kind = text(node, "kind").unwrap_or("");
    let line = format!(
        "projectile {kind} speed={} life={} damage={} gravity={} pierce={} splash={} homing={}",
        number_or(node, "speed", 20.0),
        number_or(node, "life", 2.0),
        number_or(node, "damage", 10.0),
        number_or(node, "gravity", 0.0),
        number_or(node, "pierce", 0.0),
        number_or(node, "splash", 0.0),
        number_or(node, "homing", 0.0)
    );
    Ok(item(node, Section::Projectile, line, Some(kind.to_owned())))
}

/// `brain KIND foes=... speed=... follow=... engage=... [leader=] [weapon=] [keep=]`.
pub(super) fn brain(node: &Node) -> Result<GameFragment, Fail> {
    let kind = text(node, "kind").unwrap_or("");
    let mut line = format!(
        "brain {kind} foes={} speed={} follow={} engage={}",
        text(node, "foes").unwrap_or(""),
        number_or(node, "speed", 3.0),
        number_or(node, "follow", 3.0),
        number_or(node, "engage", 15.0)
    );
    for key in ["leader", "weapon"] {
        if let Some(value) = text(node, key).filter(|value| !value.is_empty()) {
            let _ = write!(line, " {key}={value}");
        }
    }
    let keep = number_or(node, "keep", -1.0);
    if keep >= 0.0 {
        let _ = write!(line, " keep={keep}");
    }
    Ok(item(node, Section::Brain, line, Some(kind.to_owned())))
}

/// `targeting leader=... foes=... range=... cone=...`.
pub(super) fn targeting(node: &Node) -> Result<GameFragment, Fail> {
    let line = format!(
        "targeting leader={} foes={} range={} cone={}",
        text(node, "leader").unwrap_or(""),
        text(node, "foes").unwrap_or(""),
        number_or(node, "range", 25.0),
        number_or(node, "cone", 80.0)
    );
    Ok(item(node, Section::Targeting, line, None))
}

/// `ability NAME owner=... input=... cooldown=... [cost=...] [do=...]`.
pub(super) fn ability(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let mut line = format!(
        "ability {name} owner={} input={} cooldown={} cost={} cost_prop={} range={} needs_target={}",
        text(node, "owner").unwrap_or(""),
        text(node, "input").unwrap_or(""),
        number_or(node, "cooldown", 1.0),
        number_or(node, "cost", 0.0),
        text(node, "cost_prop").filter(|prop| !prop.is_empty()).unwrap_or("focus"),
        number_or(node, "range", 0.0),
        u8::from(matches!(node.params.get("needs_target"), Some(Param::Bool(true))))
    );
    if let Some(actions) = text(node, "do").map(|actions| actions.split_whitespace().collect::<String>()).filter(|actions| !actions.is_empty()) {
        let _ = write!(line, " do={actions}");
    }
    Ok(item(node, Section::Ability, line, Some(name.to_owned())))
}

/// `weapon NAME mode=... owner=... ...`: empty optional parameters are left out.
pub(super) fn weapon(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let mut line = format!(
        "weapon {name} mode={} owner={} input={} ammo={} reload={} cooldown={} spread={} aim={} muzzle={} range={} damage={}",
        text(node, "mode").unwrap_or("projectile"),
        text(node, "owner").unwrap_or(""),
        text(node, "input").filter(|input| !input.is_empty()).unwrap_or("none"),
        number_or(node, "ammo", 0.0),
        number_or(node, "reload", 1.0),
        number_or(node, "cooldown", 0.2),
        number_or(node, "spread", 0.0),
        text(node, "aim").filter(|aim| !aim.is_empty()).unwrap_or("heading"),
        number_or(node, "muzzle", 1.0),
        number_or(node, "range", 0.0),
        number_or(node, "damage", 10.0)
    );
    let _ = write!(line, " arc={}", number_or(node, "arc", 120.0));
    for (key, field) in [("shot", "shot"), ("reload_input", "reload_input"), ("foes", "foes")] {
        if let Some(value) = text(node, field).filter(|value| !value.is_empty()) {
            let _ = write!(line, " {key}={value}");
        }
    }
    if matches!(node.params.get("auto"), Some(Param::Bool(true))) {
        line.push_str(" auto=1");
    }
    Ok(item(node, Section::Weapon, line, Some(name.to_owned())))
}
