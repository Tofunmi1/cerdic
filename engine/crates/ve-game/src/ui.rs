//! Game UI: a retained set of elements (text, bars, panels) that scripts set by id, laid out into rectangles and pen strokes for the
//! renderer. Units are fractions of the screen height (x runs from 0 to the aspect ratio), so text scale and margins are global
//! settings. Text is drawn from procedural skeleton fonts (see [`font`]): there are no font files.
mod font;
pub(crate) mod layout;
pub(crate) mod tree;

pub use layout::UiBox;
pub use tree::UiDef;

use font::{Style, layout as layout_text};
use serde_json::{Value, json};

/// Margin kept clear at the screen edge, as a fraction of the height.
const MARGIN: f64 = 0.05;
/// The most elements a game may show at once.
pub(crate) const MAX_ELEMENTS: usize = 256;

fn number(spec: &Value, key: &str, default: f64) -> f64 {
    spec.get(key)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
fn pair(spec: &Value, key: &str, default: [f64; 2]) -> [f64; 2] {
    spec.get(key)
        .and_then(Value::as_array)
        .and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]))
        .filter(|p| p.iter().all(|v| v.is_finite()))
        .unwrap_or(default)
}
fn text<'a>(spec: &'a Value, key: &str, default: &'a str) -> &'a str {
    spec.get(key).and_then(Value::as_str).unwrap_or(default)
}

/// Where a named anchor sits on the screen, as (x fraction, y fraction) of the usable area.
fn anchor_fraction(name: &str) -> [f64; 2] {
    let x = match name.chars().nth(1).or_else(|| name.chars().next()) {
        Some('l') => 0.0,
        Some('r') => 1.0,
        _ => 0.5,
    };
    let y = match name.chars().next() {
        Some('t') => 0.0,
        Some('b') => 1.0,
        _ => 0.5,
    };
    match name {
        "c" | "" => [0.5, 0.5],
        "l" | "cl" => [0.0, 0.5],
        "r" | "cr" => [1.0, 0.5],
        "t" | "tc" => [0.5, 0.0],
        "b" | "bc" => [0.5, 1.0],
        _ => [x, y],
    }
}

fn style_of(spec: &Value) -> Style {
    let mut style = match spec.get("font") {
        Some(Value::String(name)) => Style::preset(name),
        Some(Value::Object(axes)) => Style::preset(
            axes.get("preset").and_then(Value::as_str).unwrap_or("sans"),
        )
        .map(|mut style| {
            let get = |key: &str, now: f64| {
                axes.get(key)
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite())
                    .unwrap_or(now)
            };
            style.weight = get("weight", style.weight).clamp(0.0, 1.0);
            style.width = get("width", style.width).clamp(0.5, 2.0);
            style.slant = get("slant", style.slant).clamp(-0.5, 0.5);
            style.spacing = get("spacing", style.spacing).clamp(0.0, 4.0);
            style.wobble = get("wobble", style.wobble).clamp(0.0, 0.6);
            style
        }),
        _ => None,
    };
    style = style.or_else(|| Style::preset("sans"));
    style.unwrap_or_else(|| Style {
        weight: 0.35,
        width: 1.0,
        slant: 0.0,
        spacing: 1.3,
        round: true,
        snap: 0.0,
        wobble: 0.0,
    })
}

/// Top-left corner of an element of the given size: its anchor point on the screen, plus the offset, minus the pivot's share of its size.
fn origin(spec: &Value, size: [f64; 2], aspect: f64) -> [f64; 2] {
    // an element a container has already placed says where its top-left corner is
    if spec.get("abs").is_some() {
        return pair(spec, "abs", [0.0, 0.0]);
    }
    let anchor = text(spec, "anchor", "tl");
    let pivot = anchor_fraction(text(spec, "pivot", anchor));
    let a = anchor_fraction(anchor);
    let at = pair(spec, "at", [0.0, 0.0]);
    let usable = [(aspect - 2.0 * MARGIN).max(0.0), 1.0 - 2.0 * MARGIN];
    [
        MARGIN + a[0] * usable[0] + at[0] - pivot[0] * size[0],
        MARGIN + a[1] * usable[1] + at[1] - pivot[1] * size[1],
    ]
}

