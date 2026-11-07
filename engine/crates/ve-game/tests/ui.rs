//! Game UI from scripts: elements set by id are laid out into rectangles and pen strokes in screen heights, with procedural fonts.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixtures unwrap definitions that are valid by construction; layout values here are exact or compared with tolerance"
)]
use serde_json::Value;
use ve_game::{GameDef, Inputs, ScriptDef, World};

fn world(script: &str) -> World {
    let text = format!(
        "game name=ui\nkind a size=0.5\nspawn a at=0,0,0\n{}\n",
        ScriptDef::new("main", script).to_text()
    );
    let mut world = World::new(&GameDef::parse(&text).unwrap(), 1);
    world.step(&Inputs::default()).unwrap();
    world
}

fn frame(world: &World, aspect: f64) -> Value {
    serde_json::from_str(&world.ui_frame(aspect)).unwrap()
}

fn items<'a>(frame: &'a Value, kind: &str) -> Vec<&'a Value> {
    frame["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["k"] == kind)
        .collect()
}

#[test]
fn a_bar_fills_by_fraction_and_sits_inside_the_margin_at_its_anchor() {
    let mut w = world(
        "on('start', () => { ui.bar('hp', 30, 120, { anchor: 'tr', size: [0.4, 0.03] }); });",
    );
    assert!(w.take_script_log().is_empty());
    let f = frame(&w, 2.0);
    let rects = items(&f, "rect");
    assert_eq!(rects.len(), 2, "a back and a fill");
    let back = rects[0];
    assert!(
        (back["x"].as_f64().unwrap() + back["w"].as_f64().unwrap() - (2.0 - 0.05)).abs() < 1e-9,
        "right edge at the margin: {back}"
    );
    assert!((back["y"].as_f64().unwrap() - 0.05).abs() < 1e-9);
    let fill = rects[1];
    assert!(
        (fill["w"].as_f64().unwrap() - 0.1).abs() < 1e-9,
        "30 of 120 of 0.4 wide"
    );
    let wide = frame(&w, 3.0);
    let moved = items(&wide, "rect")[0]["x"].as_f64().unwrap();
    assert!(
        moved > back["x"].as_f64().unwrap(),
        "a wider screen moves a right-anchored bar right"
    );
}

#[test]
fn text_is_drawn_from_skeleton_strokes_and_the_axes_change_it() {
    let w = world(
        "on('start', () => { ui.text('a', 'HP 100', { size: 0.05, shadow: false }); ui.text('b', 'HP 100', { size: 0.05, shadow: false, font: { preset: 'bold', slant: 0.3 } }); \
         ui.text('c', 'HP 100', { size: 0.05, shadow: false, font: 'pixel' }); });",
    );
    let f = frame(&w, 1.78);
    let paths = items(&f, "path");
    assert!(
        paths.len() > 12,
        "H, P, 1, 0, 0 each have strokes: {}",
        paths.len()
    );
    let (plain, bold) = (
        paths[0],
        paths
            .iter()
            .find(|p| p["w"].as_f64() != paths[0]["w"].as_f64())
            .unwrap(),
    );
    assert!(
        bold["w"].as_f64().unwrap() > plain["w"].as_f64().unwrap(),
        "weight changes the pen"
    );
    assert!(plain["round"] == true);
    let pixel = paths.iter().rev().next().unwrap();
    assert!(
        pixel["round"] == false,
        "the pixel preset uses a square pen"
    );
}

#[test]
fn width_is_measured_and_a_stack_lays_out_rows_without_overlap() {
    let w = world(
        "on('start', () => { log(ui.measure('Knight', 0.04), ui.measure('Knight Knight', 0.04)); \
         ui.stack('s', [{ id: 'n', t: 'text', text: 'Knight' }, { id: 'hp', t: 'bar', value: 5, max: 10, size: [0.3, 0.025] }], { anchor: 'bl', at: [0, 0], gap: 0.01 }); });",
    );
    let mut w = w;
    let log = w.take_script_log();
    let widths: Vec<f64> = log[0].split(' ').map(|s| s.parse().unwrap()).collect();
    assert!(
        widths[1] > widths[0] * 1.8,
        "twice the text is about twice as wide: {log:?}"
    );
    let f = frame(&w, 1.78);
    let rects = items(&f, "rect");
    let bar_bottom = rects[0]["y"].as_f64().unwrap() + rects[0]["h"].as_f64().unwrap();
    assert!(
        (bar_bottom - 0.95).abs() < 1e-9,
        "the stack's bottom sits on the bottom margin: {bar_bottom}"
    );
    let text_bottom = items(&f, "path")
        .iter()
        .flat_map(|p| {
            p["pts"]
                .as_array()
                .unwrap()
                .iter()
                .skip(1)
                .step_by(2)
                .map(|v| v.as_f64().unwrap())
        })
        .fold(0.0_f64, f64::max);
    assert!(
        text_bottom < rects[0]["y"].as_f64().unwrap() + 0.02,
        "the text sits above the bar"
    );
}

#[test]
fn removing_and_clearing_elements_and_bad_specs_are_reported() {
    let mut w = world(
        "on('start', () => { ui.panel('p', { size: [0.2, 0.1] }); ui.text('t', 'x'); ui.remove('p'); try { ui.set('bad', { t: 'sparkle' }); } catch (e) { log('refused: ' + e.message); } });",
    );
    let log = w.take_script_log();
    assert!(
        log.iter()
            .any(|l| l.contains("refused") && l.contains("'text', 'bar' or 'panel'")),
        "{log:?}"
    );
    let f = frame(&w, 1.78);
    assert!(items(&f, "rect").is_empty(), "the panel was removed");
    assert!(!items(&f, "path").is_empty(), "the text stayed");
}

#[test]
fn a_game_without_scripts_has_an_empty_ui() {
    let def = GameDef::parse("game name=none\nkind a size=0.5\nspawn a at=0,0,0\n").unwrap();
    let mut w = World::new(&def, 1);
    w.step(&Inputs::default()).unwrap();
    assert!(items(&frame(&w, 1.78), "rect").is_empty());
}
