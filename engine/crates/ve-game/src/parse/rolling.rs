//! Rolling surface and terrain declarations and validated kind fields.
use super::{
    GameDef,
    helpers::{getnum, required},
};
use std::collections::BTreeMap;
#[cfg(test)]
mod fuzz;

/// Ground material properties, with identifiers assigned from one in declaration order.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceDef {
    /// Surface name.
    pub name: String,
    /// Scene parts carrying this surface.
    pub parts: Vec<String>,
    /// Constant rolling deceleration in metres per second squared.
    pub decel: f64,
    /// Multiplier for the ball's bounce restitution.
    pub bounce: f64,
}

/// Scene parts from which the host builds the ground grid.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainDef {
    /// Ground-producing scene parts.
    pub parts: Vec<String>,
    /// Grid sample spacing in metres.
    pub cell: f64,
}

impl GameDef {
    /// Ground materials in surface identifier order (identifier one is the first entry).
    #[must_use]
    pub fn surfaces(&self) -> &[SurfaceDef] {
        &self.surfaces
    }

    /// Host terrain sampling declaration, when present.
    #[must_use]
    pub fn terrain(&self) -> Option<&TerrainDef> {
        self.terrain.as_ref()
    }
}

impl SurfaceDef {
    pub(super) fn to_text(&self) -> String {
        format!(
            "surface {} parts={} decel={} bounce={}",
            self.name,
            self.parts.join(","),
            self.decel,
            self.bounce
        )
    }
}
impl TerrainDef {
    pub(super) fn to_text(&self) -> String {
        format!("terrain parts={} cell={}", self.parts.join(","), self.cell)
    }
}

pub(super) fn parse_surface(
    def: &mut GameDef,
    name: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    if def.surfaces.len() >= usize::from(u8::MAX) {
        return Err("surface ids exceed 255".into());
    }
    let surface = SurfaceDef {
        name: name.ok_or("surface needs a name")?.into(),
        parts: parts(fields)?,
        decel: positive(fields, "decel", 1.0, f64::MAX)?,
        bounce: bounded(fields, "bounce", 1.0, 0.0, 1.0)?,
    };
    def.surfaces.push(surface);
    Ok(())
}

pub(super) fn parse_terrain(
    def: &mut GameDef,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    def.terrain = Some(TerrainDef {
        parts: parts(fields)?,
        cell: positive(fields, "cell", 0.25, f64::MAX)?,
    });
    Ok(())
}

fn parts(fields: &BTreeMap<String, String>) -> Result<Vec<String>, String> {
    let parts: Vec<_> = required(fields, "parts")?
        .split(',')
        .map(str::to_owned)
        .collect();
    if parts.iter().any(String::is_empty) {
        return Err("parts needs nonempty part names".into());
    }
    Ok(parts)
}

pub(super) fn flag(fields: &BTreeMap<String, String>, key: &str) -> Result<bool, String> {
    match fields.get(key).map_or("0", String::as_str) {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("invalid {key}: use 0 or 1")),
    }
}

pub(super) fn bounded(
    fields: &BTreeMap<String, String>,
    key: &str,
    default: f64,
    min: f64,
    max: f64,
) -> Result<f64, String> {
    let value = getnum(fields, key, default).map_err(|_| format!("invalid {key} number"))?;
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!("{key} must be finite and from {min} to {max}"));
    }
    Ok(value)
}

pub(super) fn positive(
    fields: &BTreeMap<String, String>,
    key: &str,
    default: f64,
    max: f64,
) -> Result<f64, String> {
    let value = bounded(fields, key, default, 0.0, max)?;
    if value <= 0.0 {
        return Err(format!("{key} must be positive"));
    }
    Ok(value)
}

pub(super) fn error_column(raw: &str, message: &str) -> Option<usize> {
    [
        "roll", "swing", "pocket", "reach", "loft", "power", "decel", "bounce", "parts", "cell",
    ]
    .into_iter()
    .find(|key| message.contains(key))
    .and_then(|key| raw.find(&format!("{key}=")))
    .map(|i| i + 1)
}

pub(super) fn suggestion(raw: &str, message: &str) -> Option<String> {
    if raw.trim_start().starts_with("surface ") || raw.trim_start().starts_with("terrain ") {
        Some("supply nonempty parts=a,b; cell and decel must be finite and positive, bounce from 0 to 1; at most 255 surfaces".into())
    } else if raw.trim_start().starts_with("kind ") && error_column(raw, message).is_some() {
        Some("use roll, swing and pocket as 0 or 1; finite reach >= 0, loft from 0 to 80 degrees, power > 0 and <= 5".into())
    } else {
        None
    }
}
