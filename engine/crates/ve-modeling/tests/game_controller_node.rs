//! `game.controller`: the node writes the `controller` game text line, so a model gives a kind real movement with nodes alone; a mistake is
//! reported against the node that made it. (The movement itself is tested in `ve-game/tests/controller.rs`.)
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(source: &str) -> Result<String, (String, String)> {
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_controller_node").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map(|e| e.game.expect("the nodes collect into a game").describe())
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

fn game(controller: &str) -> String {
    format!(
        "hero = game.kind@1(name=\"player\", size=0.4, speed=4, tags=[\"hero\"])\nmove = game.input@1(name=\"move\", axis=\"x,z\", target=\"tag:hero\")\njump = game.input@1(name=\"jump\", key=\"press\", target=\"tag:hero\")\nctl = {controller}\nall = game.collect@1(hero, move, jump, ctl)\noutput rules = all\n"
    )
}

#[test]
fn the_node_writes_the_controller_line_with_its_numbers() {
    let text = evaluate(&game("game.controller@1(kind=\"player\", speed=6, jump=8, extra_jumps=1, relative=\"camera\", turn=540)")).unwrap();
    let line = text.lines().find(|l| l.starts_with("controller player")).unwrap_or_else(|| panic!("no controller line in:\n{text}"));
    for wanted in ["speed=6", "jump=8", "extra_jumps=1", "relative=camera", "turn=540", "mode=walk"] {
        assert!(line.contains(wanted), "{wanted} is in {line}");
    }
}

#[test]
fn a_platformer_locks_the_depth_axis_by_default_and_the_lock_can_be_set() {
    let text = evaluate(&game("game.controller@1(kind=\"player\", mode=\"platformer\", jump=9, gravity=30)")).unwrap();
    assert!(text.contains("mode=platformer") && text.contains("lock=z"), "{text}");
    let text = evaluate(&game("game.controller@1(kind=\"player\", lock=\"x\")")).unwrap();
    assert!(text.contains("lock=x"), "{text}");
}

#[test]
fn top_down_has_no_gravity_and_no_jump() {
    let text = evaluate(&game("game.controller@1(kind=\"player\", mode=\"top_down\", jump=9)")).unwrap();
    let line = text.lines().find(|l| l.starts_with("controller player")).unwrap();
    assert!(line.contains("jump=0") && line.contains("gravity=0"), "{line}");
}

#[test]
fn a_controller_for_a_kind_that_does_not_exist_is_reported_against_its_node() {
    let (code, message) = evaluate(&game("game.controller@1(kind=\"ghost\", speed=5)")).unwrap_err();
    assert_eq!(code, "game.controller", "{message}");
    assert!(message.contains("ghost"), "{message}");
}

fn with_camera(camera: &str) -> String {
    format!("hero = game.kind@1(name=\"player\", size=0.4, tags=[\"hero\"])\ncam = {camera}\nall = game.collect@1(hero, cam)\noutput rules = all\n")
}

#[test]
fn the_camera_node_writes_the_camera_line_and_leaves_automatic_values_to_the_mode() {
    let text = evaluate(&with_camera("game.camera@1(name=\"main\", mode=\"third\", shoulder=0.5, smoothing=0.2)")).unwrap();
    let line = text.lines().find(|l| l.starts_with("camera main")).unwrap_or_else(|| panic!("no camera line in:\n{text}"));
    for wanted in ["mode=third", "shoulder=0.5", "smoothing=0.2", "distance=4.5", "height=1.5", "target=tag:hero"] {
        assert!(line.contains(wanted), "{wanted} (a mode default where the node left it automatic) is in {line}");
    }
    let iso = evaluate(&with_camera("game.camera@1(name=\"view\", mode=\"iso\")")).unwrap();
    let line = iso.lines().find(|l| l.starts_with("camera view")).unwrap();
    assert!(line.contains("yaw=45") && line.contains("pitch=-35") && line.contains("distance=16"), "{line}");
}

#[test]
fn a_fixed_camera_needs_a_place_and_bounds_are_checked() {
    let text = evaluate(&with_camera("game.camera@1(name=\"cctv\", mode=\"fixed\", at=\"0, 5, -10\")")).unwrap();
    assert!(text.contains("at=0,5,-10"), "{text}");
    let (code, message) = evaluate(&with_camera("game.camera@1(name=\"cctv\", mode=\"fixed\")")).unwrap_err();
    assert_eq!(code, "game.camera", "{message}");
    let (code, message) = evaluate(&with_camera("game.camera@1(name=\"side\", mode=\"side\", bounds=\"1,2,3\")")).unwrap_err();
    assert_eq!(code, "game.camera", "{message}");
    assert!(message.contains("bounds"), "{message}");
}

#[test]
fn the_input_node_writes_its_bindings_and_a_wrong_key_count_is_reported_against_the_node() {
    let text = evaluate(&game("game.controller@1(kind=\"player\")").replace(
        "move = game.input@1(name=\"move\", axis=\"x,z\", target=\"tag:hero\")",
        "move = game.input@1(name=\"move\", axis=\"x,z\", target=\"tag:hero\", keys=\"KeyD, KeyA, KeyW, KeyS\", pad=\"stick:left\", label=\"Move around\")",
    ))
    .unwrap();
    let line = text.lines().find(|l| l.starts_with("input move")).unwrap();
    assert!(line.contains("keys=KeyD,KeyA,KeyW,KeyS") && line.contains("pad=stick:left") && line.contains("label=Move_around"), "{line}");
    let (code, message) = evaluate(&game("game.controller@1(kind=\"player\")").replace(
        "target=\"tag:hero\")\njump",
        "target=\"tag:hero\", keys=\"KeyD\")\njump",
    ))
    .unwrap_err();
    assert_eq!(code, "game.input", "{message}");
    assert!(message.contains("keys"), "{message}");
}

#[test]
fn level_and_state_nodes_write_the_flow_and_a_missing_level_is_reported() {
    let source = "hero = game.kind@1(name=\"player\", size=0.4, tags=[\"hero\"])\nrock = game.kind@1(name=\"rock\", size=0.5)\nh = game.spawn@1(kind=\"player\", at=[0, 0, 0])\nr = game.spawn@1(kind=\"rock\", at=[3, 0, 3], level=\"cave\")\nlv = game.level@1(name=\"cave\", start=[1, 0, 2], place_heroes=true)\ns = game.state@1(name=\"title\", frozen=true)\nplay = game.state@1(name=\"playing\")\ngo = game.rule@1(on=\"start\", state=\"playing\", do=\"level:cave\")\nall = game.collect@1(hero, rock, h, r, lv, s, play, go)\noutput rules = all\n";
    let text = evaluate(source).unwrap();
    for wanted in ["state title frozen=1", "state playing", "level cave start=1,0,2", "spawn rock at=3,0,3", "level=cave", "state=playing", "do=level:cave"] {
        assert!(text.contains(wanted), "{wanted} is in:\n{text}");
    }
    let (code, message) = evaluate(&source.replace("lv = game.level@1(name=\"cave\", start=[1, 0, 2], place_heroes=true)\n", "").replace("lv, ", "")).unwrap_err();
    assert!(code.starts_with("game."), "{code}: {message}");
    assert!(message.contains("cave"), "{message}");
}

#[test]
fn the_ui_node_writes_a_menu_that_the_game_lays_out_and_a_bad_parent_is_reported_against_the_node() {
    let source = "hero = game.kind@1(name=\"player\", size=0.4, tags=[\"hero\"])\nh = game.spawn@1(kind=\"player\", at=[0, 0, 0])\ns1 = game.state@1(name=\"title\", frozen=true)\ns2 = game.state@1(name=\"playing\")\nscore = game.var@1(name=\"score\", value=0)\nmenu = game.ui@1(name=\"menu\", type=\"stack\", anchor=\"c\", fill=\"#202830\", gap=0.02, show=\"title\")\nplay = game.ui@1(name=\"play\", type=\"button\", parent=\"menu\", text=\"Play now\", do=\"go:playing\", tip=\"Start the game\")\nhud = game.ui@1(name=\"hud\", type=\"text\", anchor=\"tr\", text=\"Score {score}\", show=\"playing\")\nall = game.collect@1(hero, h, s1, s2, score, menu, play, hud)\noutput rules = all\n";
    let text = evaluate(source).unwrap();
    for wanted in ["ui menu type=stack", "anchor=c", "fill=\"#202830\"", "gap=0.02", "show=title", "ui play type=button", "parent=menu", "text=\"Play now\"", "do=go:playing", "ui hud type=text", "text=\"Score {score}\""] {
        assert!(text.contains(wanted), "{wanted} is in:\n{text}");
    }
    let game = ve_game::GameDef::parse(&text).unwrap();
    let boxes = ve_game::World::new(&game, 1).ui_layout(2.0);
    assert_eq!(boxes.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["menu", "play", "tooltip"], "the title shows the menu with its button and the tip of the focused one");
    let (code, message) = evaluate(&source.replace("parent=\"menu\"", "parent=\"nowhere\"")).unwrap_err();
    assert_eq!(code, "game.ui", "{message}");
    assert!(message.contains("nowhere"), "{message}");
}

#[test]
fn a_large_game_can_be_gathered_in_groups_whose_names_are_checked_on_the_whole() {
    // the screens name a state that the outer group declares: each group is read alone, the whole is checked once
    let screens = "hero = game.kind@1(name=\"player\", size=0.4, tags=[\"hero\"])\nh = game.spawn@1(kind=\"player\", at=[0, 0, 0])\ns1 = game.state@1(name=\"title\", frozen=true)\ns2 = game.state@1(name=\"playing\")\nmenu = game.ui@1(name=\"menu\", type=\"stack\", anchor=\"c\", show=\"title\")\nplay = game.ui@1(name=\"play\", type=\"button\", parent=\"menu\", text=\"Play\", do=\"go:playing\")\nui_group = game.collect@1(menu, play, name=\"screens\")\nall = game.collect@1(hero, h, s1, s2, ui_group)\noutput rules = all\n";
    let text = evaluate(screens).unwrap();
    assert!(text.contains("ui menu type=stack") && text.contains("state title frozen=1"), "{text}");
    // and a name that nobody declares is still caught, once, against the node that wrote it
    let (code, message) = evaluate(&screens.replace("s1 = game.state@1(name=\"title\", frozen=true)\n", "").replace("hero, h, s1, s2,", "hero, h, s2,")).unwrap_err();
    assert!(code.starts_with("game."), "{code}: {message}");
    assert!(message.contains("title"), "{message}");
}

#[test]
fn two_controllers_for_one_kind_are_refused() {
    let source = "hero = game.kind@1(name=\"player\", size=0.4, tags=[\"hero\"])\na = game.controller@1(kind=\"player\")\nb = game.controller@1(kind=\"player\", speed=7)\nall = game.collect@1(hero, a, b)\noutput rules = all\n";
    let (code, message) = evaluate(source).unwrap_err();
    assert_eq!(code, "game.duplicate", "{message}");
    assert!(message.contains("player"), "the kind is named: {message}");
}
