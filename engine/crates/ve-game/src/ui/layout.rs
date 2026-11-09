//! Lays the declared UI tree out (see `tree.rs`) for one screen shape and state: sizes, container placement, bindings, focus, minimap dots,
//! tooltips; and turns the result into the drawing specs `ui.rs` renders. All units are fractions of the screen height.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::tree::UiDef;
use crate::GameDef;
use crate::world::Entity;

/// One laid-out element, for the host, the pointer and tests.
#[derive(Clone, Debug, PartialEq)]
pub struct UiBox {
    /// The element's id (`tooltip` for the tip of the focused button).
    pub id: String,
    /// text, bar, panel, button, stack, grid, minimap or tooltip.
    pub kind: String,
    /// `[x, y, width, height]`, top-left origin, y down.
    pub rect: [f64; 4],
    /// The words shown, with bindings filled in (text, button, tooltip).
    pub text: String,
    /// Whether this is the button that has the focus.
    pub focused: bool,
    /// A bar's filled share, 0 to 1.
    pub fraction: f64,
    /// A minimap's dots as screen positions.
    pub dots: Vec<[f64; 2]>,
    /// A slot's key label (`{key:power}` filled in).
    pub label: String,
}

/// A number that floats up from where something was hurt or healed, until it fades.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Floater {
    /// Where it started, in the world.
    pub pos: [f64; 3],
    /// The words: `12`, `+7`, `Dodge!`.
    pub text: String,
    /// 0 damage, 1 healing, 2 dodge.
    pub kind: u8,
    /// Fixed steps since it appeared.
    pub age: u32,
}

/// How long a floating number lasts, in fixed steps.
pub(crate) const FLOATER_LIFE: u32 = 70;

/// What the layout reads from the world.
pub(crate) struct Context<'a> {
    /// Elements hidden by their `when=`.
    pub hidden: &'a BTreeSet<String>,
    /// The camera the host reported (`camera.x/y/z/yaw/pitch/fov`), for projecting floating numbers.
    pub camera: &'a BTreeMap<String, f64>,
    pub floaters: &'a [Floater],
    pub aspect: f64,
    pub def: &'a GameDef,
    pub state: &'a str,
    pub vars: &'a BTreeMap<String, f64>,
    pub labels: &'a BTreeMap<String, String>,
    pub focus: &'a str,
    pub entities: &'a [Entity],
}

const TEXT_SIZE: f64 = 0.03;
const TIP_SIZE: f64 = 0.022;

fn num(item: &UiDef, key: &str, default: f64) -> f64 {
    item.fields.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn pair(item: &UiDef, key: &str) -> Option<[f64; 2]> {
    let text = item.fields.get(key)?;
    let mut parts = text.split(',').map(|p| p.parse::<f64>().ok());
    Some([parts.next()??, parts.next()??])
}

/// A count of elements as a number to multiply by (a screen holds a few hundred at most).
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// A grid's column count (validated as a whole number of at least 1 when the game is read).
fn columns(item: &UiDef) -> usize {
    item.fields.get("cols").and_then(|c| c.parse().ok()).unwrap_or(3).max(1)
}

fn show_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 { format!("{value:.0}") } else { value.to_string() }
}

/// A number a binding names: a variable, or `KIND.prop` / `TAG.prop` for the first entity of that kind or tag (`hero.health`).
fn lookup(name: &str, cx: &Context) -> Option<f64> {
    if let Some(value) = cx.vars.get(name) {
        return Some(*value);
    }
    let (who, prop) = name.rsplit_once('.')?;
    if who == "targeted" {
        // the leader's current target, whoever it is
        return cx.entities.iter().find(|e| e.props.get("targeted").is_some_and(|t| *t > 0.0)).and_then(|e| e.props.get(prop).copied());
    }
    let tag = format!("tag:{who}");
    cx.entities
        .iter()
        .find(|e| e.kind == who || crate::world::matches_selector(cx.def, &tag, e))
        .and_then(|e| e.props.get(prop).copied())
}