fn rect(
    items: &mut Vec<Value>,
    at: [f64; 2],
    size: [f64; 2],
    fill: &str,
    border: Option<(&str, f64)>,
    radius: f64,
) {
    items.push(json!({
        "k": "rect", "x": at[0], "y": at[1], "w": size[0], "h": size[1], "fill": fill, "radius": radius,
        "stroke": border.map(|b| b.0), "stroke_w": border.map_or(0.0, |b| b.1),
    }));
}

fn paths(items: &mut Vec<Value>, laid: &font::Laid, at: [f64; 2], color: &str, round: bool) {
    for path in &laid.paths {
        let points: Vec<f64> = path
            .iter()
            .flat_map(|p| [at[0] + p[0], at[1] + p[1]])
            .collect();
        items.push(
            json!({ "k": "path", "pts": points, "w": laid.pen, "color": color, "round": round }),
        );
    }
}

fn draw_text(items: &mut Vec<Value>, spec: &Value, aspect: f64) {
    let size = number(spec, "size", 0.03).clamp(0.005, 1.0);
    let style = style_of(spec);
    let content = text(spec, "text", "");
    let laid = layout_text(content, size, &style);
    let pad = if spec.get("plate").and_then(Value::as_bool) == Some(true) {
        size * 0.5
    } else {
        0.0
    };
    let box_size = [laid.width + 2.0 * pad, size + 2.0 * pad];
    let top_left = origin(spec, box_size, aspect);
    if pad > 0.0 {
        rect(
            items,
            top_left,
            box_size,
            text(spec, "plate_color", "rgba(0,0,0,0.55)"),
            None,
            size * 0.4,
        );
    }
    let at = [top_left[0] + pad, top_left[1] + pad];
    if spec.get("shadow").and_then(Value::as_bool) != Some(false) {
        paths(
            items,
            &laid,
            [at[0] + size * 0.06, at[1] + size * 0.06],
            text(spec, "shadow_color", "rgba(0,0,0,0.6)"),
            style.round,
        );
    }
    paths(
        items,
        &laid,
        at,
        text(spec, "color", "#ffffff"),
        style.round,
    );
}

fn draw_bar(items: &mut Vec<Value>, spec: &Value, aspect: f64) {
    let size = pair(spec, "size", [0.3, 0.025]);
    let top_left = origin(spec, size, aspect);
    let max = number(spec, "max", 1.0).max(1e-9);
    let fraction = (number(spec, "value", 0.0) / max).clamp(0.0, 1.0);
    let radius = number(spec, "radius", size[1] * 0.3);
    rect(
        items,
        top_left,
        size,
        text(spec, "back", "rgba(0,0,0,0.55)"),
        Some((text(spec, "border", "rgba(255,255,255,0.35)"), 0.0025)),
        radius,
    );
    if fraction > 0.0 {
        rect(
            items,
            top_left,
            [size[0] * fraction, size[1]],
            text(spec, "color", "#d94a4a"),
            None,
            radius,
        );
    }
    if let Some(trail) = spec.get("trail").and_then(Value::as_f64) {
        let trail = (trail / max).clamp(0.0, 1.0);
        if trail > fraction {
            rect(
                items,
                [top_left[0] + size[0] * fraction, top_left[1]],
                [size[0] * (trail - fraction), size[1]],
                text(spec, "trail_color", "#ffd76a"),
                None,
                0.0,
            );
        }
    }
    if let Some(label) = spec.get("label").and_then(Value::as_str) {
        let label_size = size[1] * 0.72;
        let laid = layout_text(label, label_size, &style_of(spec));
        let at = [
            top_left[0] + (size[0] - laid.width) / 2.0,
            top_left[1] + (size[1] - label_size) / 2.0,
        ];
        paths(items, &laid, at, text(spec, "label_color", "#ffffff"), true);
    }
}

fn draw_panel(items: &mut Vec<Value>, spec: &Value, aspect: f64) {
    let size = pair(spec, "size", [0.3, 0.2]);
    let top_left = origin(spec, size, aspect);
    let border = spec
        .get("border")
        .and_then(Value::as_str)
        .map(|color| (color, number(spec, "border_w", 0.003)));
    rect(
        items,
        top_left,
        size,
        text(spec, "fill", "rgba(20,24,32,0.7)"),
        border,
        number(spec, "radius", 0.012),
    );
}

