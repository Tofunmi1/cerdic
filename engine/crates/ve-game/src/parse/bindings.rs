//! Device bindings of an `input` line: `keys=` (keyboard `KeyboardEvent.code` names, or `Mouse0` to `Mouse4`), `pad=` (gamepad controls) and
//! `label=` (what a prompt calls the action). The game only records and validates them and exports the table with
//! [`GameDef::bindings_json`](crate::GameDef::bindings_json); the page owns the devices.
//!
//! An axis input names its keys as a positive and a negative one per axis, in axis order (`axis=x,z keys=KeyD,KeyA,KeyW,KeyS` is +x, -x,
//! +z, -z). Gamepad controls: `button<N>` (a press or a held key), `stick:left` and `stick:right` (an axis pair), `axis<N>+` and `axis<N>-`
//! (one direction of one axis, for a one-axis input, positive then negative).
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::{GameDef, InputDef};

/// The devices that drive one input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings {
    /// Keyboard and mouse-button names.
    pub keys: Vec<String>,
    /// Gamepad controls.
    pub pad: Vec<String>,
    /// What a prompt calls the action (empty means the input's name).
    pub label: String,
}

fn list(fields: &BTreeMap<String, String>, key: &str) -> Result<Vec<String>, String> {
    let Some(text) = fields.get(key) else {
        return Ok(Vec::new());
    };
    let items: Vec<String> = text.split(',').map(str::to_owned).collect();
    if items.iter().any(|item| item.is_empty() || !item.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '+' || c == '-')) {
        return Err(format!("{key} needs names separated by commas (letters and digits, no spaces)"));
    }
    Ok(items)
}

/// Whether a gamepad control fits an input with `axes` axes: a button drives a press (no axes), a stick an axis pair, and one direction of
/// one gamepad axis a one-axis input.
fn pad_ok(control: &str, axes: usize) -> Result<(), String> {
    let number = |rest: &str| !rest.is_empty() && rest.len() <= 2 && rest.chars().all(|c| c.is_ascii_digit());
    let good = match control {
        "stick:left" | "stick:right" => axes == 2,
        _ if control.starts_with("button") => axes == 0 && number(&control["button".len()..]),
        _ => {
            let direction = control.ends_with('+') || control.ends_with('-');
            let digits = control.strip_prefix("axis").map(|rest| &rest[..rest.len().saturating_sub(1)]);
            axes == 1 && direction && digits.is_some_and(number)
        }
    };
    if good {
        Ok(())
    } else {
        Err(format!("pad control '{control}' does not fit this input (button<N> for a press, stick:left or stick:right for two axes, axis<N>+ or axis<N>- for one axis)"))
    }
}

/// Reads and checks `keys=`, `pad=` and `label=` for an input with `axes` axes.
pub(super) fn parse_bindings(axes: usize, fields: &BTreeMap<String, String>) -> Result<Bindings, String> {
    let keys = list(fields, "keys")?;
    if fields.contains_key("keys") && keys.is_empty() {
        return Err("keys needs at least one key".into());
    }
    if axes > 0 && !keys.is_empty() && keys.len() != axes * 2 {
        return Err(format!("keys needs {} names for {axes} axis(es): a positive and a negative key per axis", axes * 2));
    }
    let pad = list(fields, "pad")?;
    for control in &pad {
        pad_ok(control, axes)?;
    }
    Ok(Bindings { keys, pad, label: fields.get("label").cloned().unwrap_or_default() })
}

/// The bindings as the trailing text of an `input` line.
pub(super) fn write_bindings(bindings: &Bindings) -> String {
    let mut text = String::new();
    if !bindings.keys.is_empty() {
        let _ = write!(text, " keys={}", bindings.keys.join(","));
    }
    if !bindings.pad.is_empty() {
        let _ = write!(text, " pad={}", bindings.pad.join(","));
    }
    if !bindings.label.is_empty() {
        let _ = write!(text, " label={}", bindings.label.replace(' ', "_"));
    }
    text
}

impl GameDef {
    /// The table of declared inputs and their device bindings as JSON, for the page:
    /// `[{"name", "kind": "axis"|"press"|"other", "axes", "keys", "pad", "label"}]`.
    #[must_use]
    pub fn bindings_json(&self) -> String {
        let rows: Vec<serde_json::Value> = self.inputs.iter().map(row).collect();
        serde_json::Value::Array(rows).to_string()
    }
}

fn row(input: &InputDef) -> serde_json::Value {
    let kind = if !input.axes.is_empty() {
        "axis"
    } else if input.key {
        "press"
    } else {
        "other"
    };
    serde_json::json!({
        "name": input.name,
        "kind": kind,
        "axes": input.axes,
        "keys": input.bindings.keys,
        "pad": input.bindings.pad,
        "label": if input.bindings.label.is_empty() { input.name.clone() } else { input.bindings.label.replace('_', " ") },
    })
}
