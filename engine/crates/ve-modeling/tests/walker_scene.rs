//! The walker scene (`web/tests/scenes/walker/walker.txt`) must evaluate, and a bot that steers like a player (move axes relative to the
//! camera yaw, a jump press) must get its hero onto the first ledge and the one after it: the controller, the solids and the nodes work
//! together, not just in their own tests.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{Inputs, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn scene_world() -> World {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/tests/scenes/walker/walker.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(5);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_walker").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "apply: {applied:?}");
    let evaluated = match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(ev) => ev,
        Err(e) => panic!("evaluate: {} {}", e.code, e.message),
    };
    assert!(evaluated.warnings.is_empty(), "no warnings: {:?}", evaluated.warnings);
    let game = evaluated.game.expect("the scene is a game");
    assert_eq!(game.cameras.len(), 3, "behind, above and through its eyes");
    assert_eq!(game.controllers.len(), 1, "the hero has a controller");
    World::new(&game, 1)
}

fn hero(world: &World) -> [f64; 3] {
    world.entities().iter().find(|e| e.kind == "player").expect("the hero exists").position
}

/// Walks toward `goal` on the ground plane as a player would with the camera looking along +z, and jumps when close.
fn walk_to(world: &mut World, goal: [f64; 3], jump_near: f64, seconds: f64) {
    let steps = (seconds * 60.0) as usize;
    for step in 0..steps {
        let at = hero(world);
        let (dx, dz) = (goal[0] - at[0], goal[2] - at[2]);
        let mut input = Inputs::default();
        input.axes.insert("camera.yaw".into(), 0.0);
        // facing +z the right-hand side is -x
        input.axes.insert("move.x".into(), (-dx).clamp(-1.0, 1.0));
        input.axes.insert("move.z".into(), dz.clamp(-1.0, 1.0));
        // a jump every 80 steps near the goal: pressed on the first step and held (the axis) through the next 30, like a thumb on the key
        if dx.hypot(dz) < jump_near {
            match step % 80 {
                0 => input.pressed.push("jump".into()),
                1..30 => {
                    input.axes.insert("jump".into(), 1.0);
                }
                _ => {}
            }
        }
        world.step(&input).unwrap();
    }
}

#[test]
fn a_bot_climbs_the_ledges_of_the_walker_scene() {
    let mut world = scene_world();
    walk_to(&mut world, [4.0, 0.0, 3.0], 2.6, 6.0);
    let on_first = hero(&world);
    assert!((on_first[1] - 1.0).abs() < 0.05, "standing on the first ledge (top at 1 m): {on_first:?}");
    walk_to(&mut world, [6.4, 0.0, 3.0], 2.6, 4.0);
    let on_second = hero(&world);
    assert!((on_second[1] - 1.5).abs() < 0.05, "and then on the second (top at 1.5 m): {on_second:?}");
}

#[test]
fn walking_into_the_tall_stack_without_jumping_is_blocked() {
    let mut world = scene_world();
    // the stack at (-5, -4) is two ledges high: with no jump the hero stays on the ground in front of it
    let mut input = Inputs::default();
    input.axes.insert("camera.yaw".into(), 0.0);
    input.axes.insert("move.x".into(), 1.0); // the right of +z is -x
    input.axes.insert("move.z".into(), -0.8);
    for _ in 0..600 {
        world.step(&input).unwrap();
    }
    assert!(hero(&world)[1] < 0.05, "still on the ground: {:?}", hero(&world));
}
