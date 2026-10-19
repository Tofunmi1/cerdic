//! Polygon refusals tell the caller what to change: every code in the table has a suggestion, and real refusals carry one.
#![expect(
    clippy::unwrap_used,
    reason = "the helper unwraps fixtures that are known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

/// The refusal an evaluation produces, with its code and suggestion.
fn refusal(text: &str) -> (String, Option<String>) {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_suggestions").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .err()
        .unwrap();
    (error.code.clone(), error.suggestion.clone())
}

const BOX: &str = "box = solid.box@1(size=[1, 1, 1])\n\
    triangles = poly.from_solid@1(box)\n\
    mesh = poly.merge_coplanar@1(triangles)\n";

#[test]
fn an_empty_selection_suggests_loosening_the_rule() {
    let (code, suggestion) = refusal(&format!(
        "{BOX}none = poly.select@1(mesh, element=\"edge\", by=\"height\", axis=\"y\", height=9, tolerance=0.01)\n\
         b = poly.bevel_network@1(mesh, none, width=0.1, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    assert_eq!(code, "selection.lost");
    assert!(suggestion.unwrap().contains("loosen"));
}

#[test]
fn an_ambiguous_selection_suggests_narrowing() {
    let (code, suggestion) = refusal(&format!(
        "{BOX}top = poly.select@1(mesh, element=\"edge\", by=\"extreme\", axis=\"y\", side=\"max\", expect=\"one\")\n\
         b = poly.bevel_network@1(mesh, top, width=0.1, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    assert_eq!(code, "selection.ambiguous");
    assert!(suggestion.unwrap().contains("narrow"));
}

#[test]
fn a_wrong_selection_kind_names_the_parameter_to_set() {
    let (code, suggestion) = refusal(&format!(
        "{BOX}faces = poly.select@1(mesh, by=\"normal\", direction=[0, 1, 0], angle=0.05)\n\
         b = poly.bevel_network@1(mesh, faces, width=0.1, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    assert_eq!(code, "poly.selection_kind");
    assert!(suggestion.unwrap().contains("element"));
}

#[test]
fn a_rule_that_does_not_fit_the_element_lists_what_does() {
    let (code, suggestion) = refusal(&format!(
        "{BOX}bad = poly.select@1(mesh, element=\"face\", by=\"sharp\")\n\
         up = poly.extrude_faces@1(mesh, bad, offset=[0, 0.5, 0])\n\
         solid = poly.to_solid@1(up)\n\
         output solid"
    ));
    assert_eq!(code, "poly.select_rule_unsupported");
    assert!(suggestion.unwrap().contains("faces take"));
}

#[test]
fn every_table_code_has_a_suggestion_and_unknown_codes_have_none() {
    for code in [
        "poly.nonmanifold",
        "poly.face_degenerate",
        "poly.winding_conflict",
        "poly.selection_empty",
        "poly.loop_invalid",
        "poly.boundary_edge",
        "poly.attribute_conflict",
        "poly.weld_collapses_face",
        "poly.budget_exceeded",
        "selection.lost",
        "selection.ambiguous",
        "selection.stale",
    ] {
        assert!(
            ve_modeling::poly_suggestion(code).is_some(),
            "no suggestion for {code}"
        );
    }
    assert!(ve_modeling::poly_suggestion("kernel.not_a_code").is_none());
}
