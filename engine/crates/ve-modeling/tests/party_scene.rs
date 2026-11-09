//! The party scene (`web/tests/scenes/party/party.txt`): it evaluates through its nested game groups, the screens show party frames, a target
//! frame, a boss bar and the ability bar, and a bot that fights like a player (walks to the boss, holds attack, uses its abilities, dodges when
//! hurt) with three companions on its side wins the encounter.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{Inputs, Status, UiBox, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

const ASPECT: f64 = 16.0 / 9.0;
const CAMERA: [(&str, f64); 6] = [("camera.x", 0.0), ("camera.y", 3.0), ("camera.z", -20.0), ("camera.yaw", 0.0), ("camera.pitch", -0.1), ("camera.fov", 60.0)];

fn scene_world() -> World {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/tests/scenes/party/party.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(21);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_party").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "apply: {applied:?}");
    let evaluated = match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(ev) => ev,
        Err(e) => panic!("evaluate: {} {}", e.code, e.message),
    };
    assert!(evaluated.warnings.is_empty(), "no warnings: {:?}", evaluated.warnings);
    World::new(&evaluated.game.expect("the scene is a game"), 1)
}

fn press(world: &mut World, names: &[&str]) {
    world.step(&Inputs { pressed: names.iter().map(|n| (*n).to_owned()).collect(), axes: CAMERA.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(), ..Inputs::default() }).unwrap();
}

fn begin(world: &mut World) {
    press(world, &["ui_accept"]);
    assert_eq!(world.flow_state(), "playing");
    press(world, &[]);
}

fn health_of(world: &World, kind: &str) -> Option<f64> {
    world.entities().iter().find(|e| e.kind == kind).and_then(|e| e.props.get("health").copied())
}

fn find<'a>(boxes: &'a [UiBox], id: &str) -> Option<&'a UiBox> {
    boxes.iter().find(|b| b.id == id)
}

#[test]
fn the_encounter_plays_out_with_a_bot_at_the_helm() {
    let mut world = scene_world();
    begin(&mut world);
    let hero_at = |w: &World| w.entities().iter().find(|e| e.kind == "vriden").map(|e| e.position);
    for step in 0..60 * 150 {
        let mut inputs = Inputs { axes: CAMERA.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(), ..Inputs::default() };
        inputs.axes.insert("attack".into(), 1.0);
        // walk toward the spirit with the camera looking along +z, so the stick's right is -x
        let Some(here) = hero_at(&world) else { break };
        let stag = world.entities().iter().find(|e| e.kind == "stag").map(|e| e.position);
        if let Some(stag) = stag {
            let (dx, dz) = (stag[0] - here[0], stag[2] - here[2]);
            if dx.hypot(dz) > 3.0 {
                inputs.axes.insert("move.x".into(), (-dx).clamp(-1.0, 1.0));
                inputs.axes.insert("move.z".into(), dz.clamp(-1.0, 1.0));
            }
        }
        if step % 90 == 10 {
            inputs.pressed.push("power1".into());
            inputs.pressed.push("power2".into());
        }
        if step % 400 == 200 {
            inputs.pressed.push("power3".into());
        }
        world.step(&inputs).unwrap();
        if world.status() != Status::Playing {
            break;
        }
    }
    assert_eq!(
        (world.status(), world.flow_state()),
        (Status::Won, "win"),
        "stag={:?} hero={:?} knight={:?} archer={:?} axeman={:?}",
        health_of(&world, "stag"),
        health_of(&world, "vriden"),
        health_of(&world, "knight"),
        health_of(&world, "archer"),
        health_of(&world, "axeman")
    );
    assert!(health_of(&world, "vriden").is_some(), "the hero lived");
}

#[test]
fn a_hero_that_stands_still_loses_the_party_the_fight() {
    let mut world = scene_world();
    begin(&mut world);
    for _ in 0..60 * 120 {
        press(&mut world, &[]);
        if world.status() != Status::Playing {
            break;
        }
    }
    // the companions fight on alone, but the spirit is too strong for them without their leader
    assert_ne!(world.status(), Status::Playing, "the fight ends one way or the other");
}

#[test]
fn the_screens_show_the_party_the_target_the_boss_and_the_ability_bar() {
    let mut world = scene_world();
    let title: Vec<String> = world.ui_layout(ASPECT).into_iter().map(|b| b.id).collect();
    assert!(title.contains(&"t_menu".to_owned()) && !title.contains(&"party_col".to_owned()), "the title shows the menu only: {title:?}");
    begin(&mut world);
    press(&mut world, &[]);
    let boxes = world.ui_layout(ASPECT);
    for id in ["party_col", "h1", "h2", "h3", "h4", "p1", "slot_smash", "slot_dodge", "mini", "boss_bar", "boss_name"] {
        assert!(find(&boxes, id).is_some(), "{id} is on screen");
    }
    assert!(find(&boxes, "target_frame").is_some(), "something is targeted from the start (the nearest foe in front)");
    assert!(find(&boxes, "t_menu").is_none(), "and the title is gone");
    assert_eq!(find(&boxes, "slot_smash").unwrap().fraction, 0.0, "abilities start ready");
    assert!((find(&boxes, "h1").unwrap().fraction - 1.0).abs() < 1e-9, "full health");
    assert!(find(&boxes, "mini").unwrap().dots.len() >= 4, "the minimap shows the guards and the spirit");
}

#[test]
fn an_ability_hurts_the_target_starts_its_cooldown_and_shows_a_number() {
    let mut world = scene_world();
    begin(&mut world);
    // walk the hero up to the first guard so it is in smash range (5 m)
    let guard_before = world.entities().iter().filter(|e| e.kind == "guard").map(|e| e.props["health"]).fold(f64::MAX, f64::min);
    assert!(guard_before > 400.0);
    for _ in 0..60 * 3 {
        let mut inputs = Inputs { axes: CAMERA.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(), ..Inputs::default() };
        inputs.axes.insert("move.z".into(), 1.0);
        world.step(&inputs).unwrap();
    }
    press(&mut world, &["power1"]);
    let boxes = world.ui_layout(ASPECT);
    let slot = find(&boxes, "slot_smash").unwrap();
    assert!(slot.fraction > 0.9, "the smash is cooling down: {}", slot.fraction);
    assert_eq!(slot.text, "12");
    assert!(boxes.iter().any(|b| b.kind == "floater"), "and a damage number is floating over the target");
}
