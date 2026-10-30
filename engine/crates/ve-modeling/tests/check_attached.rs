#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
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
        transaction: TransactionId::parse("t_check_attached").unwrap(),
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

fn scene(b_x: f64, check_params: &str) -> String {
    format!(
        "a = solid.box@1(size=[1,1,1])\nb = solid.box@1(size=[1,1,1])\nbm = xform.move@1(b, by=[{b_x},0,0])\nchk = check.attached@1(a, bm{check_params})\noutput a = a\noutput b = bm\n"
    )
}

fn floating_warnings(evaluation: &Evaluation) -> Vec<&ve_modeling::EvaluationWarning> {
    evaluation
        .warnings
        .iter()
        .filter(|warning| warning.code == "attach.floating")
        .collect()
}

#[test]
fn touching_boxes_pass() {
    let evaluation = evaluate(&scene(1.0, "")).unwrap();
    assert!(floating_warnings(&evaluation).is_empty());
}

#[test]
fn a_floating_box_warns() {
    let evaluation = evaluate(&scene(2.5, "")).unwrap();
    let warnings = floating_warnings(&evaluation);
    // Two parts 1.5 m apart both float from each other: nothing says which should move.
    assert_eq!(warnings.len(), 2);
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("`bm` floats 1.50 m from `a`"))
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("`a` floats 1.50 m from `bm`"))
    );
    assert!(warnings.iter().all(|w| w.related.len() == 1));
}

#[test]
fn severity_error_fails() {
    let error = evaluate(&scene(2.5, ", severity=\"error\"")).unwrap_err();
    assert!(error.contains("attach.floating"), "{error}");
}

#[test]
fn tolerance_decides() {
    // A gap of 0.4 m: both boxes float at the default 0.02 m tolerance.
    assert_eq!(
        floating_warnings(&evaluate(&scene(1.4, "")).unwrap()).len(),
        2
    );
    assert!(floating_warnings(&evaluate(&scene(1.4, ", tolerance=0.5")).unwrap()).is_empty());
}

#[test]
fn a_part_planted_in_another_passes() {
    let text = "s = solid.sphere@1(radius=1)\nb = solid.box@1(size=[0.5,0.5,0.5])\nbm = xform.move@1(b, by=[0.8,0,0])\nchk = check.attached@1(s, bm)\noutput s = s\noutput b = bm\n";
    assert!(floating_warnings(&evaluate(text).unwrap()).is_empty());
}

#[test]
fn three_parts_name_only_the_floating_one() {
    let text = "a = solid.box@1(size=[1,1,1])\nb = solid.box@1(size=[1,1,1])\nbm = xform.move@1(b, by=[1,0,0])\nc = solid.box@1(size=[1,1,1])\ncm = xform.move@1(c, by=[2.4,0,0])\nchk = check.attached@1(a, bm, cm)\noutput a = a\noutput b = bm\noutput c = cm\n";
    let evaluation = evaluate(text).unwrap();
    let warnings = floating_warnings(&evaluation);
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].message.contains("cm"),
        "{}",
        warnings[0].message
    );
    assert!(!warnings[0].message.contains("floats from bm"));
}

#[test]
fn needs_two_inputs() {
    let text = "a = solid.box@1(size=[1,1,1])\nchk = check.attached@1(a)\noutput a = a\n";
    let error = evaluate(text).unwrap_err();
    assert!(
        error.contains("graph.missing_input") || error.contains("parse"),
        "{error}"
    );
}
