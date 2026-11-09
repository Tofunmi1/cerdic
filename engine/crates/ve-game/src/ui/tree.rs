//! The declared UI tree (ADR 0074): `ui ID type=... [parent=ID] [show=STATE,...] [hide=STATE,...] key=value...`.
//!
//! Types: `text` (`text=`, with `{var}` and `{key:ACTION}` bindings), `bar` (`value=` a variable or number, `max=`), `panel`, `button`
//! (`text=`, optional `do=` actions, optional `tip=`), `stack` (`dir=down|right`, `gap`, `pad`, `align=start|center|end`), `grid`
//! (`cols`, `cell=w,h`, `gap`, `pad`) and `minimap` (`radius` metres, `follow=` selector, `tags=` / `kinds=` lists of what to show). The other
//! keys (`anchor`, `at`, `size`, `fill`, `border`, `color`, `font`, ...) are the visual keys of `ui.rs` and pass through to it.
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::parse::helpers::{nonempty, number, validate_action};
use crate::{Diagnostic, GameDef};

/// The types a `ui` statement may declare.
pub(crate) const TYPES: [&str; 10] = ["text", "bar", "panel", "button", "stack", "grid", "minimap", "slot", "portrait", "floaters"];

/// The icons a `slot` can draw (strokes, see `ui.rs`).
pub(crate) const ICONS: [&str; 8] = ["sword", "shield", "bolt", "heal", "dash", "star", "flame", "skull"];

/// One declared element.
#[derive(Clone, Debug, PartialEq)]
pub struct UiDef {
    /// Unique name; a button raises `ui:ID` and `parent=` refers to it.
    pub id: String,
    /// One of text, bar, panel, button, stack, grid, minimap.
    pub kind: String,
    /// Every other key as written.
    pub fields: BTreeMap<String, String>,
    pub(crate) line: usize,
}

impl UiDef {
    /// A field's text, or `""`.
    pub(crate) fn field(&self, key: &str) -> &str {
        self.fields.get(key).map_or("", String::as_str)
    }

    /// The states named by a list field (`show`, `hide`).
    pub(crate) fn list(&self, key: &str) -> Vec<&str> {
        self.field(key).split(',').filter(|name| !name.is_empty()).collect()
    }

    /// The actions a button runs when accepted.
    pub(crate) fn actions(&self) -> Vec<String> {
        self.field("do").split(';').map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect()
    }

    pub(crate) fn to_text(&self) -> String {
        let mut line = format!("ui {} type={}", self.id, self.kind);
        for (key, value) in &self.fields {
            let _ = if value.chars().any(|c| c.is_whitespace() || c == '#') { write!(line, " {key}=\"{value}\"") } else { write!(line, " {key}={value}") };
        }
        line
    }
}

fn pair(text: &str) -> bool {
    let parts: Vec<&str> = text.split(',').collect();
    parts.len() == 2 && parts.iter().all(|p| number(p).is_ok())
}

