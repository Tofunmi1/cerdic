//! Creature and walkable level game fragments.
use super::{GameFragment, Param, Section, at_node, item, number, number_or, text};
use crate::Fail;
use std::fmt::Write as _;
use ve_graph::Node;

pub(super) fn agent(node: &Node) -> Result<GameFragment, Fail> {
    let Some(Param::List(points)) = node.params.get("patrol") else {
        return Err(at_node(
            "game.agent",
            node,
            "patrol needs at least two x,z points",
        ));
    };
    let patrol = points
        .iter()
        .map(|p| {
            let Param::List(coords) = p else {
                return Err(at_node("game.agent", node, "patrol needs x,z pairs"));
            };
            if coords.len() != 2 {
                return Err(at_node("game.agent", node, "patrol needs x,z pairs"));
            }
            let x = coords[0]
                .as_f64()
                .ok_or_else(|| at_node("game.agent", node, "patrol needs numeric x"))?;
            let z = coords[1]
                .as_f64()
                .ok_or_else(|| at_node("game.agent", node, "patrol needs numeric z"))?;
            Ok(format!("{x},{z}"))
        })
        .collect::<Result<Vec<_>, Fail>>()?
        .join(";");
    let speed = number(node, "speed")?;
    let sight = number_or(node, "sight", 5.0);
    let kind = text(node, "kind").unwrap_or("");
    let automatic = |key, default| match number_or(node, key, 0.0) {
        0.0 => default,
        value => value,
    };
    Ok(item(
        node,
        Section::Agent,
        format!(
            "agent {kind} patrol={patrol} speed={speed} chase_speed={} sight={sight} lose={}",
            automatic("chase_speed", speed * 1.6),
            automatic("lose", sight * 1.5),
        ),
        Some(kind.to_owned()),
    ))
}

/// `controller KIND ...`: only the parameters that differ from the parser's own defaults are written, so the text stays short and a 0 that
/// means "automatic" is left out.
pub(super) fn controller(node: &Node) -> Result<GameFragment, Fail> {
    let kind = text(node, "kind").unwrap_or("");
    let mut line = format!("controller {kind} mode={}", text(node, "mode").unwrap_or("walk"));
    for key in ["speed", "accel", "decel"] {
        let value = number_or(node, key, 0.0);
        if value > 0.0 {
            let _ = write!(line, " {key}={value}");
        }
    }
    for (key, default) in [("air", 0.6), ("jump", 0.0), ("gravity", 20.0), ("coyote", 0.1), ("buffer", 0.1), ("cut", 0.5), ("turn", 0.0), ("step", 0.3)] {
        let value = number_or(node, key, default);
        if (value - default).abs() > f64::EPSILON {
            let _ = write!(line, " {key}={value}");
        }
    }
    let extra = number_or(node, "extra_jumps", 0.0);
    if extra > 0.0 {
        let _ = write!(line, " extra_jumps={extra}");
    }
    let fall = number_or(node, "fall_limit", -10_000.0);
    if fall > -10_000.0 {
        let _ = write!(line, " fall_limit={fall}");
    }
    if text(node, "relative") == Some("camera") {
        line.push_str(" relative=camera");
    }
    if let Some(lock @ ("x" | "z")) = text(node, "lock") {
        let _ = write!(line, " lock={lock}");
    }
    for (key, default) in [("move", "move"), ("jump_input", "jump")] {
        let value = text(node, key).unwrap_or(default);
        if value != default {
            let _ = write!(line, " {key}={value}");
        }
    }
    Ok(item(node, Section::Controller, line, Some(kind.to_owned())))
}

