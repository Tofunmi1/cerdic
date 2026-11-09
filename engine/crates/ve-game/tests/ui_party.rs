//! The party game's screen elements (extending the UI of ADR 0074): an ability `slot` with an icon, a key label and a cooldown overlay, a
//! `portrait`, `when=` to show an element only while an expression holds (a target frame while something is targeted), the `targeted`
//! binding (`{targeted.health}`), and `floaters`: damage numbers and "Dodge!" projected from the world to the screen. Written first.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{GameDef, Inputs, UiBox, World};

const ASPECT: f64 = 2.0;

const GAME: &str = "game name=hud
kind hero size=0.4 tags=hero
kind imp size=0.5 tags=enemy
health hero hp=100 iframes=0.5
health imp hp=40 height=1.2
input power key=press
input hit key=press
input mend key=press
targeting leader=tag:hero foes=tag:enemy
ability smash owner=tag:hero input=power cooldown=2 cost=0 range=8 needs_target=1 do=hurt:b:5
rule on=input:hit do=hurt:kind:imp:12
rule on=input:mend do=heal:kind:hero:7
spawn hero at=0,0,0
spawn imp at=0,0,4
ui frame type=panel size=0.2,0.1 anchor=tr when=targeted.health>0
ui name type=text anchor=tl text=\"Imp {targeted.health}\" when=targeted.health>0
ui slot1 type=slot size=0.08,0.08 anchor=bc value=hero.cd_smash max=hero.cdmax_smash icon=sword key=\"{key:power}\"
ui who type=portrait size=0.07,0.07 anchor=cl text=H fill=rgba(40,60,120,0.9)
ui nums type=floaters
";

fn world(extra: &str) -> World {
    World::new(&GameDef::parse(&format!("{GAME}{extra}")).unwrap(), 4)
}

fn step(world: &mut World, pressed: &[&str], axes: &[(&str, f64)]) {
    let mut inputs = Inputs { pressed: pressed.iter().map(|p| (*p).to_owned()).collect(), ..Inputs::default() };
    for (name, value) in axes {
        inputs.axes.insert((*name).to_owned(), *value);
    }
    world.step(&inputs).unwrap();
}

fn find<'a>(boxes: &'a [UiBox], id: &str) -> Option<&'a UiBox> {
    boxes.iter().find(|b| b.id == id)
}

fn near(a: f64, b: f64, tolerance: f64, what: &str) {
    assert!((a - b).abs() <= tolerance, "{what}: {a} against {b}");
}

/// The camera stands behind the hero looking along +z, two metres up, with a 60 degree view.
const CAMERA: [(&str, f64); 6] = [("camera.x", 0.0), ("camera.y", 2.0), ("camera.z", -8.0), ("camera.yaw", 0.0), ("camera.pitch", 0.0), ("camera.fov", 60.0)];

#[test]
fn when_shows_an_element_only_while_its_expression_holds() {
    let mut w = world("");
    let before = w.ui_layout(ASPECT);
    assert!(find(&before, "frame").is_none() && find(&before, "name").is_none(), "nothing is targeted yet: the target frame is hidden");
    step(&mut w, &[], &[]);
    let boxes = w.ui_layout(ASPECT);
    assert!(find(&boxes, "frame").is_some(), "the imp is targeted: the frame shows");
    assert_eq!(find(&boxes, "name").unwrap().text, "Imp 40", "and {{targeted.health}} reads the target's health");
    step(&mut w, &["hit"], &[]);
    step(&mut w, &["hit"], &[]);
    step(&mut w, &["hit"], &[]);
    step(&mut w, &["hit"], &[]);
    step(&mut w, &[], &[]);
    assert!(find(&w.ui_layout(ASPECT), "frame").is_none(), "the imp is dead: the frame goes");
}

#[test]
fn a_slot_shows_its_cooldown_as_a_share_and_as_seconds() {
    let mut w = world("");
    step(&mut w, &[], &[]);
    let ready = w.ui_layout(ASPECT);
    let slot = find(&ready, "slot1").unwrap();
    assert_eq!((slot.kind.as_str(), slot.fraction, slot.text.as_str()), ("slot", 0.0, ""), "ready: no overlay, no number");
    step(&mut w, &["power"], &[]);
    let used = w.ui_layout(ASPECT);
    let slot = find(&used, "slot1").unwrap();
    near(slot.fraction, 1.0, 0.02, "just used: the whole slot is covered");
    assert_eq!(slot.text, "2");
    for _ in 0..60 {
        step(&mut w, &[], &[]);
    }
    let half = w.ui_layout(ASPECT);
    let slot = find(&half, "slot1").unwrap();
    near(slot.fraction, 0.5, 0.05, "a second later half is left");
    assert_eq!(slot.text, "1");
    near(slot.rect[2], 0.08, 1e-9, "the slot is square");
}