pub(crate) fn parse_ui(def: &mut GameDef, id: Option<&str>, fields: &BTreeMap<String, String>, line: usize) -> Result<(), String> {
    let id = id.ok_or("a ui element needs an id")?;
    if def.ui.iter().any(|item| item.id == id) {
        return Err(format!("duplicate ui id '{id}'"));
    }
    let kind = fields.get("type").ok_or("a ui element needs type= (text, bar, panel, button, stack, grid or minimap)")?;
    if !TYPES.contains(&kind.as_str()) {
        return Err(format!("unknown ui type '{kind}' (use {})", TYPES.join(", ")));
    }
    let mut rest = fields.clone();
    rest.remove("type");
    for key in ["size", "at", "cell"] {
        let Some(value) = rest.get(key) else { continue };
        // the size of a text is its letter height, one number; everywhere else a size is width,height
        if key == "size" && kind == "text" {
            if !number(value).is_ok_and(|n| n > 0.0) {
                return Err("a text's size is its letter height: one number above 0, such as size=0.03".into());
            }
        } else if !pair(value) {
            return Err(format!("{key} needs two numbers, such as {key}=0.3,0.05"));
        }
    }
    if let Some(value) = rest.get("text_size")
        && !number(value).is_ok_and(|n| n > 0.0)
    {
        return Err("text_size must be a number above 0".into());
    }
    for key in ["gap", "pad", "radius"] {
        if let Some(value) = rest.get(key)
            && !number(value).is_ok_and(|n| n >= 0.0)
        {
            return Err(format!("{key} must be a number that is not negative"));
        }
    }
    match kind.as_str() {
        "text" | "button" => {
            nonempty(rest.get("text").map_or("", String::as_str), "this element needs text=\"...\"")?;
        }
        "bar" => {
            nonempty(rest.get("value").map_or("", String::as_str), "a bar needs value= (a variable or a number)")?;
        }
        "slot" => {
            nonempty(rest.get("value").map_or("", String::as_str), "a slot needs value= (the cooldown left, such as hero.cd_smash) and max= (hero.cdmax_smash)")?;
            if let Some(icon) = rest.get("icon")
                && !ICONS.contains(&icon.as_str())
            {
                return Err(format!("unknown icon '{icon}' (use {})", ICONS.join(", ")));
            }
        }
        "stack" => {
            if !matches!(rest.get("dir").map_or("down", String::as_str), "down" | "right") {
                return Err("dir must be down or right".into());
            }
            if !matches!(rest.get("align").map_or("start", String::as_str), "start" | "center" | "end") {
                return Err("align must be start, center or end".into());
            }
        }
        "grid" if rest.get("cols").is_some_and(|c| !c.parse::<usize>().is_ok_and(|n| n >= 1)) => {
            return Err("cols must be a whole number of at least 1".into());
        }
        "minimap" if rest.get("radius").is_some_and(|r| number(r).is_ok_and(|n| n <= 0.0)) => {
            return Err("radius must be more than 0 (metres from the centre to the edge)".into());
        }
        _ => {}
    }
    if let Some(when) = rest.get("when") {
        crate::expr::Expr::parse(when).map_err(|error| format!("when: {error}"))?;
    }
    if let Some(parent) = rest.get("parent") {
        let Some(found) = def.ui.iter().find(|item| item.id == *parent) else {
            return Err(format!("parent '{parent}' must be declared before its children"));
        };
        if !matches!(found.kind.as_str(), "stack" | "grid") {
            return Err(format!("parent '{parent}' is a {}, not a container (stack or grid)", found.kind));
        }
    }
    for action in rest.get("do").map_or("", String::as_str).split(';').map(str::trim).filter(|a| !a.is_empty()) {
        validate_action(action)?;
    }
    def.ui.push(UiDef { id: id.to_owned(), kind: kind.clone(), fields: rest, line });
    Ok(())
}

/// Checks that need the whole game: states in `show`/`hide`, state and level names in `do=`, and variables in bindings.
pub(crate) fn check(def: &GameDef) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for item in &def.ui {
        let mut complain = |message: String, hint: &str| out.push(Diagnostic::new(item.line, 1, message, hint));
        for key in ["show", "hide"] {
            for name in item.list(key) {
                if !def.states.iter().any(|state| state.name == name) {
                    complain(format!("{key}={name}: no state named '{name}'"), "declare it with: state NAME");
                }
            }
        }
        for action in item.actions() {
            if let Some(name) = action.strip_prefix("go:").or_else(|| action.strip_prefix("restart:"))
                && !def.states.iter().any(|state| state.name == name)
            {
                complain(format!("no state named '{name}' for {action}"), "declare it with: state NAME");
            }
            if let Some(name) = action.strip_prefix("level:")
                && !def.levels.iter().any(|level| level.name == name)
            {
                complain(format!("no level named '{name}' for {action}"), "declare it with: level NAME");
            }
        }
        for key in ["text", "tip", "label"] {
            for name in bindings(item.field(key)) {
                if !name.starts_with("key:") && !binding_ok(def, name) {
                    complain(format!("{key} binds {{{name}}}, but there is no variable '{name}'"), "declare it with: var NAME=0, or use KIND.prop such as hero.health");
                }
            }
        }
        if let Ok(expr) = crate::expr::Expr::parse(item.field("when")) {
            for name in crate::parse::helpers::expr_names(&expr) {
                if !binding_ok(def, &name) {
                    complain(format!("when reads '{name}', but there is no variable or kind.prop of that name"), "declare it with: var NAME=0, or use KIND.prop such as hero.health or targeted.health");
                }
            }
        }
        for key in ["value", "max"] {
            let text = item.field(key);
            if !text.is_empty() && number(text).is_err() && !binding_ok(def, text) {
                complain(format!("{key}={text} is neither a number nor a variable"), "declare it with: var NAME=0, or use KIND.prop such as hero.health");
            }
        }
    }
    out
}

/// Whether a binding names something that can exist: a declared variable, or `KIND.prop` / `TAG.prop` for a declared kind or tag.
fn binding_ok(def: &GameDef, name: &str) -> bool {
    // `targeted.PROP` is the leader's current target, whoever that is
    def.vars.iter().any(|var| var.name == name)
        || name.starts_with("targeted.")
        || name.rsplit_once('.').is_some_and(|(who, _)| def.kinds.iter().any(|kind| kind.name == who || kind.tags.iter().any(|tag| tag == who)))
}

/// The names inside `{...}` in a text.
pub(crate) fn bindings(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else { break };
        names.push(&rest[open + 1..open + close]);
        rest = &rest[open + close + 1..];
    }
    names
}
