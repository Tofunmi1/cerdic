//! The declared game UI (F4 of `plans/game-foundations-plan.md`, ADR 0074): `ui ID type=... parent=... show=...` statements make a
//! screen-space tree (text, bar, panel, button, stack, grid, minimap) laid out in fractions of the screen height, bound to variables,
//! shown only in the flow states they name, navigated by `ui_next`/`ui_prev`/`ui_accept`, clicked by `ui:ID`, and hit-tested. Written
//! before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::collections::BTreeMap;

use ve_game::{GameDef, Inputs, UiBox, World};

const ASPECT: f64 = 2.0;

const GAME: &str = "game name=ui
var score=7
var hp=30
var clicks=0
kind hero size=0.4 tags=hero
spawn hero at=1,0,2
input start key=press
state title frozen=1
state playing
ui menu type=stack anchor=c dir=down gap=0.02 pad=0.03 fill=rgba(0,0,0,0.6) show=title
ui play type=button parent=menu text=Play size=0.3,0.06 do=go:playing
ui opts type=button parent=menu text=Options size=0.3,0.06 tip=\"Change the settings\"
ui quit type=button parent=menu text=Quit size=0.3,0.06 do=add:clicks:10
ui score type=text anchor=tr text=\"Score {score}\" show=playing
ui hp type=bar anchor=tl size=0.3,0.03 value=hp max=100 show=playing
rule on=ui:opts do=add:clicks:1
";

fn world() -> World {
    World::new(&GameDef::parse(GAME).unwrap(), 1)
}

fn tap(world: &mut World, names: &[&str]) {
    let inputs = Inputs { pressed: names.iter().map(|n| (*n).to_owned()).collect(), ..Inputs::default() };
    world.step(&inputs).unwrap();
}

fn find<'a>(boxes: &'a [UiBox], id: &str) -> &'a UiBox {
    boxes.iter().find(|b| b.id == id).unwrap()
}

fn near(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() < 1e-9, "{what}: {a} against {b}");
}

#[test]
fn only_the_elements_of_the_current_state_are_laid_out() {
    let mut w = world();
    let ids = |w: &World| w.ui_layout(ASPECT).into_iter().map(|b| b.id).collect::<Vec<_>>();
    assert_eq!(ids(&w), ["menu", "play", "opts", "quit"], "the title shows the menu (a container first, then its buttons)");
    tap(&mut w, &["ui_accept"]);
    assert_eq!(w.flow_state(), "playing");
    assert_eq!(ids(&w), ["score", "hp"]);
}

#[test]
fn a_stack_places_its_children_one_after_the_other_with_gap_and_padding() {
    let boxes = world().ui_layout(ASPECT);
    let menu = find(&boxes, "menu");
    near(menu.rect[2], 0.3 + 2.0 * 0.03, "width: the widest child plus the padding");
    near(menu.rect[3], 3.0 * 0.06 + 2.0 * 0.02 + 2.0 * 0.03, "height: three buttons, two gaps, the padding");
    // anchored at the centre of the usable area (a margin of 0.05 on every side)
    near(menu.rect[0], 0.05 + 0.5 * (ASPECT - 0.1) - 0.5 * menu.rect[2], "centred in x");
    near(menu.rect[1], 0.05 + 0.5 * 0.9 - 0.5 * menu.rect[3], "centred in y");
    let (play, opts, quit) = (find(&boxes, "play"), find(&boxes, "opts"), find(&boxes, "quit"));
    near(play.rect[0], menu.rect[0] + 0.03, "left padding");
    near(play.rect[1], menu.rect[1] + 0.03, "top padding");
    near(opts.rect[1], play.rect[1] + 0.06 + 0.02, "second button below the first plus the gap");
    near(quit.rect[1], opts.rect[1] + 0.06 + 0.02, "and the third");
    near(quit.rect[2], 0.3, "children keep their size");
}