/// The strokes of an icon in a unit square (y down). Icons are drawn with the same pen as the procedural font: no image files.
fn icon_strokes(name: &str) -> Vec<Vec<[f64; 2]>> {
    match name {
        "sword" => vec![vec![[0.2, 0.8], [0.82, 0.18]], vec![[0.13, 0.58], [0.42, 0.87]], vec![[0.2, 0.8], [0.1, 0.92]]],
        "shield" => vec![vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.5], [0.5, 0.86], [0.2, 0.5], [0.2, 0.2]]],
        "bolt" => vec![vec![[0.58, 0.08], [0.25, 0.55], [0.5, 0.55], [0.4, 0.92], [0.76, 0.4], [0.52, 0.4], [0.58, 0.08]]],
        "heal" => vec![vec![[0.5, 0.18], [0.5, 0.82]], vec![[0.18, 0.5], [0.82, 0.5]]],
        "dash" => vec![vec![[0.18, 0.25], [0.48, 0.5], [0.18, 0.75]], vec![[0.5, 0.25], [0.8, 0.5], [0.5, 0.75]]],
        "flame" => vec![vec![[0.5, 0.08], [0.76, 0.45], [0.7, 0.76], [0.5, 0.92], [0.3, 0.76], [0.26, 0.5], [0.42, 0.62], [0.5, 0.08]]],
        "skull" => vec![
            vec![[0.25, 0.6], [0.2, 0.4], [0.3, 0.2], [0.7, 0.2], [0.8, 0.4], [0.75, 0.6], [0.65, 0.66], [0.65, 0.86], [0.35, 0.86], [0.35, 0.66], [0.25, 0.6]],
            vec![[0.38, 0.45], [0.4, 0.47]],
            vec![[0.6, 0.45], [0.62, 0.47]],
        ],
        _ => {
            // a five-pointed star: outer and inner points alternate around the centre
            let points: Vec<[f64; 2]> = (0..=10)
                .map(|i| {
                    let (angle, radius) = (f64::from(i) * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2, if i % 2 == 0 { 0.42 } else { 0.18 });
                    [0.5 + radius * angle.cos(), 0.52 + radius * angle.sin()]
                })
                .collect();
            vec![points]
        }
    }
}

fn draw_icon(items: &mut Vec<Value>, spec: &Value, aspect: f64) {
    let size = pair(spec, "size", [0.05, 0.05]);
    let top_left = origin(spec, size, aspect);
    let pen = size[0].min(size[1]) * 0.075;
    for stroke in icon_strokes(text(spec, "name", "star")) {
        let points: Vec<f64> = stroke.iter().flat_map(|p| [top_left[0] + p[0] * size[0], top_left[1] + p[1] * size[1]]).collect();
        items.push(json!({ "k": "path", "pts": points, "w": pen, "color": text(spec, "color", "#ffffff"), "round": true }));
    }
}

/// Lays every element out for a screen of the given aspect ratio (width over height).
pub(crate) fn frame(elements: &[(String, Value)], aspect: f64) -> Value {
    let aspect = if aspect.is_finite() && aspect > 0.2 {
        aspect
    } else {
        16.0 / 9.0
    };
    let mut items = Vec::new();
    for (_, spec) in elements {
        if spec.get("hidden").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        match text(spec, "t", "") {
            "text" => draw_text(&mut items, spec, aspect),
            "bar" => draw_bar(&mut items, spec, aspect),
            "panel" => draw_panel(&mut items, spec, aspect),
            "icon" => draw_icon(&mut items, spec, aspect),
            _ => {}
        }
    }
    json!({ "aspect": aspect, "items": items })
}

/// Whether a spec names an element kind the layout knows.
pub(crate) fn known(spec: &Value) -> bool {
    matches!(text(spec, "t", ""), "text" | "bar" | "panel" | "icon")
}

/// The width a text would take at a size and font, in screen heights (for scripts that lay out rows themselves).
pub(crate) fn measure(spec: &Value) -> f64 {
    layout_text(
        text(spec, "text", ""),
        number(spec, "size", 0.03).clamp(0.005, 1.0),
        &style_of(spec),
    )
    .width
}
