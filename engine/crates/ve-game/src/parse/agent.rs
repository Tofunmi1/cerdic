//! Autonomous creature and walkable level declarations.
use super::{GameDef, helpers::required};
use std::collections::BTreeMap;
#[cfg(test)]
mod fuzz;

pub(super) fn error_column(raw: &str, message: &str) -> Option<usize> {
    if !raw.trim_start().starts_with("agent ") && !raw.trim_start().starts_with("nav ") {
        return None;
    }
    raw.split_whitespace().find_map(|token| {
        let key = token.split_once('=').map_or(token, |(key, _)| key);
        (message.contains(key) && key != "agent" && key != "nav")
            .then(|| {
                raw.find(token)
                    .map(|index| raw[..index].chars().count() + 1)
            })
            .flatten()
    })
}

/// Behaviour shared by every entity of a declared kind.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentDef {
    /// Declared entity kind.
    pub kind: String,
    /// Ordered horizontal patrol loop, in metres.
    pub patrol: Vec<[f64; 2]>,
    /// Patrol speed in metres per second.
    pub speed: f64,
    /// Chase speed in metres per second.
    pub chase_speed: f64,
    /// Horizontal notice distance in metres.
    pub sight: f64,
    /// Horizontal distance at which a chase ends.
    pub lose: f64,
}

/// Authored parts and build configuration for a host-provided navmesh.
#[derive(Clone, Debug, PartialEq)]
pub struct NavDef {
    /// Names of the floor, ramp and stair parts.
    pub parts: Vec<String>,
    /// Grid cell size in metres.
    pub cell: f64,
    /// Agent radius in metres.
    pub radius: f64,
    /// Agent height in metres.
    pub height: f64,
    /// Maximum step height in metres.
    pub step: f64,
    /// Maximum walkable slope in degrees.
    pub slope: f64,
}

impl GameDef {
    /// Walkable level declaration, when supplied by the author.
    #[must_use]
    pub fn nav(&self) -> Option<&NavDef> {
        self.navigation.as_ref()
    }
}

fn value(
    fields: &BTreeMap<String, String>,
    key: &str,
    default: f64,
    zero: bool,
) -> Result<f64, String> {
    let value = fields
        .get(key)
        .map_or(Ok(default), |text| text.parse::<f64>())
        .map_err(|_| format!("{key} must be a finite number"))?;
    if !value.is_finite() || value < 0.0 || (!zero && value == 0.0) {
        return Err(format!(
            "{key} must be finite and {}",
            if zero { "nonnegative" } else { "positive" }
        ));
    }
    Ok(value)
}

pub(super) fn parse_agent(
    def: &mut GameDef,
    kind: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let kind = kind.ok_or("agent needs a kind")?;
    if !def.kinds.iter().any(|entry| entry.name == kind) {
        return Err(format!("unknown agent kind '{kind}'"));
    }
    let patrol = required(fields, "patrol")?
        .split(';')
        .map(|point| {
            let coords = point
                .split(',')
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "patrol needs finite x,z points")?;
            if coords.len() != 2 || coords.iter().any(|v| !v.is_finite()) {
                return Err("patrol needs finite x,z points");
            }
            Ok([coords[0], coords[1]])
        })
        .collect::<Result<Vec<_>, _>>()?;
    if patrol.len() < 2 {
        return Err("patrol needs at least two x,z points".into());
    }
    required(fields, "speed")?;
    let speed = value(fields, "speed", 1.0, false)?;
    let sight = value(fields, "sight", 5.0, false)?;
    let agent = AgentDef {
        kind: kind.to_owned(),
        patrol,
        speed,
        sight,
        chase_speed: value(fields, "chase_speed", speed * 1.6, false)?,
        lose: value(fields, "lose", sight * 1.5, false)?,
    };
    if def.agents.iter().any(|entry| entry.kind == kind) {
        return Err(format!("duplicate agent kind '{kind}'"));
    }
    def.agents.push(agent);
    Ok(())
}

pub(super) fn parse_nav(
    def: &mut GameDef,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let parts: Vec<String> = required(fields, "parts")?
        .split(',')
        .map(str::to_owned)
        .collect();
    if parts.iter().any(String::is_empty) {
        return Err("nav parts needs nonempty part names".into());
    }
    let nav = NavDef {
        parts,
        cell: value(fields, "cell", 0.25, false)?,
        radius: value(fields, "radius", 0.3, true)?,
        height: value(fields, "height", 1.8, false)?,
        step: value(fields, "step", 0.4, true)?,
        slope: value(fields, "slope", 45.0, false)?,
    };
    if nav.slope >= 90.0 {
        return Err("slope must be between 0 and 90 degrees".into());
    }
    def.navigation = Some(nav);
    Ok(())
}

impl AgentDef {
    pub(crate) fn to_text(&self) -> String {
        let patrol = self
            .patrol
            .iter()
            .map(|p| format!("{},{}", p[0], p[1]))
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "agent {} patrol={patrol} speed={} chase_speed={} sight={} lose={}",
            self.kind, self.speed, self.chase_speed, self.sight, self.lose
        )
    }
}
impl NavDef {
    pub(super) fn to_text(&self) -> String {
        format!(
            "nav parts={} cell={} radius={} height={} step={} slope={}",
            self.parts.join(","),
            self.cell,
            self.radius,
            self.height,
            self.step,
            self.slope
        )
    }
}