#[test]
fn a_horizontal_stack_and_alignment_and_a_grid() {
    let text = "game name=ui2
kind hero size=0.4
ui row type=stack dir=right gap=0.01 pad=0 anchor=tl align=center
ui a type=panel parent=row size=0.1,0.1
ui b type=panel parent=row size=0.2,0.05
ui g type=grid anchor=br cols=3 cell=0.1,0.1 gap=0.01 pad=0.02
ui g0 type=panel parent=g size=0.05,0.05
ui g1 type=panel parent=g size=0.05,0.05
ui g2 type=panel parent=g size=0.05,0.05
ui g3 type=panel parent=g size=0.05,0.05
";
    let boxes = World::new(&GameDef::parse(text).unwrap(), 1).ui_layout(ASPECT);
    let (a, b, row) = (find(&boxes, "a"), find(&boxes, "b"), find(&boxes, "row"));
    near(b.rect[0], a.rect[0] + 0.1 + 0.01, "b to the right of a plus the gap");
    near(row.rect[2], 0.1 + 0.01 + 0.2, "the row is as wide as its children and gap");
    near(row.rect[3], 0.1, "and as tall as the tallest");
    near(b.rect[1], a.rect[1] + (0.1 - 0.05) / 2.0, "the shorter one is centred across the row");
    let (g0, g1, g3, grid) = (find(&boxes, "g0"), find(&boxes, "g1"), find(&boxes, "g3"), find(&boxes, "g"));
    near(g1.rect[0] - g0.rect[0], 0.11, "grid columns are a cell and a gap apart");
    near(g3.rect[0], g0.rect[0], "the fourth wraps to the first column");
    near(g3.rect[1] - g0.rect[1], 0.11, "on the second row");
    near(grid.rect[2], 3.0 * 0.1 + 2.0 * 0.01 + 2.0 * 0.02, "three columns wide");
    near(grid.rect[3], 2.0 * 0.1 + 0.01 + 2.0 * 0.02, "two rows tall");
}

#[test]
fn text_is_bound_to_variables_and_bars_to_a_fraction() {
    let mut w = world();
    tap(&mut w, &["ui_accept"]);
    let boxes = w.ui_layout(ASPECT);
    assert_eq!(find(&boxes, "score").text, "Score 7");
    near(find(&boxes, "hp").fraction, 0.30, "30 of 100");
    let mut w2 = World::new(&GameDef::parse(&GAME.replace("var score=7", "var score=12.5")).unwrap(), 1);
    tap(&mut w2, &["ui_accept"]);
    assert_eq!(find(&w2.ui_layout(ASPECT), "score").text, "Score 12.5", "a fraction keeps its digits and a whole number drops them");
}

#[test]
fn text_and_bars_can_read_the_props_of_an_entity_by_kind_or_tag() {
    let text = "game name=h
kind hero size=0.4 tags=player
health hero hp=80
spawn hero at=0,0,0
input hit key=press
rule on=input:hit do=hurt:kind:hero:20
ui hp type=bar size=0.3,0.03 value=hero.health max=hero.max_health
ui label type=text text=\"HP {hero.health} of {player.max_health}\"
";
    let mut w = World::new(&GameDef::parse(text).unwrap(), 1);
    let boxes = w.ui_layout(ASPECT);
    near(find(&boxes, "hp").fraction, 1.0, "full");
    assert_eq!(find(&boxes, "label").text, "HP 80 of 80", "by kind name and by tag");
    tap(&mut w, &["hit"]);
    let boxes = w.ui_layout(ASPECT);
    near(find(&boxes, "hp").fraction, 0.75, "60 of 80");
    assert_eq!(find(&boxes, "label").text, "HP 60 of 80");
    let bad = GameDef::parse("game name=h\nkind hero size=0.4\nui l type=text text=\"{ghost.health}\"\n");
    assert!(bad.unwrap_err().iter().any(|e| e.message.contains("ghost")), "a kind or tag that does not exist is refused");
}

#[test]
fn focus_moves_through_the_visible_buttons_and_wraps() {
    let mut w = world();
    let focused = |w: &World| w.ui_layout(ASPECT).into_iter().find(|b| b.focused).map(|b| b.id);
    assert_eq!(focused(&w).as_deref(), Some("play"), "the first button starts focused");
    tap(&mut w, &["ui_next"]);
    assert_eq!(focused(&w).as_deref(), Some("opts"));
    tap(&mut w, &["ui_next", ]);
    tap(&mut w, &["ui_next"]);
    assert_eq!(focused(&w).as_deref(), Some("play"), "wraps past the last");
    tap(&mut w, &["ui_prev"]);
    assert_eq!(focused(&w).as_deref(), Some("quit"), "and back before the first");
}

