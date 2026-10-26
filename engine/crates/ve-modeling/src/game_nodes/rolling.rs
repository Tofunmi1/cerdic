//! Terrain and surface fragments, kept in collection order for stable surface ids.
use super::{GameFragment, Param, Section, at_node, item, number_or, text};
use crate::Fail;
use ve_graph::Node;

pub(super) fn kind_fields(node: &Node, mut line: String) -> Result<String, Fail> {
    for key in ["roll", "swing", "pocket"] {
        if matches!(node.params.get(key), Some(Param::Bool(true))) {
            line.push_str(&format!(" {key}=1"));
        }
    }
    for (key, default, max) in [
        ("reach", 0.0_f64, f64::MAX),
        ("loft", 0.0, 80.0),
        ("power", 1.0, 5.0),
    ] {
        let Some(raw) = node.params.get(key) else {
            continue;
        };
        let value = raw
            .as_f64()
            .ok_or_else(|| at_node("game.kind", node, &format!("{key} must be a number")))?;
        if !value.is_finite() || !(0.0..=max).contains(&value) || (key == "power" && value <= 0.0) {
            return Err(at_node(
                "game.kind",
                node,
                &format!(
                    "invalid {key}: reach >= 0, loft from 0 to 80, power > 0 and <= 5; all must be finite"
                ),
            ));
        }
        // Graph nodes contain schema defaults too; omit those from the text just as describe() does.
        if value.to_bits() != default.to_bits() {
            line.push_str(&format!(" {key}={value}"));
        }
    }
    Ok(line)
}

fn parts<'a>(node: &'a Node, code: &str) -> Result<Vec<&'a str>, Fail> {
    let Some(Param::List(values)) = node.params.get("parts") else {
        return Err(at_node(code, node, "parts must name authored parts"));
    };
    let names = values
        .iter()
        .map(|value| match value {
            Param::Str(name) if !name.is_empty() => Ok(name.as_str()),
            _ => Err(at_node(code, node, "parts needs nonempty part names")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if names.is_empty() {
        return Err(at_node(
            code,
            node,
            "parts must name at least one authored part",
        ));
    }
    Ok(names)
}

#[cfg(test)]
mod tests;

pub(super) fn terrain(node: &Node) -> Result<GameFragment, Fail> {
    let names = parts(node, "game.terrain")?;
    let cell = number_or(node, "cell", 0.25);
    if !cell.is_finite() || cell <= 0.0 {
        return Err(at_node(
            "game.terrain",
            node,
            "cell must be finite and greater than zero",
        ));
    }
    Ok(item(
        node,
        Section::Terrain,
        format!("terrain parts={} cell={cell}", names.join(",")),
        None,
    ))
}

pub(super) fn surface(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let names = parts(node, "game.surface")?;
    let decel = number_or(node, "decel", 0.0);
    let bounce = number_or(node, "bounce", 0.5);
    if !decel.is_finite() || decel <= 0.0 {
        return Err(at_node(
            "game.surface",
            node,
            "decel must be finite and greater than zero",
        ));
    }
    if !(0.0..=1.0).contains(&bounce) {
        return Err(at_node(
            "game.surface",
            node,
            "bounce must be from zero to one",
        ));
    }
    Ok(item(
        node,
        Section::Surface,
        format!(
            "surface {name} parts={} decel={decel} bounce={bounce}",
            names.join(",")
        ),
        Some(name.to_owned()),
    ))
}
