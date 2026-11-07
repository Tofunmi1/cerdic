//! Field expressions remain typed values until an explicit bounded meshing node.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

#[test]
fn field_sphere_meshes_and_can_feed_solid_boolean() {
    let source = "field = field.sphere@1(radius=0.75)\n\
        solid = field.to_solid@1(field, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.1)\n\
        box = solid.box@1(size=[0.5, 0.5, 0.5])\n\
        result = solid.union@1(solid, box)\n\
        output result";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_field_graph").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert_eq!(result.parts.len(), 1);
    assert!(
        (result.parts[0].volume - 4.0 * std::f64::consts::PI * 0.75_f64.powi(3) / 3.0).abs() < 0.04
    );
}

#[test]
fn core_field_operators_are_available_through_the_graph() {
    let programs = [
        "plane = field.plane@1(normal=[0, 1, 0], offset=0)\n\
         sphere = field.sphere@1(radius=0.5)\n\
         result = field.to_solid@1(sphere, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
        "shape = field.ellipsoid@1(radii=[0.6, 0.5, 0.4])\n\
         result = field.to_solid@1(shape, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
        "shape = field.box@1(half=[0.5, 0.5, 0.5], rounding=0.1)\n\
         result = field.to_solid@1(shape, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
        "shape = field.torus@1(major=0.5, tube=0.15)\n\
         result = field.to_solid@1(shape, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
        "shape = field.capsule@1(radius=0.2, half_length=0.4)\n\
         result = field.to_solid@1(shape, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
        "shape = field.cylinder@1(radius=0.3, half_height=0.4)\n\
         moved = field.move@1(shape, by=[0.1, 0, 0])\n\
         scaled = field.scale@1(moved, factor=1.1)\n\
         mirrored = field.mirror@1(scaled, axes=[1, 0, 0])\n\
         offset = field.offset@1(mirrored, amount=0.05)\n\
         shell = field.shell@1(offset, half_width=0.08)\n\
         result = field.to_solid@1(shell, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
        "shape = field.tube@1(points=[[-0.5, 0, 0], [0, 0.3, 0], [0.5, 0, 0]], radius=0.12)\n\
         result = field.to_solid@1(shape, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.16)\n\
         output result",
        "shape = field.sphere@1(radius=0.2)\n\
         repeated = field.repeat@1(shape, period=[0.8, 1, 1], count=[1, 0, 0])\n\
         result = field.to_solid@1(repeated, min=[-1.3, -0.8, -0.8], max=[1.3, 0.8, 0.8], edge_length=0.16)\n\
         output result",
        "a = field.sphere@1(radius=0.4)\n\
         b = field.sphere@1(radius=0.2)\n\
         joined = field.smooth_union@1(a, b, radius=0.1)\n\
         cut = field.subtract@1(joined, b)\n\
         result = field.to_solid@1(cut, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.2)\n\
         output result",
    ];
    for (index, source) in programs.iter().enumerate() {
        let commands = parse_text(&builtin_registry(), source).unwrap().commands;
        let mut session = Session::with_builtin(15);
        let applied = session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse(&format!("t_g8_core_{index}")).unwrap(),
            label: None,
            commands,
        });
        assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
        let result = Evaluator::default()
            .evaluate(session.document(), Quality::Final)
            .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message));
        assert_eq!(result.parts.len(), 1, "fixture {index}");
        assert!(!result.parts[0].indices.is_empty(), "fixture {index}");
    }
}

#[test]
fn field_graph_reports_clipping_and_type_checks_field_sockets() {
    let source = "field = field.sphere@1(radius=0.75)\n\
        solid = field.to_solid@1(field, min=[-0.5, -0.5, -0.5], max=[0.5, 0.5, 0.5], edge_length=0.1)\n\
        output solid";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_clipped_graph").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }));
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "field.clipped");

    let mismatch = "field = field.sphere@1(radius=0.75)\n\
        result = solid.union@1(field, field)\n\
        output result";
    let commands = parse_text(&builtin_registry(), mismatch).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let rejected = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_type_mismatch").unwrap(),
        label: None,
        commands,
    });
    let Outcome::Rejected { diagnostics } = rejected else {
        panic!("field-to-solid link unexpectedly committed: {rejected:?}");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "graph.port_type")
    );
}

#[test]
fn field_grid_caps_are_refused_during_plan_preflight() {
    let source = "field = field.sphere@1(radius=0.5)\n\
        result = field.to_solid@1(field, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.008)\n\
        output result";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_plan_cells").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }));
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "budget.field_cells");
    let preview = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    assert_eq!(preview.parts.len(), 1);
}

#[test]
fn preview_uses_coarser_field_spacing_and_tracks_final_shape() {
    let source = "field = field.sphere@1(radius=0.5)\n\
        result = field.to_solid@1(field, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.1)\n\
        output result";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_preview_grid").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }));
    let final_result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    let preview_result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    let exact = 4.0 * std::f64::consts::PI * 0.5_f64.powi(3) / 3.0;
    assert!((preview_result.parts[0].volume - exact).abs() < 0.08);
    assert!((preview_result.parts[0].volume - final_result.parts[0].volume).abs() < 0.08);
}

#[test]
fn inexact_offset_and_shell_report_distance_warnings_but_exact_offset_does_not() {
    let source = "sphere = field.sphere@1(radius=0.4)\n\
        moved = field.move@1(sphere, by=[0.3, 0, 0])\n\
        blend = field.smooth_union@1(sphere, moved, radius=0.1)\n\
        offset = field.offset@1(blend, amount=0.04)\n\
        shell = field.shell@1(offset, half_width=0.08)\n\
        mesh = field.to_solid@1(shell, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.12)\n\
        output mesh";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_inexact_warning").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }));
    let offset_id = session
        .document()
        .nodes
        .iter()
        .find(|node| node.label.as_deref() == Some("offset"))
        .unwrap()
        .id
        .to_string();
    let shell_id = session
        .document()
        .nodes
        .iter()
        .find(|node| node.label.as_deref() == Some("shell"))
        .unwrap()
        .id
        .to_string();
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    let warnings: Vec<_> = evaluation
        .warnings
        .iter()
        .filter(|warning| warning.code == "field.distance_inexact")
        .collect();
    assert_eq!(warnings.len(), 2);
    assert!(warnings.iter().any(|warning| warning.node == offset_id));
    assert!(warnings.iter().any(|warning| warning.node == shell_id));

    let exact = "sphere = field.sphere@1(radius=0.4)\n\
        offset = field.offset@1(sphere, amount=0.04)\n\
        mesh = field.to_solid@1(offset, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.12)\n\
        output mesh";
    let commands = parse_text(&builtin_registry(), exact).unwrap().commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_exact_offset").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }));
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert!(
        evaluation
            .warnings
            .iter()
            .all(|warning| warning.code != "field.distance_inexact")
    );
}