#[test]
fn accepting_a_button_raises_its_event_and_runs_its_actions() {
    let mut w = world();
    tap(&mut w, &["ui_next"]);
    tap(&mut w, &["ui_accept"]);
    assert_eq!(w.vars().get("clicks").copied(), Some(1.0), "the rule on ui:opts ran");
    assert_eq!(w.flow_state(), "title");
    tap(&mut w, &["ui_next"]);
    tap(&mut w, &["ui_accept"]);
    assert_eq!(w.vars().get("clicks").copied(), Some(11.0), "quit's own do= ran");
    tap(&mut w, &["ui_next"]);
    tap(&mut w, &["ui_accept"]);
    assert_eq!(w.flow_state(), "playing", "play's do= moved the game on");
}

#[test]
fn hidden_buttons_are_not_focusable_and_nothing_is_heard_without_any() {
    let mut w = world();
    tap(&mut w, &["ui_accept"]);
    assert_eq!(w.flow_state(), "playing");
    tap(&mut w, &["ui_next"]);
    tap(&mut w, &["ui_accept"]);
    assert_eq!(w.vars().get("clicks").copied(), Some(0.0), "the title's buttons are gone");
    assert!(w.ui_layout(ASPECT).iter().all(|b| !b.focused));
}

#[test]
fn a_click_on_a_button_focuses_and_presses_it() {
    let mut w = world();
    let boxes = w.ui_layout(ASPECT);
    let opts = find(&boxes, "opts");
    let (x, y) = (opts.rect[0] + opts.rect[2] / 2.0, opts.rect[1] + opts.rect[3] / 2.0);
    assert_eq!(w.ui_hit(ASPECT, x, y).as_deref(), Some("opts"));
    assert_eq!(w.ui_hit(ASPECT, 0.01, 0.01), None, "empty screen");
    let menu = find(&boxes, "menu");
    assert_eq!(w.ui_hit(ASPECT, menu.rect[0] + 0.005, menu.rect[1] + 0.005).as_deref(), None, "the container's padding is not a button");
    tap(&mut w, &["ui:opts"]);
    assert_eq!(w.vars().get("clicks").copied(), Some(1.0));
    assert!(w.ui_layout(ASPECT).iter().any(|b| b.id == "opts" && b.focused), "and it keeps the focus");
}

#[test]
fn a_tooltip_follows_the_focused_button() {
    let mut w = world();
    assert!(w.ui_layout(ASPECT).iter().all(|b| b.kind != "tooltip"), "no tip on the first button");
    tap(&mut w, &["ui_next"]);
    let boxes = w.ui_layout(ASPECT);
    let tip = boxes.iter().find(|b| b.kind == "tooltip").expect("options has a tip");
    assert_eq!(tip.text, "Change the settings");
    let opts = find(&boxes, "opts");
    assert!(tip.rect[1] >= opts.rect[1] + opts.rect[3], "below the button it explains");
}

#[test]
fn a_prompt_names_the_key_the_page_supplied() {
    let text = "game name=p\nkind hero size=0.4\nui hint type=text anchor=b text=\"Press {key:jump} to jump\"\nui plain type=text anchor=t text=\"Press {key:missing}\"\n";
    let mut w = World::new(&GameDef::parse(text).unwrap(), 1);
    assert_eq!(find(&w.ui_layout(ASPECT), "hint").text, "Press jump to jump", "without labels the action's own name");
    w.set_ui_labels(BTreeMap::from([("jump".to_owned(), "Space".to_owned())]));
    let boxes = w.ui_layout(ASPECT);
    assert_eq!(find(&boxes, "hint").text, "Press Space to jump");
    assert_eq!(find(&boxes, "plain").text, "Press missing");
}