/// `text` with every `{name}` replaced by the variable's value, or `{key:ACTION}` by the page's label for the action.
fn interpolate(text: &str, cx: &Context) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else { break };
        out.push_str(&rest[..open]);
        let name = &rest[open + 1..open + close];
        match name.strip_prefix("key:") {
            Some(action) => out.push_str(cx.labels.get(action).map_or(action, String::as_str)),
            None => out.push_str(&show_number(lookup(name, cx).unwrap_or(0.0))),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

fn shown(def: &GameDef, item: &UiDef, cx: &Context) -> bool {
    let (show, state) = (item.list("show"), cx.state);
    let in_state = (show.is_empty() || show.contains(&state)) && !item.list("hide").contains(&state) && !cx.hidden.contains(&item.id);
    in_state && item.fields.get("parent").is_none_or(|parent| def.ui.iter().find(|p| p.id == *parent).is_some_and(|p| shown(def, p, cx)))
}

fn children<'a>(def: &'a GameDef, parent: &UiDef, cx: &Context) -> Vec<&'a UiDef> {
    def.ui.iter().filter(|c| c.fields.get("parent") == Some(&parent.id) && shown(def, c, cx)).collect()
}

/// The elements whose `when=` expression is false right now (they and their children are not shown).
pub(crate) fn hidden_by_when(def: &GameDef, cx: &Context) -> BTreeSet<String> {
    def.ui
        .iter()
        .filter(|item| !item.field("when").is_empty())
        .filter(|item| crate::expr::Expr::parse(item.field("when")).is_ok_and(|expr| expr.eval(&|name| lookup(name, cx).unwrap_or(0.0)) == 0.0))
        .map(|item| item.id.clone())
        .collect()
}

/// The visible buttons in focus order (declaration order).
pub(crate) fn visible_buttons<'a>(def: &'a GameDef, cx: &Context) -> Vec<&'a UiDef> {
    def.ui.iter().filter(|item| item.kind == "button" && shown(def, item, cx)).collect()
}

/// The button that has the focus: the remembered one while it is visible, otherwise the first visible button.
pub(crate) fn effective_focus(def: &GameDef, cx: &Context, remembered: &str) -> Option<String> {
    let buttons = visible_buttons(def, cx);
    buttons.iter().find(|b| b.id == remembered).or(buttons.first()).map(|b| b.id.clone())
}

fn text_size(item: &UiDef) -> f64 {
    if item.kind == "button" { num(item, "text_size", TEXT_SIZE) } else { num(item, "size", TEXT_SIZE) }.clamp(0.005, 1.0)
}

fn text_width(item: &UiDef, words: &str, size: f64) -> f64 {
    let mut spec = json!({ "text": words, "size": size });
    if let Some(font) = item.fields.get("font") {
        spec["font"] = Value::String(font.clone());
    }
    super::measure(&spec)
}

fn size_of(def: &GameDef, item: &UiDef, cx: &Context) -> [f64; 2] {
    match item.kind.as_str() {
        "text" => {
            let size = text_size(item);
            let plate = if item.field("plate") == "true" { size } else { 0.0 };
            [text_width(item, &interpolate(item.field("text"), cx), size) + plate, size + plate]
        }
        "bar" => pair(item, "size").unwrap_or([0.3, 0.025]),
        "panel" | "minimap" => pair(item, "size").unwrap_or([0.3, 0.2]),
        "slot" | "portrait" => pair(item, "size").unwrap_or([0.07, 0.07]),
        "button" => pair(item, "size").unwrap_or_else(|| {
            let size = text_size(item);
            [text_width(item, &interpolate(item.field("text"), cx), size) + 1.2 * size, 2.2 * size]
        }),
        "stack" => {
            let kids: Vec<[f64; 2]> = children(def, item, cx).iter().map(|c| size_of(def, c, cx)).collect();
            let (gap, pad) = (num(item, "gap", 0.01), num(item, "pad", 0.015));
            let gaps = gap * count(kids.len().saturating_sub(1));
            let (along, across) = if item.field("dir") == "right" {
                (kids.iter().map(|k| k[0]).sum::<f64>(), kids.iter().map(|k| k[1]).fold(0.0, f64::max))
            } else {
                (kids.iter().map(|k| k[1]).sum::<f64>(), kids.iter().map(|k| k[0]).fold(0.0, f64::max))
            };
            if item.field("dir") == "right" { [along + gaps + 2.0 * pad, across + 2.0 * pad] } else { [across + 2.0 * pad, along + gaps + 2.0 * pad] }
        }
        "grid" => {
            let items = children(def, item, cx).len();
            let cols = columns(item);
            let rows = items.div_ceil(cols);
            let (cell, gap, pad) = (pair(item, "cell").unwrap_or([0.1, 0.1]), num(item, "gap", 0.01), num(item, "pad", 0.01));
            let span = |n: usize, cell: f64| if n > 0 { count(n) * cell + count(n - 1) * gap } else { 0.0 };
            [span(cols.min(items), cell[0]) + 2.0 * pad, span(rows, cell[1]) + 2.0 * pad]
        }
        _ => [0.0, 0.0],
    }
}

