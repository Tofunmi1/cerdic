//! `rooms.plan`: a building made from a room layout: floors, walls on every region edge and door openings, no roof.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?.commands;
    let mut session = Session::with_builtin(31);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_rooms").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default().evaluate(session.document(), Quality::Final).map_err(|e| format!("{e:?}"))
}

fn plan(params: &str) -> Result<Evaluation, String> {
    evaluate(&format!("b = rooms.plan@1({params})\noutput b = b\n"))
}

const CUSTOMS: &str = "rooms=\"hall:6,6,8,8; office:3,3,4,4; store:3,3,5,5; scales:3,3,4,4\", links=\"0-1; 0-2; 2-3\"";

#[test]
fn the_building_stands_on_the_floor_inside_its_footprint_with_walls_of_the_given_height() {
    let ev = plan(&format!("{CUSTOMS}, seed=2, wall_height=2.8")).unwrap();
    let part = &ev.parts[0];
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &part.positions {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    assert!((hi[1] - 2.8).abs() < 1e-3 && lo[1] > -0.11, "walls 2.8 m, floor 0.1 m thick: {} .. {}", lo[1], hi[1]);
    assert!(lo[0] > -0.2 && lo[2] > -0.2, "the plan starts at the origin: {:?}", lo);
    assert!(part.volume > 5.0, "{}", part.volume);
}

#[test]
fn the_same_seed_repeats_and_another_seed_differs() {
    let a = plan(&format!("{CUSTOMS}, seed=1")).unwrap();
    let b = plan(&format!("{CUSTOMS}, seed=1")).unwrap();
    let c = plan(&format!("{CUSTOMS}, seed=5")).unwrap();
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
    assert_ne!(a.parts[0].positions, c.parts[0].positions);
}

#[test]
fn doors_are_cut_so_a_wider_door_leaves_less_wall() {
    let narrow = plan(&format!("{CUSTOMS}, seed=1, door_width=0.7")).unwrap().parts[0].volume;
    let wide = plan(&format!("{CUSTOMS}, seed=1, door_width=2.0")).unwrap().parts[0].volume;
    assert!(wide < narrow, "wider doors remove more wall: {wide} against {narrow}");
}

#[test]
fn bad_plans_are_refused_with_a_reason() {
    assert!(plan("rooms=\"a:3,3,4,4; b:3,3,4,4\", links=\"0-5\"").is_err(), "a link to a room that does not exist");
    assert!(plan("rooms=\"a:3,3,4,4; b:3,3,4,4\", links=\"\"").is_err(), "rooms that no link connects");
    assert!(plan("rooms=\"a:3,3\", links=\"\"").is_err(), "a room needs four numbers");
    assert!(plan("rooms=\"a:3,3,4,4\", links=\"x\"").is_err(), "a link needs two numbers");
}

#[test]
fn one_room_is_a_box_of_walls_with_no_door() {
    let ev = plan("rooms=\"cell:4,4,4,4\", links=\"\"").unwrap();
    assert!(ev.parts[0].volume > 1.0);
}