#[test]
fn a_minimap_shows_the_tagged_entities_around_the_centre() {
    let text = "game name=m
kind hero size=0.4 tags=hero
kind coin size=0.3 tags=coin
spawn hero at=10,0,10
spawn coin at=12,0,10
spawn coin at=10,0,7
spawn coin at=100,0,100
ui map type=minimap anchor=br size=0.2,0.2 radius=5 follow=tag:hero tags=coin
";
    let boxes = World::new(&GameDef::parse(text).unwrap(), 1).ui_layout(ASPECT);
    let map = find(&boxes, "map");
    assert_eq!(map.dots.len(), 2, "two coins within five metres, the far one is off the map");
    let [cx, cy] = [map.rect[0] + map.rect[2] / 2.0, map.rect[1] + map.rect[3] / 2.0];
    let east = map.dots.iter().find(|d| d[0] > cx + 1e-9).expect("the coin to the east");
    near(east[0], cx + 0.2 / 2.0 * (2.0 / 5.0), "2 m of 5 m radius, half the map wide");
    near(east[1], cy, "level with the centre");
    let behind = map.dots.iter().find(|d| (d[0] - cx).abs() < 1e-9).expect("the coin straight behind the hero in z");
    near(behind[1], cy + 0.2 / 2.0 * (3.0 / 5.0), "3 m of 5 m radius; +z is up the map, so a smaller z is lower");
}

#[test]
fn the_frame_has_rectangles_and_strokes_for_what_is_visible() {
    let w = world();
    let frame: serde_json::Value = serde_json::from_str(&w.ui_frame(ASPECT)).unwrap();
    let items = frame["items"].as_array().unwrap();
    assert!(items.iter().any(|i| i["k"] == "rect"), "the menu panel and buttons are rectangles");
    assert!(items.iter().any(|i| i["k"] == "path"), "and their words are pen strokes");
    let mut playing = world();
    tap(&mut playing, &["ui_accept"]);
    let later: serde_json::Value = serde_json::from_str(&playing.ui_frame(ASPECT)).unwrap();
    assert_ne!(frame["items"], later["items"], "the picture follows the state");
}

#[test]
fn the_focus_survives_a_save_and_the_ui_a_text_round_trip() {
    let mut w = world();
    tap(&mut w, &["ui_next"]);
    let saved = w.snapshot().to_json();
    let mut other = world();
    other.restore(&ve_game::Snapshot::from_json(&saved).unwrap()).unwrap();
    assert!(other.ui_layout(ASPECT).iter().any(|b| b.id == "opts" && b.focused));
    let def = GameDef::parse(GAME).unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def.to_text(), again.to_text());
    assert_eq!(World::new(&again, 1).ui_layout(ASPECT), world().ui_layout(ASPECT));
}

#[test]
fn mistakes_in_the_ui_are_refused_with_a_reason() {
    let refused = |extra: &str| {
        GameDef::parse(&format!("game name=bad\nkind hero size=0.4\nstate a\nstate b\n{extra}\n"))
            .map_or_else(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(" | "), |_| String::new())
    };
    assert!(refused("ui x type=sparkle").contains("type"));
    assert!(refused("ui x type=text").contains("text"));
    assert!(refused("ui x type=button").contains("text"));
    assert!(refused("ui x type=bar size=0.3,0.03").contains("value"));
    assert!(refused("ui x type=text text=hi parent=nowhere").contains("nowhere"));
    assert!(refused("ui p type=panel size=0.1,0.1\nui x type=text text=hi parent=p").contains("container"));
    assert!(refused("ui x type=text text=hi\nui x type=text text=hi").contains("duplicate"));
    assert!(refused("ui x type=text text=hi show=nowhere").contains("nowhere"));
    assert!(refused("ui x type=button text=hi do=go:nowhere").contains("nowhere"));
    assert!(refused("ui x type=panel size=0.1").contains("size"));
    assert!(refused("ui s type=stack dir=sideways").contains("dir"));
    assert!(refused("ui g type=grid cols=0").contains("cols"));
    assert!(refused("ui x type=text text=\"Score {nope}\"").contains("nope"), "a binding to a variable that does not exist");
    assert!(refused("ui m type=minimap size=0.2,0.2 radius=-1").contains("radius"));
    assert_eq!(refused("ui x type=text text=hi show=a"), "");
}

#[test]
fn a_game_without_ui_lays_out_nothing() {
    let w = World::new(&GameDef::parse("game name=none\nkind hero size=0.4\nspawn hero at=0,0,0\n").unwrap(), 1);
    assert!(w.ui_layout(ASPECT).is_empty());
}