fn place(def: &GameDef, item: &UiDef, cx: &Context, at: [f64; 2], focus: Option<&str>, out: &mut Vec<UiBox>) {
    let size = size_of(def, item, cx);
    let mut laid = UiBox {
        id: item.id.clone(),
        kind: item.kind.clone(),
        rect: [at[0], at[1], size[0], size[1]],
        text: String::new(),
        focused: item.kind == "button" && focus == Some(item.id.as_str()),
        fraction: 0.0,
        dots: Vec::new(),
        label: String::new(),
    };
    let read =|key: &str, default: f64| {
        let text = item.field(key);
        text.parse().ok().or_else(|| lookup(text, cx)).unwrap_or(default)
    };
    match item.kind.as_str() {
        "text" | "button" | "portrait" => laid.text = interpolate(item.field("text"), cx),
        "bar" => laid.fraction = (read("value", 0.0) / read("max", 1.0).max(1e-9)).clamp(0.0, 1.0),
        "slot" => {
            laid.label = interpolate(item.field("key"), cx);
            // the share of the cooldown still to wait, and the whole seconds left as the words on it
            let left = read("value", 0.0).max(0.0);
            laid.fraction = (left / read("max", 1.0).max(1e-9)).clamp(0.0, 1.0);
            if left > 0.0 {
                // whole seconds left, rounded up; the tiny allowance keeps 1.0000000002 s (float dust) from showing as 2
                laid.text = show_number((left - 1e-6).ceil().max(1.0));
            }
        }
        "minimap" => laid.dots = minimap_dots(def, item, cx, laid.rect),
        "floaters" => {
            out.extend(cx.floaters.iter().filter_map(|floater| floater_box(&item.id, floater, cx)));
            return;
        }
        _ => {}
    }
    out.push(laid);
    match item.kind.as_str() {
        "stack" => {
            let (gap, pad) = (num(item, "gap", 0.01), num(item, "pad", 0.015));
            let right = item.field("dir") == "right";
            let (mut x, mut y) = (at[0] + pad, at[1] + pad);
            for child in children(def, item, cx) {
                let kid = size_of(def, child, cx);
                let (room, used) = if right { (size[1] - 2.0 * pad, kid[1]) } else { (size[0] - 2.0 * pad, kid[0]) };
                let shift = match item.field("align") {
                    "center" => (room - used) / 2.0,
                    "end" => room - used,
                    _ => 0.0,
                };
                let corner = if right { [x, y + shift] } else { [x + shift, y] };
                place(def, child, cx, corner, focus, out);
                if right { x += kid[0] + gap } else { y += kid[1] + gap }
            }
        }
        "grid" => {
            let cols = columns(item);
            let (cell, gap, pad) = (pair(item, "cell").unwrap_or([0.1, 0.1]), num(item, "gap", 0.01), num(item, "pad", 0.01));
            for (index, child) in children(def, item, cx).into_iter().enumerate() {
                let corner = [at[0] + pad + count(index % cols) * (cell[0] + gap), at[1] + pad + count(index / cols) * (cell[1] + gap)];
                place(def, child, cx, corner, focus, out);
            }
        }
        _ => {}
    }
}

