//! HUD declaration parser.
use super::{
    GameDef, HudDef,
    helpers::{getnum, required},
};
use std::collections::BTreeMap;

pub(super) fn parse_hud(
    definition: &mut GameDef,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    if definition.huds.len() >= 8 {
        return Err("at most 8 hud statements are allowed".into());
    }
    let text = required(fields, "text")?.to_owned();
    if text.chars().count() > 64 {
        return Err("hud text exceeds 64 characters".into());
    }
    let anchor = fields.get("anchor").map_or("gaze", String::as_str);
    if !matches!(anchor, "gaze" | "left" | "right" | "top" | "bottom") {
        return Err(format!("unknown hud anchor '{anchor}'"));
    }
    for part in text.split('{').skip(1) {
        if let Some((name, _)) = part.split_once('}')
            && !definition.vars.iter().any(|var| var.name == name)
        {
            return Err(format!("unknown variable '{name}' in hud text"));
        }
    }
    let size = getnum(fields, "size", 0.005)?;
    if !(0.0..=1.0).contains(&size) {
        return Err("hud size must be from 0.0 to 1.0 metres".into());
    }
    definition.huds.push(HudDef {
        text,
        anchor: anchor.to_owned(),
        size,
    });
    Ok(())
}
