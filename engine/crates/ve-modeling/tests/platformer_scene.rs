//! The platformer scene (`web/tests/scenes/platformer/platformer.txt`): a bot that runs right and jumps at every platform edge collects the
//! three coins and wins; one that never jumps falls into the pit, is put back on safe ground and the game counts the fall.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{Inputs, Status, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn scene_world() -> World {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/tests/scenes/platformer/platformer.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(6);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_platformer").unwrap(),
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

/// The side camera looks along -z (yaw 180 degrees), so the stick's right is +x.
fn input(right: f64, jump_step: Option<usize>) -> Inputs {
    let mut inputs = Inputs::default();
    inputs.axes.insert("camera.yaw".into(), std::f64::consts::PI);
    inputs.axes.insert("move.x".into(), right);
    match jump_step {
        Some(0) => inputs.pressed.push("jump".into()),
        Some(1..35) => {
            inputs.axes.insert("jump".into(), 1.0);
        }
        _ => {}
    }
    inputs
}

fn shown(world: &World) -> Vec<String> {
    world.ui_layout(16.0 / 9.0).into_iter().map(|b| b.id).collect()
}

#[test]
fn each_screen_shows_its_own_ui_and_the_buttons_drive_the_flow() {
    let mut world = scene_world();
    assert_eq!(shown(&world), ["t_menu", "t_name", "t_play", "t_help", "tooltip"], "the title: a menu with one button, whose tip shows");
    let help = world.ui_layout(16.0 / 9.0).into_iter().find(|b| b.id == "t_help").unwrap();
    assert!(help.text.contains("W A S D") || help.text.contains("move"), "the help names the controls: {}", help.text);
    world.set_ui_labels([("move".to_owned(), "A D".to_owned()), ("jump".to_owned(), "Space".to_owned()), ("view".to_owned(), "V".to_owned())].into());
    let help = world.ui_layout(16.0 / 9.0).into_iter().find(|b| b.id == "t_help").unwrap();
    assert_eq!(help.text, "A D to run   Space to jump (again in the air)   V changes the view");
    tap(&mut world, "ui_accept");
    assert_eq!(world.flow_state(), "playing");
    assert_eq!(shown(&world), ["h_coins", "h_lives"]);
    let hud = world.ui_layout(16.0 / 9.0);
    assert_eq!(hud[0].text, "Coins 0 / 3");
    assert!((hud[1].fraction - 1.0).abs() < 1e-9, "three of three lives");
    tap(&mut world, "pause");
    assert_eq!(shown(&world), ["p_menu", "p_name", "p_resume", "p_quit"]);
    tap(&mut world, "ui_next");
    tap(&mut world, "ui_accept");
    assert_eq!(world.flow_state(), "title", "Quit to title restarts the game at its title");
    assert_eq!(world.vars().get("lives").copied(), Some(3.0));
}

#[test]
fn game_over_and_win_offer_a_button_that_starts_a_fresh_game() {
    let mut lost = scene_world();
    tap(&mut lost, "start");
    for _ in 0..60 * 60 {
        lost.step(&input(1.0, None)).unwrap();
        if lost.status() != Status::Playing {
            break;
        }
    }
    assert_eq!(shown(&lost), ["o_menu", "o_name", "o_again"]);
    tap(&mut lost, "ui_accept");
    assert_eq!((lost.flow_state(), lost.status()), ("playing", Status::Playing), "Try again goes straight back into the game");
    assert_eq!(lost.vars().get("lives").copied(), Some(3.0));
    let mut won = scene_world();
    tap(&mut won, "start");
    play_to_the_end(&mut won);
    assert_eq!(shown(&won), ["w_menu", "w_name", "w_falls", "w_again"]);
    assert_eq!(won.ui_layout(16.0 / 9.0).iter().find(|b| b.id == "w_falls").map(|b| b.text.as_str()), Some("Falls: 0"));
    tap(&mut won, "ui_accept");
    assert_eq!(won.flow_state(), "playing");
    assert_eq!(won.vars().get("score").copied(), Some(0.0));
}

/// Presses a key input once, as the page does on the key's rise.
fn tap(world: &mut World, name: &str) {
    let mut inputs = input(0.0, None);
    inputs.pressed.push(name.to_owned());
    world.step(&inputs).unwrap();
}