/// `camera NAME ...`: parameters at their automatic value are left out so the parser applies the mode's own defaults.
pub(super) fn camera(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("main");
    let mode = text(node, "mode").unwrap_or("third");
    let mut line = format!("camera {name} mode={mode} target={}", text(node, "target").unwrap_or("tag:hero"));
    if number_or(node, "distance", 0.0) > 0.0 {
        let _ = write!(line, " distance={}", number_or(node, "distance", 0.0));
    }
    if number_or(node, "height", -1.0) >= 0.0 {
        let _ = write!(line, " height={}", number_or(node, "height", -1.0));
    }
    if number_or(node, "smoothing", -1.0) >= 0.0 {
        let _ = write!(line, " smoothing={}", number_or(node, "smoothing", -1.0));
    }
    for (key, auto) in [("pitch", 999.0), ("yaw", 999.0)] {
        let value = number_or(node, key, auto);
        if (value - auto).abs() > f64::EPSILON {
            let _ = write!(line, " {key}={value}");
        }
    }
    for (key, default) in [("shoulder", 0.0), ("fov", 60.0), ("deadzone", 0.0), ("min_pitch", -60.0), ("max_pitch", 75.0)] {
        let value = number_or(node, key, default);
        if (value - default).abs() > f64::EPSILON {
            let _ = write!(line, " {key}={value}");
        }
    }
    if matches!(node.params.get("collide"), Some(Param::Bool(false))) {
        line.push_str(" collide=0");
    }
    for key in ["at", "bounds"] {
        if let Some(value) = text(node, key).filter(|value| !value.is_empty()) {
            let _ = write!(line, " {key}={}", value.replace(' ', ""));
        }
    }
    Ok(item(node, Section::Camera, line, Some(name.to_owned())))
}

/// `ui NAME type=... [key=value ...]`: only the parameters the node set are written; a number at -1 means "the default".
pub(super) fn ui(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let mut line = format!("ui {name} type={}", text(node, "type").unwrap_or(""));
    for key in [
        "parent", "show", "hide", "when", "icon", "key", "text", "do", "tip", "anchor", "at", "size", "value", "max_var", "label", "dir", "align", "cell", "follow", "tags", "kinds", "fill", "border", "color", "font",
        "plate",
    ] {
        if let Some(value) = text(node, key).map(str::trim).filter(|value| !value.is_empty()) {
            let value = if matches!(key, "show" | "hide" | "tags" | "kinds" | "at" | "size" | "cell") { value.replace(' ', "") } else { value.to_owned() };
            // the tree calls the variable form of a bar's maximum `max`
            let key = if key == "max_var" { "max" } else { key };
            // a value with a space, or a `#` (which would start a comment), is quoted
            if value.chars().any(|c| c.is_whitespace() || c == '#') {
                let _ = write!(line, " {key}=\"{}\"", value.replace('"', "'"));
            } else {
                let _ = write!(line, " {key}={value}");
            }
        }
    }
    for key in ["gap", "pad", "cols", "radius", "max"] {
        let value = number_or(node, key, -1.0);
        // a number given for `max` is skipped when the variable form is also given
        if value >= 0.0 && !(key == "max" && line.contains(" max=")) {
            let _ = write!(line, " {key}={value}");
        }
    }
    // the letter height is `size` on a text and `text_size` on a button
    let letter = number_or(node, "letter", -1.0);
    if letter > 0.0 {
        let key = if text(node, "type") == Some("button") { "text_size" } else { "size" };
        let _ = write!(line, " {key}={letter}");
    }
    Ok(item(node, Section::Ui, line, Some(name.to_owned())))
}

/// `state NAME [frozen=1]`.
pub(super) fn state(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let frozen = matches!(node.params.get("frozen"), Some(Param::Bool(true)));
    let line = if frozen { format!("state {name} frozen=1") } else { format!("state {name}") };
    Ok(item(node, Section::State, line, Some(name.to_owned())))
}

/// `level NAME [start=x,y,z]`: the start is written only when the node asks to place heroes.
pub(super) fn level(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let mut line = format!("level {name}");
    if matches!(node.params.get("place_heroes"), Some(Param::Bool(true)))
        && let Some(Param::List(at)) = node.params.get("start")
    {
        let v: Vec<f64> = at.iter().filter_map(Param::as_f64).collect();
        if v.len() == 3 {
            let _ = write!(line, " start={},{},{}", v[0], v[1], v[2]);
        }
    }
    Ok(item(node, Section::Level, line, Some(name.to_owned())))
}

pub(super) fn nav(node: &Node) -> GameFragment {
    let parts = match node.params.get("parts") {
        Some(Param::List(values)) => values
            .iter()
            .filter_map(|v| match v {
                Param::Str(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(","),
        _ => String::new(),
    };
    item(
        node,
        Section::Nav,
        format!(
            "nav parts={parts} cell={} radius={} height={} step={} slope={}",
            number_or(node, "cell", 0.25),
            number_or(node, "radius", 0.3),
            number_or(node, "height", 1.8),
            number_or(node, "step", 0.4),
            number_or(node, "slope", 45.0),
        ),
        None,
    )
}
