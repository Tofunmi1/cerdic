//! Hands as nodes: `game.kind(grab=true)` makes a kind grabbable and `game.input(follow="right_hand")` binds an entity to a controller, and
//! the joined game plays through `ve-game`'s hands. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_game::{HandInput, HandSide, Inputs, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g24_hands").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

const NODES: &str = "\
seated = game.var@1(name=\"seated\", value=0)
glove = game.kind@1(name=\"glove\", size=0.08, tags=[\"hand_r\"])
fuse = game.kind@1(name=\"fuse\", size=0.1, tags=[\"item\"], grab=true)
socket = game.kind@1(name=\"socket\", size=0.12, tags=[\"port\"])
g0 = game.spawn@1(kind=\"glove\", at=[0, 0, 0])
f0 = game.spawn@1(kind=\"fuse\", at=[0, 1, 0.5])
s0 = game.spawn@1(kind=\"socket\", at=[-2, 1, 0])
hand = game.input@1(name=\"rh\", follow=\"right_hand\", target=\"tag:hand_r\")
seat = game.rule@1(on=\"touch\", a=\"kind:fuse\", b=\"kind:socket\", do=\"add:seated:1;destroy:a\")
done = game.goal@1(outcome=\"win\", when=\"seated>=1\")
all = game.collect@1(seated, glove, fuse, socket, g0, f0, s0, hand, seat, done, name=\"fuses\")
output rules = all
";

#[test]
fn the_nodes_make_a_game_with_a_grabbable_fuse_and_a_hand_that_follows_a_controller() {
    let result = evaluate(NODES).unwrap();
    let game = result.game.as_ref().expect("the program defines a game");
    let text = game.to_text();
    assert!(
        text.contains("kind fuse size=0.1 speed=0 tags=item grab=1"),
        "{text}"
    );
    assert!(text.contains("follow=right_hand"), "{text}");
    assert!(
        !text.contains("tags=hand_r grab"),
        "a kind that is not grabbable says nothing about grabbing: {text}"
    );
}

#[test]
fn that_game_can_be_won_by_carrying_the_fuse_to_the_socket_with_a_hand() {
    let result = evaluate(NODES).unwrap();
    let mut world = World::new(result.game.as_ref().unwrap(), 1);
    let hand = |position: [f64; 3], grip: bool| Inputs {
        hands: vec![HandInput {
            side: HandSide::Right,
            position,
            grip,
            trigger: false,
        }],
        ..Inputs::default()
    };
    world.step(&hand([0.0, 1.0, 0.4], false)).unwrap();
    world.step(&hand([0.0, 1.0, 0.4], true)).unwrap();
    assert_eq!(world.vars()["seated"], 0.0);
    let outcome = world.step(&hand([-1.9, 1.0, -0.1], true)).unwrap();
    assert_eq!(world.vars()["seated"], 1.0);
    assert_eq!(outcome.status, ve_game::Status::Won);
}

#[test]
fn an_unknown_hand_is_refused_with_the_choices() {
    let error = evaluate(&NODES.replace("right_hand", "third_hand")).unwrap_err();
    assert!(
        error.contains("right_hand"),
        "the message lists the choices: {error}"
    );
}