#[test]
fn the_game_waits_on_its_title_until_start_and_pauses_and_resumes() {
    let mut world = scene_world();
    assert_eq!(world.flow_state(), "title");
    let still = world.hash();
    for _ in 0..120 {
        world.step(&input(1.0, None)).unwrap();
    }
    assert_eq!(world.hash(), still, "nothing happens behind the title");
    tap(&mut world, "start");
    assert_eq!(world.flow_state(), "playing");
    for _ in 0..30 {
        world.step(&input(1.0, None)).unwrap();
    }
    tap(&mut world, "pause");
    assert_eq!(world.flow_state(), "paused");
    let paused = world.hash();
    for _ in 0..120 {
        world.step(&input(1.0, None)).unwrap();
    }
    assert_eq!(world.hash(), paused);
    tap(&mut world, "pause");
    assert_eq!(world.flow_state(), "playing");
}

#[test]
fn winning_shows_the_win_screen_and_start_returns_to_the_title_with_a_fresh_game() {
    let mut world = scene_world();
    tap(&mut world, "start");
    play_to_the_end(&mut world);
    assert_eq!((world.status(), world.flow_state()), (Status::Won, "win"));
    tap(&mut world, "start");
    assert_eq!(world.flow_state(), "title");
    assert_eq!(world.status(), Status::Playing);
    assert_eq!(world.vars().get("score").copied(), Some(0.0));
    assert_eq!(world.vars().get("lives").copied(), Some(3.0));
    assert_eq!(world.entities().iter().filter(|e| e.kind == "coin").count(), 3, "the coins are back");
}

fn play_to_the_end(world: &mut World) {
    let edges = [3.0, 9.0, 15.0, 21.0];
    let mut jumping_since: Option<usize> = None;
    for step in 0..60 * 30 {
        let at = hero(world);
        let near_edge = edges.iter().any(|edge| (edge - at[0]).abs() < 0.45);
        if jumping_since.is_none() && near_edge && world.entities().iter().any(|e| e.kind == "player" && e.props.get("grounded") == Some(&1.0)) {
            jumping_since = Some(step);
        }
        // a jump is a press, then the key held for 35 steps; a new one may start 15 steps after it ends
        let held = jumping_since.map(|since| step - since).filter(|n| *n < 35);
        if jumping_since.is_some_and(|since| step - since >= 50) {
            jumping_since = None;
        }
        world.step(&input(1.0, held)).unwrap();
        if world.status() != Status::Playing {
            break;
        }
    }
}

#[test]
fn a_bot_that_jumps_at_every_edge_collects_the_coins_and_wins() {
    let mut world = scene_world();
    tap(&mut world, "start");
    let edges = [3.0, 9.0, 15.0, 21.0];
    let mut jumping_since: Option<usize> = None;
    for step in 0..60 * 30 {
        let at = hero(&world);
        let near_edge = edges.iter().any(|edge| (edge - at[0]).abs() < 0.45);
        if jumping_since.is_none() && near_edge && world.entities().iter().any(|e| e.kind == "player" && e.props.get("grounded") == Some(&1.0)) {
            jumping_since = Some(step);
        }
        // a jump is a press, then the key held for 35 steps; a new one may start 15 steps after it ends
        let held = jumping_since.map(|since| step - since).filter(|n| *n < 35);
        if jumping_since.is_some_and(|since| step - since >= 50) {
            jumping_since = None;
        }
        world.step(&input(1.0, held)).unwrap();
        if world.status() == Status::Won {
            break;
        }
    }
    assert_eq!(world.status(), Status::Won, "the bot won; it ended at {:?} with score {:?}", hero(&world), world.vars().get("score"));
    assert_eq!(world.vars().get("score").copied(), Some(3.0));
    assert_eq!(world.vars().get("falls").copied(), Some(0.0), "without falling");
}

#[test]
fn running_off_the_first_platform_falls_and_is_put_back_and_three_falls_end_the_game() {
    let mut world = scene_world();
    tap(&mut world, "start");
    for _ in 0..60 * 8 {
        world.step(&input(1.0, None)).unwrap();
        if world.vars().get("falls").copied().unwrap_or(0.0) > 0.0 {
            break;
        }
    }
    assert_eq!(world.vars().get("falls").copied(), Some(1.0), "the fall was counted");
    assert_eq!(world.vars().get("lives").copied(), Some(2.0), "and cost a life");
    assert!(hero(&world)[1] > -1.0, "the hero is back on the platform: {:?}", hero(&world));
    for _ in 0..60 * 60 {
        world.step(&input(1.0, None)).unwrap();
        if world.status() != Status::Playing {
            break;
        }
    }
    assert_eq!((world.status(), world.flow_state()), (Status::Lost, "game_over"));
    assert_eq!(world.vars().get("lives").copied(), Some(0.0));
}
