//! Input-dependent lake defaults and graph round trips.
#![expect(clippy::unwrap_used, reason = "valid, bounded scene fixtures")]
use ve_graph::{
    CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text, to_text,
};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn scene(source: &str) -> (Evaluation, String) {
    let registry = builtin_registry();
    let commands = parse_text(&registry, source).unwrap().commands;
    let mut session = Session::with_builtin(15);
    assert!(matches!(
        session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_lake_defaults").unwrap(),
            label: None,
            commands,
        }),
        Outcome::Committed { .. }
    ));
    let printed = to_text(&registry, session.document()).unwrap();
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    (result, printed)
}

#[test]
fn omitted_seed_uses_the_visible_tile_centre_in_world_coordinates_and_round_trips() {
    let source = "t = terrain.tile(size=[32, 32], tile_x=2, tile_z=-1, cell=0.5, height=0)\n\
                  p = t | terrain.flatten(at=[80, -16], radius=3, blend=3, to=\"height\", height=-1)\n\
                  w = p | water.fill(level=-0.3)\noutput w\n";
    let (result, printed) = scene(source);
    let (again, _) = scene(&printed);
    let part = &result.parts[0];
    assert_eq!(part.positions, again.parts[0].positions);
    assert_eq!(part.colors, again.parts[0].colors);
    assert!(
        part.positions
            .iter()
            .all(|p| (p[0] - 80.0).abs() < 7.0 && (p[2] + 16.0).abs() < 7.0)
    );
    let look = part
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert_eq!(look.flow.to_bits(), 0.0_f32.to_bits());
}

#[test]
fn explicit_origin_seed_is_preserved_instead_of_using_the_centre() {
    let source = "t = terrain.tile(size=[32, 32], cell=1, height=0)\n\
                  p = t | terrain.flatten(at=[0, 0], radius=3, blend=3, to=\"height\", height=-1)\n\
                  w = p | water.fill(at=[0, 0], level=-0.3)\noutput w\n";
    let (result, printed) = scene(source);
    assert!(printed.contains("at=[0, 0]"));
    let (again, _) = scene(&printed);
    assert_eq!(result.parts[0].positions, again.parts[0].positions);
    assert!(
        result.parts[0]
            .positions
            .iter()
            .all(|p| p[0] < 7.0 && p[2] < 7.0)
    );
}

#[test]
fn invalid_level_and_shore_diagnostics_name_the_parameter() {
    for (argument, name) in [
        ("level=1e20", "level"),
        ("shore=0", "shore"),
        ("shore=51", "shore"),
    ] {
        let source = format!("t = terrain.tile()\nw = water.fill(t, {argument})\noutput w");
        let errors = parse_text(&builtin_registry(), &source).unwrap_err();
        assert!(
            errors.iter().any(|e| e.param.as_deref() == Some(name)),
            "{errors:?}"
        );
    }
}
