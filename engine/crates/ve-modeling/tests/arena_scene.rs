//! The arena scene (`web/tests/scenes/arena/arena.txt`): it evaluates with its health, projectile and weapon nodes, a hero that holds the
//! fire key kills all four turrets (the blaster aims at the nearest) and wins while the turrets hurt it on the way; one that does nothing
//! is shot to pieces; and the screens show the hero's health and ammo from entity props.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{Inputs, Status, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

const ASPECT: f64 = 16.0 / 9.0;

fn scene_world() -> World {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/tests/scenes/arena/arena.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(11);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_arena").unwrap(),
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

fn start(world: &mut World) {
    world.step(&Inputs { pressed: vec!["ui_accept".into()], ..Inputs::default() }).unwrap();
    assert_eq!(world.flow_state(), "playing");
}

fn hero_health(world: &World) -> f64 {
    world.entities().iter().find(|e| e.kind == "player").and_then(|e| e.props.get("health").copied()).unwrap_or(0.0)
}

fn turrets(world: &World) -> usize {
    world.entities().iter().filter(|e| e.kind == "turret").count()
}

#[test]
fn a_hero_holding_fire_clears_the_arena_and_wins() {
    let mut world = scene_world();
    start(&mut world);
    let mut reloading_until = 0;
    for step in 0..60 * 40 {
        let mut inputs = Inputs::default();
        inputs.axes.insert("fire".into(), 1.0);
        // reload when the magazine runs dry, as a player would
        let ammo = world.entities().iter().find(|e| e.kind == "player").and_then(|e| e.props.get("ammo_blaster").copied()).unwrap_or(16.0);
        if ammo < 1.0 && step >= reloading_until {
            inputs.pressed.push("reload".into());
            reloading_until = step + 80;
        }
        world.step(&inputs).unwrap();
        if world.status() != Status::Playing {
            break;
        }
    }
    assert_eq!(world.status(), Status::Won, "turrets left: {}, hero health {}", turrets(&world), hero_health(&world));
    assert_eq!(world.flow_state(), "win");
    assert_eq!(world.vars().get("kills").copied(), Some(4.0));
    assert!(hero_health(&world) > 0.0, "and the hero lived");
}

#[test]
fn a_hero_that_only_stands_there_is_shot_down() {
    let mut world = scene_world();
    start(&mut world);
    for _ in 0..60 * 120 {
        world.step(&Inputs::default()).unwrap();
        if world.status() != Status::Playing {
            break;
        }
    }
    assert_eq!((world.status(), world.flow_state()), (Status::Lost, "game_over"));
    assert_eq!(turrets(&world), 4, "it never fired");
}

#[test]
fn the_hud_shows_health_and_ammo_read_from_the_hero() {
    let mut world = scene_world();
    start(&mut world);
    let boxes = world.ui_layout(ASPECT);
    let by = |id: &str| boxes.iter().find(|b| b.id == id).unwrap();
    assert!((by("h_hp").fraction - 1.0).abs() < 1e-9, "full health");
    assert_eq!(by("h_ammo").text, "Ammo 16 / 16");
    world.step(&Inputs { axes: [("fire".to_owned(), 1.0)].into(), ..Inputs::default() }).unwrap();
    assert_eq!(by_text(&world, "h_ammo"), "Ammo 15 / 16", "one round gone");
}

fn by_text(world: &World, id: &str) -> String {
    world.ui_layout(ASPECT).into_iter().find(|b| b.id == id).map(|b| b.text).unwrap()
}
