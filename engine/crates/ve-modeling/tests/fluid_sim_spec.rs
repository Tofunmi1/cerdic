//! `fluid.sim`: the description of a live liquid that reaches the runtime, and the surface it starts with. Do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_fluid").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

const TANK: &str = "water = fluid.sim@1(min=[0, 0, 0], max=[0.4, 0.5, 0.4], spacing=0.05, bounds_min=[0, 0, 0], bounds_max=[1, 1, 0.4], max_triangles=3000)\noutput water = water\n";

#[test]
fn the_part_carries_the_liquid_description_and_a_padded_surface() {
    let result = evaluate(TANK).unwrap_or_else(|e| panic!("{e:?}"));
    let part = result.parts.iter().find(|p| p.name == "water").unwrap();
    let spec = part.fluid.as_ref().expect("the part is a liquid");
    assert_eq!(spec.spacing, 0.05);
    assert_eq!(spec.bounds_max, [1.0, 1.0, 0.4]);
    assert_eq!(spec.gravity, [0.0, -9.81, 0.0]);
    assert_eq!(
        part.indices.len(),
        3000 * 3,
        "padded to max_triangles so the runtime's chunks have room"
    );
    // The real surface wraps the block (0 to 0.4, 0 to 0.5, 0 to 0.4) a little outside its particle centres.
    let top = part.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!(top > 0.47 && top < 0.58, "the rest surface's top: {top}");
}

#[test]
fn a_block_with_too_many_particles_is_refused_with_the_fix() {
    let error = evaluate("water = fluid.sim@1(min=[0, 0, 0], max=[10, 10, 10], spacing=0.05)\noutput water = water\n").err().unwrap();
    assert_eq!(error.0, "budget.fluid");
}

#[test]
fn a_surface_with_more_triangles_than_max_triangles_is_refused() {
    let error = evaluate("water = fluid.sim@1(min=[0, 0, 0], max=[0.4, 0.5, 0.4], spacing=0.05, max_triangles=200)\noutput water = water\n").err().unwrap();
    assert_eq!(error.0, "budget.fluid");
    assert!(error.1.contains("max_triangles"), "{}", error.1);
}

#[test]
fn collider_bones_resolve_to_segments_and_others_warn() {
    let program = "box = solid.box@1(size=[0.05, 0.4, 0.05])\npaddle_at = xform.move@1(box, by=[0.2, 0.6, 0.2])\npaddle = rig.bone@1(paddle_at, head=[0.2, 0.8, 0.2], tail=[0.2, 0.4, 0.2])\noutput paddle = paddle\n";
    let result = evaluate(&format!("{program}water = fluid.sim@1(min=[0, 0, 0], max=[0.4, 0.3, 0.4], spacing=0.05, max_triangles=2000, collide_bones=\"paddle, water\")\noutput water = water\n")).unwrap();
    let spec = result
        .parts
        .iter()
        .find(|p| p.name == "water")
        .unwrap()
        .fluid
        .clone()
        .unwrap();
    assert_eq!(spec.collide_bones, vec!["paddle".to_owned()]);
    assert_eq!(spec.segments[0].head, [0.2, 0.8, 0.2]);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.code == "fluid.collide_bone" && w.message.contains("water")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn a_material_put_on_the_liquid_keeps_it_a_liquid() {
    let result = evaluate(&format!("{TANK}blue = solid.material@1(water, color=[0.1, 0.3, 0.9], roughness=0.1)\noutput sea = blue\n")).unwrap();
    assert!(
        result
            .parts
            .iter()
            .find(|p| p.name == "sea")
            .unwrap()
            .fluid
            .is_some()
    );
}

#[test]
fn the_same_liquid_always_starts_as_the_same_surface() {
    let a = evaluate(TANK).unwrap();
    let b = evaluate(TANK).unwrap();
    let positions = |r: &Evaluation| {
        r.parts
            .iter()
            .find(|p| p.name == "water")
            .unwrap()
            .positions
            .clone()
    };
    assert_eq!(positions(&a), positions(&b));
}