/// A point in the world as a position on the screen (in screen heights, y down), for the camera the host reported; `None` behind the camera.
pub(crate) fn project(point: [f64; 3], camera: &BTreeMap<String, f64>, aspect: f64) -> Option<[f64; 2]> {
    let get = |name: &str| camera.get(name).copied();
    let (eye, yaw, pitch) = ([get("camera.x")?, get("camera.y")?, get("camera.z")?], get("camera.yaw")?, get("camera.pitch")?);
    let fov = get("camera.fov").unwrap_or(60.0).clamp(10.0, 170.0).to_radians();
    let forward = [pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()];
    let right = [-yaw.cos(), 0.0, yaw.sin()];
    let up = [right[1] * forward[2] - right[2] * forward[1], right[2] * forward[0] - right[0] * forward[2], right[0] * forward[1] - right[1] * forward[0]];
    let to = [point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let depth = dot(to, forward);
    if depth < 0.1 {
        return None;
    }
    let half = (fov / 2.0).tan();
    Some([aspect * (0.5 + dot(to, right) / (depth * half * aspect) * 0.5), 0.5 - dot(to, up) / (depth * half) * 0.5])
}

/// One floating number as a box on the screen: it starts over the place it happened and rises as it ages.
fn floater_box(id: &str, floater: &Floater, cx: &Context) -> Option<UiBox> {
    let [x, y] = project(floater.pos, cx.camera, cx.aspect)?;
    let age = f64::from(floater.age) / f64::from(FLOATER_LIFE);
    let size = if floater.kind == 2 { 0.032 } else { 0.036 };
    let width = super::measure(&json!({ "text": floater.text, "size": size }));
    Some(UiBox {
        id: id.to_owned(),
        kind: "floater".into(),
        rect: [x - width / 2.0, y - 0.09 * age - size, width, size],
        text: floater.text.clone(),
        focused: false,
        fraction: age,
        dots: Vec::new(),
        label: floater.kind.to_string(),
    })
}

fn minimap_dots(def: &GameDef, item: &UiDef, cx: &Context, rect: [f64; 4]) -> Vec<[f64; 2]> {
    let radius = num(item, "radius", 20.0).max(1e-6);
    let centre = item.fields.get("follow").and_then(|selector| cx.entities.iter().find(|e| crate::world::matches_selector(def, selector, e)));
    let Some(centre) = centre else { return Vec::new() };
    let tags = item.list("tags");
    let kinds = item.list("kinds");
    cx.entities
        .iter()
        .filter(|e| kinds.contains(&e.kind.as_str()) || tags.iter().any(|tag| crate::world::matches_selector(def, &format!("tag:{tag}"), e)))
        .filter_map(|e| {
            let (dx, dz) = (e.position[0] - centre.position[0], e.position[2] - centre.position[2]);
            (dx.abs() <= radius && dz.abs() <= radius).then(|| [rect[0] + rect[2] / 2.0 + dx / radius * rect[2] / 2.0, rect[1] + rect[3] / 2.0 - dz / radius * rect[3] / 2.0])
        })
        .collect()
}

/// The origin of a top-level element: its anchor on the screen plus its offset, as `ui.rs` computes it for scripted elements.
fn origin(item: &UiDef, size: [f64; 2], aspect: f64) -> [f64; 2] {
    let mut spec = json!({});
    for key in ["anchor", "pivot"] {
        if let Some(value) = item.fields.get(key) {
            spec[key] = Value::String(value.clone());
        }
    }
    if let Some(at) = pair(item, "at") {
        spec["at"] = json!(at);
    }
    super::origin(&spec, size, aspect)
}

/// Every visible element laid out for a screen of this aspect ratio, containers before their children, then the focused button's tooltip.
pub(crate) fn layout(def: &GameDef, cx: &Context, aspect: f64) -> Vec<UiBox> {
    let focus = effective_focus(def, cx, cx.focus);
    let mut out = Vec::new();
    for item in def.ui.iter().filter(|item| !item.fields.contains_key("parent") && shown(def, item, cx)) {
        let size = size_of(def, item, cx);
        place(def, item, cx, origin(item, size, aspect), focus.as_deref(), &mut out);
    }
    if let Some(button) = focus.as_deref().and_then(|id| out.iter().find(|b| b.id == id)).cloned()
        && let Some(tip) = def.ui.iter().find(|i| i.id == button.id).map(|i| interpolate(i.field("tip"), cx)).filter(|t| !t.is_empty())
    {
        let width = text_width(&UiDef { id: String::new(), kind: "text".into(), fields: BTreeMap::new(), line: 0 }, &tip, TIP_SIZE) + TIP_SIZE;
        out.push(UiBox {
            id: "tooltip".into(),
            kind: "tooltip".into(),
            rect: [button.rect[0], button.rect[1] + button.rect[3] + 0.01, width, TIP_SIZE * 2.0],
            text: tip,
            focused: false,
            fraction: 0.0,
            dots: Vec::new(),
            label: String::new(),
        });
    }
    out
}

/// The button under a point, if any (the topmost).
pub(crate) fn hit(boxes: &[UiBox], x: f64, y: f64) -> Option<String> {
    boxes.iter().rev().find(|b| b.kind == "button" && x >= b.rect[0] && x <= b.rect[0] + b.rect[2] && y >= b.rect[1] && y <= b.rect[1] + b.rect[3]).map(|b| b.id.clone())
}

fn to_value(text: &str) -> Value {
    if let Ok(n) = text.parse::<f64>() {
        return json!(n);
    }
    match text {
        "true" => return json!(true),
        "false" => return json!(false),
        _ => {}
    }
    let parts: Vec<f64> = text.split(',').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == text.split(',').count() && parts.len() > 1 { json!(parts) } else { json!(text) }
}

const VISUAL: [&str; 13] = ["font", "color", "fill", "border", "border_w", "back", "label_color", "plate", "plate_color", "shadow", "shadow_color", "trail", "trail_color"];

fn styled(item: &UiDef, mut spec: Value) -> Value {
    for key in VISUAL {
        if let Some(value) = item.fields.get(key) {
            spec[key] = to_value(value);
        }
    }
    if item.kind != "minimap"
        && let Some(radius) = item.fields.get("radius")
    {
        spec["radius"] = to_value(radius);
    }
    spec
}

/// A panel spec covering a box, with extra keys merged in.
fn panel(b: &UiBox, extra: Value) -> Value {
    let mut spec = json!({ "t": "panel", "abs": [b.rect[0], b.rect[1]], "size": [b.rect[2], b.rect[3]] });
    if let (Some(map), Value::Object(more)) = (spec.as_object_mut(), extra) {
        map.extend(more);
    }
    spec
}

/// The drawing specs `ui::frame` renders for these boxes, in draw order.
pub(crate) fn specs(def: &GameDef, boxes: &[UiBox]) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for b in boxes {
        let Some(item) = def.ui.iter().find(|i| i.id == b.id) else {
            if b.kind == "tooltip" {
                out.push((b.id.clone(), panel(b, json!({ "fill": "rgba(10,12,18,0.92)", "border": "rgba(255,255,255,0.4)", "radius": 0.008 }))));
                out.push((format!("{}_text", b.id), json!({ "t": "text", "text": b.text, "size": TIP_SIZE, "abs": [b.rect[0] + TIP_SIZE / 2.0, b.rect[1] + TIP_SIZE / 2.0], "shadow": false })));
            }
            continue;
        };
        match b.kind.as_str() {
            "text" => out.push((b.id.clone(), styled(item, json!({ "t": "text", "text": b.text, "size": text_size(item), "abs": [b.rect[0], b.rect[1]] })))),
            "slot" => {
                let (x, y, w, h) = (b.rect[0], b.rect[1], b.rect[2], b.rect[3]);
                out.push((b.id.clone(), styled(item, panel(b, json!({ "fill": "rgba(20,24,34,0.88)", "border": "rgba(255,255,255,0.45)", "radius": 0.006 })))));
                if let Some(icon) = item.fields.get("icon") {
                    out.push((format!("{}_icon", b.id), json!({ "t": "icon", "name": icon, "abs": [x + w * 0.12, y + h * 0.12], "size": [w * 0.76, h * 0.76], "color": item.fields.get("color").map_or("#e8e0cc", String::as_str) })));
                }
                if b.fraction > 0.0 {
                    out.push((format!("{}_cooldown", b.id), json!({ "t": "panel", "abs": [x, y], "size": [w, h * b.fraction], "fill": "rgba(0,0,0,0.62)", "radius": 0.006 })));
                    let size = h * 0.42;
                    let tw = text_width(item, &b.text, size);
                    out.push((format!("{}_seconds", b.id), json!({ "t": "text", "text": b.text, "size": size, "abs": [x + (w - tw) / 2.0, y + (h - size) / 2.0], "shadow": true })));
                }
                if !b.label.is_empty() {
                    out.push((format!("{}_key", b.id), json!({ "t": "text", "text": b.label, "size": h * 0.26, "color": "#ffd76a", "abs": [x + w * 0.06, y + h * 0.7], "shadow": true })));
                }
            }
            "portrait" => {
                let (x, y, w, h) = (b.rect[0], b.rect[1], b.rect[2], b.rect[3]);
                out.push((b.id.clone(), styled(item, panel(b, json!({ "fill": "rgba(60,70,100,0.92)", "border": "#e8d9a8", "border_w": 0.004, "radius": w.min(h) / 2.0 })))));
                let size = h * 0.5;
                let tw = text_width(item, &b.text, size);
                out.push((format!("{}_initial", b.id), json!({ "t": "text", "text": b.text, "size": size, "abs": [x + (w - tw) / 2.0, y + (h - size) / 2.0], "shadow": false })));
            }
            "floater" => {
                let fade = 1.0 - ((b.fraction - 0.6) / 0.4).clamp(0.0, 1.0);
                let (r, g, bl) = match b.label.as_str() {
                    "1" => (126, 224, 138),
                    "2" => (255, 215, 106),
                    _ => (255, 255, 255),
                };
                out.push((format!("{}_{}", b.id, out.len()), json!({ "t": "text", "text": b.text, "size": b.rect[3], "color": format!("rgba({r},{g},{bl},{fade:.2})"), "shadow": true, "abs": [b.rect[0], b.rect[1]] })));
            }
            "bar" => out.push((b.id.clone(), styled(item, json!({ "t": "bar", "abs": [b.rect[0], b.rect[1]], "size": [b.rect[2], b.rect[3]], "value": b.fraction, "max": 1.0 })))),
            "panel" => out.push((b.id.clone(), styled(item, panel(b, json!({}))))),
            "stack" | "grid" => {
                if item.fields.contains_key("fill") || item.fields.contains_key("border") {
                    out.push((b.id.clone(), styled(item, panel(b, json!({})))));
                }
            }
            "button" => {
                let size = text_size(item);
                let (fill, border) = if b.focused { ("rgba(80,110,190,0.95)", "#ffffff") } else { ("rgba(30,36,52,0.85)", "rgba(255,255,255,0.25)") };
                let mut spec = panel(b, json!({ "fill": fill, "border": border, "radius": 0.012 }));
                spec = styled(item, spec);
                if b.focused {
                    spec["fill"] = json!(item.fields.get("focus_fill").map_or(fill, String::as_str));
                    spec["border"] = json!("#ffffff");
                }
                out.push((b.id.clone(), spec));
                let width = text_width(item, &b.text, size);
                let colour = item.fields.get("color").map_or("#ffffff", String::as_str);
                out.push((format!("{}_text", b.id), json!({ "t": "text", "text": b.text, "size": size, "color": colour, "abs": [b.rect[0] + (b.rect[2] - width) / 2.0, b.rect[1] + (b.rect[3] - size) / 2.0], "shadow": false })));
            }
            "minimap" => {
                out.push((b.id.clone(), styled(item, panel(b, json!({ "fill": "rgba(14,18,26,0.75)", "border": "rgba(255,255,255,0.35)", "radius": 0.01 })))));
                for (n, dot) in b.dots.iter().enumerate() {
                    out.push((format!("{}_dot{n}", b.id), json!({ "t": "panel", "abs": [dot[0] - 0.004, dot[1] - 0.004], "size": [0.008, 0.008], "fill": item.fields.get("dot").map_or("#ffd76a", String::as_str), "radius": 0.004 })));
                }
                out.push((format!("{}_me", b.id), json!({ "t": "panel", "abs": [b.rect[0] + b.rect[2] / 2.0 - 0.005, b.rect[1] + b.rect[3] / 2.0 - 0.005], "size": [0.01, 0.01], "fill": "#6ad7ff", "radius": 0.005 })));
            }
            _ => {}
        }
    }
    out
}