#[test]
fn a_portrait_is_a_box_of_its_size() {
    let boxes = world("").ui_layout(ASPECT);
    let who = find(&boxes, "who").unwrap();
    assert_eq!(who.kind, "portrait");
    near(who.rect[2], 0.07, 1e-9, "width");
    assert_eq!(who.text, "H");
}

#[test]
fn damage_numbers_float_up_from_where_the_hit_landed() {
    let mut w = world("");
    step(&mut w, &["hit"], &CAMERA);
    let first = w.ui_layout(ASPECT);
    let number: Vec<&UiBox> = first.iter().filter(|b| b.kind == "floater").collect();
    assert_eq!(number.len(), 1, "one hit, one number");
    assert_eq!(number[0].text, "12");
    // the imp is 12 m ahead of the camera and a little below it; a point straight ahead projects to the middle of the screen
    near(number[0].rect[0] + number[0].rect[2] / 2.0, ASPECT / 2.0, 0.15, "horizontally centred (the hit was straight ahead)");
    let y0 = number[0].rect[1];
    for _ in 0..30 {
        step(&mut w, &[], &CAMERA);
    }
    let later = w.ui_layout(ASPECT);
    let rising = later.iter().find(|b| b.kind == "floater").unwrap();
    assert!(rising.rect[1] < y0 - 0.01, "it rises: {} then {}", y0, rising.rect[1]);
    for _ in 0..90 {
        step(&mut w, &[], &CAMERA);
    }
    assert!(w.ui_layout(ASPECT).iter().all(|b| b.kind != "floater"), "and is gone after a moment");
}

#[test]
fn a_number_to_the_right_appears_to_the_right_and_one_behind_the_camera_does_not_appear() {
    let mut right = world("spawn imp at=3,0,4\n");
    step(&mut right, &["hit"], &CAMERA);
    let boxes = right.ui_layout(ASPECT);
    let xs: Vec<f64> = boxes.iter().filter(|b| b.kind == "floater").map(|b| b.rect[0] + b.rect[2] / 2.0).collect();
    assert_eq!(xs.len(), 2, "both imps were hit");
    let (low, high) = (xs[0].min(xs[1]), xs[0].max(xs[1]));
    assert!(high > low + 0.05, "one number is clearly to the side of the other: {xs:?}");
    // the camera looks the other way: nothing projects in front of it
    let mut away = world("");
    let mut turned = CAMERA;
    turned[3].1 = std::f64::consts::PI;
    step(&mut away, &["hit"], &turned);
    assert!(away.ui_layout(ASPECT).iter().all(|b| b.kind != "floater"), "a point behind the camera is not drawn");
}

#[test]
fn a_hit_during_invulnerability_shows_dodge_and_healing_shows_a_plus() {
    let mut w = world("input roll key=press\nrule on=input:roll do=iframes:tag:hero:0.5\ninput smack key=press\nrule on=input:smack do=hurt:tag:hero:10\n");
    step(&mut w, &["roll"], &CAMERA);
    step(&mut w, &["smack"], &CAMERA);
    let texts: Vec<String> = w.ui_layout(ASPECT).into_iter().filter(|b| b.kind == "floater").map(|b| b.text).collect();
    assert_eq!(texts, vec!["Dodge!".to_owned()], "the blow was dodged");
    // healing needs room: hurt the hero first
    let mut hurt = world("input smack key=press\nrule on=input:smack do=hurt:tag:hero:20\n");
    step(&mut hurt, &["smack"], &CAMERA);
    step(&mut hurt, &["mend"], &CAMERA);
    let texts: Vec<String> = hurt.ui_layout(ASPECT).into_iter().filter(|b| b.kind == "floater").map(|b| b.text).collect();
    assert!(texts.contains(&"+7".to_owned()), "{texts:?}");
}

#[test]
fn the_new_elements_draw_and_round_trip() {
    let mut w = world("");
    step(&mut w, &["hit"], &CAMERA);
    let frame: serde_json::Value = serde_json::from_str(&w.ui_frame(ASPECT)).unwrap();
    let items = frame["items"].as_array().unwrap();
    assert!(items.iter().filter(|i| i["k"] == "path").count() > 10, "icons and words are strokes");
    let def = GameDef::parse(GAME).unwrap();
    assert_eq!(GameDef::parse(&def.to_text()).unwrap().to_text(), def.to_text());
}

#[test]
fn mistakes_in_the_new_elements_are_refused() {
    let refused = |extra: &str| GameDef::parse(&format!("{GAME}{extra}\n")).map_or_else(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(" | "), |_| String::new());
    assert!(refused("ui s2 type=slot size=0.08,0.08").contains("value"));
    assert!(refused("ui t type=text text=hi when=ghost.health>0").contains("ghost"));
    assert!(refused("ui t type=text text=hi when=\"1 +\"").contains("when"));
    assert!(refused("ui s3 type=slot size=0.08,0.08 value=hero.cd_smash icon=teapot").contains("icon"));
    assert_eq!(refused("ui t type=text text=hi when=targeted.health>0"), "");
}
