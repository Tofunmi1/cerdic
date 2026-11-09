//! The top-down room (`web/tests/scenes/toproom/toproom.txt`): a bot that steers toward each gem in turn, with the view looking straight
//! down (yaw 0), collects all four around the pillars and walls and wins; the hero never leaves the room and never jumps.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{Inputs, Status, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn scene_world() -> World {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/tests/scenes/toproom/toproom.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(8);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_toproom").unwrap(),
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

fn hero(world: &World) -> [f64; 3] {
    world.entities().iter().find(|e| e.kind == "player").expect("the hero exists").position
}

/// Steers toward `goal` as a player would with the view looking along +z (the stick's right is -x).
fn steer(world: &mut World, goal: [f64; 2], seconds: f64) {
    for _ in 0..(seconds * 60.0) as usize {
        let at = hero(world);
        let (dx, dz) = (goal[0] - at[0], goal[1] - at[2]);
        if dx.hypot(dz) < 0.2 {
            return;
        }
        let mut inputs = Inputs::default();
        inputs.axes.insert("camera.yaw".into(), 0.0);
        inputs.axes.insert("move.x".into(), (-dx).clamp(-1.0, 1.0));
        inputs.axes.insert("move.z".into(), dz.clamp(-1.0, 1.0));
        world.step(&inputs).unwrap();
    }
}

#[test]
fn a_bot_collects_the_four_gems_around_the_pillars_and_wins() {
    let mut world = scene_world();
    // waypoints keep clear of the pillars at (-3,2), (3,-2) and (0,4.5)
    let route: [[f64; 2]; 10] =
        [[-5.0, 0.0], [-7.0, 5.0], [-5.0, 0.0], [0.0, 0.0], [5.0, 0.0], [7.0, 5.0], [6.0, 0.0], [7.0, -5.0], [0.0, -4.0], [-7.0, -5.0]];
    for waypoint in route {
        steer(&mut world, waypoint, 6.0);
        assert!(hero(&world)[1].abs() < 1e-9, "never leaves the floor: {:?}", hero(&world));
        let at = hero(&world);
        assert!(at[0].abs() < 9.0 && at[2].abs() < 7.0, "inside the room: {at:?}");
    }
    let left: Vec<[f64; 3]> = world.entities().iter().filter(|e| e.kind == "gem").map(|e| e.position).collect();
    assert_eq!(world.status(), Status::Won, "ended at {:?} with {:?} gems; left: {left:?}", hero(&world), world.vars().get("gems"));
}

#[test]
fn walls_and_pillars_stop_the_hero() {
    let mut world = scene_world();
    steer(&mut world, [-20.0, 0.0], 8.0); // toward the west wall, far past it
    let at = hero(&world);
    assert!(at[0] > -8.5, "stopped by the west wall (its face is at x = -8.7, radius 0.3): {at:?}");
}
