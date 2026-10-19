//! G7 subdivision through the registered graph operator.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

#[test]
fn g7_subdivide_is_registered_and_the_output_can_become_a_solid() {
    let text = "box = solid.box@1(size=[1, 1, 1])\n\
        cage = poly.from_solid@1(box)\n\
        edges = poly.select@1(cage, element=\"edge\", by=\"sharp\", angle=1.0)\n\
        points = poly.select@1(cage, element=\"vertex\")\n\
        smooth = poly.subdivide@1(cage, creases=edges, corners=points, levels=1, sharpness=2, corner_sharpness=2)\n\
        result = poly.to_solid@1(smooth)\n\
        output result";
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g7_graph").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let evaluated = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert_eq!(evaluated.parts.len(), 1);
    assert!(evaluated.parts[0].volume > 0.0);
    assert_eq!(evaluated.parts[0].indices.len() / 3, 72);
}

#[test]
fn subdivision_is_a_terminal_step_for_v1_planar_patch_and_bevel_edits() {
    for levels in [1, 2] {
        let extrude = format!(
            "box = solid.box@1(size=[2, 2, 2])\n\
             triangles = poly.from_solid@1(box)\n\
             cage = poly.merge_coplanar@1(triangles)\n\
             smooth = poly.subdivide@1(cage, levels={levels})\n\
             top = poly.select@1(smooth, element=\"face\", by=\"box\", box_min=[-1, 0.3, -1], box_max=[1, 1, 1])\n\
             raised = poly.extrude_faces@1(smooth, top, offset=[0, 0.1, 0])\n\
             output raised"
        );
        let bevel = format!(
            "box = solid.box@1(size=[2, 2, 2])\n\
             triangles = poly.from_solid@1(box)\n\
             cage = poly.merge_coplanar@1(triangles)\n\
             smooth = poly.subdivide@1(cage, levels={levels})\n\
             edges = poly.select@1(smooth, element=\"edge\", by=\"sharp\", angle=0.3)\n\
             rounded = poly.bevel_edges@1(smooth, edges, width=0.02, segments=1)\n\
             output rounded"
        );
        for (program, expected) in [
            (extrude.as_str(), "poly.patch_nonplanar"),
            (bevel.as_str(), "poly.bevel_network_unsupported"),
        ] {
            let batch = parse_text(&builtin_registry(), program).unwrap();
            let mut session = Session::with_builtin(2);
            let applied = session.apply(&CommandBatch {
                doc: session.document().id.clone(),
                base_revision: 0,
                transaction: TransactionId::parse("t_g7_terminal_edit").unwrap(),
                label: None,
                commands: batch.commands,
            });
            assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
            let result = Evaluator::default().evaluate(session.document(), Quality::Final);
            assert_eq!(
                result.unwrap_err().code,
                expected,
                "level {levels}: {program}"
            );
        }
    }
}

#[test]
fn g7_subdivision_graph_survives_document_save_and_reload() {
    let source = "box = solid.box@1(size=[1, 1, 1])\n\
        cage = poly.from_solid@1(box)\n\
        smooth = poly.subdivide@1(cage, levels=2)\n\
        output cage\n\
        output smooth";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(2);
    let result = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g7_reload").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(result, Outcome::Committed { .. }), "{result:?}");
    let saved = session.document().canonical_json();
    let document: ve_graph::Document = serde_json::from_str(&saved).unwrap();
    assert_eq!(document.canonical_json(), saved);
    let loaded = Session::new(builtin_registry(), document);
    let output = Evaluator::default()
        .evaluate(loaded.document(), Quality::Final)
        .unwrap();
    assert_eq!(output.parts.len(), 2);
    assert!(output.parts.iter().any(|part| part.indices.len() == 864));
    assert!(output.parts.iter().any(|part| part.indices.len() == 36));
    let undo = session.undo();
    assert!(undo.is_some(), "subdivision graph transaction is undoable");
    assert!(
        Evaluator::default()
            .evaluate(session.document(), Quality::Final)
            .unwrap()
            .parts
            .is_empty()
    );
    assert!(
        session.redo().is_some(),
        "subdivision graph transaction is redoable"
    );
    let redone = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert!(redone.parts.iter().any(|part| part.indices.len() == 864));
    assert!(redone.parts.iter().any(|part| part.indices.len() == 36));
}
