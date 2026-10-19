//! G3-01 to G3-06: the text form reads to the same document as typed commands, prints
//! deterministically, and reports problems with names and source spans.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "case helpers unwrap fixtures that are known to be valid and fail loudly otherwise"
)]
use ve_graph::{
    CommandBatch, Document, OpId, Outcome, Session, TransactionId, builtin_registry, parse_text,
    to_text,
};

const VASE: &str = "\
outer = profile.polygon@1(points=[[0, 0], [0.06, 0], [0.11, 0.12], [0.07, 0.26], [0.05, 0.3], [0, 0.3]])
inner = profile.polygon@1(points=[[0, 0.01], [0.056, 0.01], [0.104, 0.12], [0.064, 0.26], [0.04, 0.31], [0, 0.31]])
wall = check.min_wall@1(outer, inner, min=3mm)
body = solid.revolve@1(outer, angle=360deg, segments=64)
cavity = solid.revolve@1(inner, angle=360deg)
vase_solid = solid.difference@1(body, cavity)
output vase = vase_solid
";

fn apply(text: &str) -> Result<Document, Vec<ve_graph::Diagnostic>> {
    let registry = builtin_registry();
    let batch = parse_text(&registry, text)?;
    let mut session = Session::with_builtin(5);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_text").unwrap(),
        label: None,
        commands: batch.commands.clone(),
    });
    match outcome {
        Outcome::Committed { .. } => Ok(session.document().clone()),
        Outcome::Rejected { diagnostics } => {
            Err(diagnostics.into_iter().map(|d| batch.locate(d)).collect())
        }
        other => panic!("{other:?}"),
    }
}

fn first_error(text: &str) -> ve_graph::Diagnostic {
    apply(text).unwrap_err().remove(0)
}

#[test]
fn g3_01_text_builds_the_document_the_commands_build() {
    let doc = apply(VASE).unwrap();
    assert_eq!(doc.nodes.len(), 6);
    assert_eq!(doc.outputs.len(), 1);
    assert_eq!(doc.outputs[0].name, "vase");
    let labels: Vec<_> = doc
        .nodes
        .iter()
        .filter_map(|n| n.label.as_deref())
        .collect();
    assert_eq!(
        labels,
        ["outer", "inner", "wall", "body", "cavity", "vase_solid"]
    );
    let angle = doc.nodes[3].params["angle"].as_f64().unwrap();
    assert!(
        (angle - std::f64::consts::TAU).abs() < 1e-12,
        "360deg is stored as radians"
    );
    assert!(
        (doc.nodes[2].params["min"].as_f64().unwrap() - 0.003).abs() < 1e-15,
        "3mm is stored in metres"
    );
    let ids: Vec<_> = doc.nodes.iter().map(|n| n.id.to_string()).collect();
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        "IDs are unique"
    );
}

#[test]
fn g3_02_canonical_text_is_stable_and_round_trips() {
    let registry = builtin_registry();
    let doc = apply(VASE).unwrap();
    let once = to_text(&registry, &doc).unwrap();
    let twice = to_text(&registry, &doc).unwrap();
    assert_eq!(once, twice);
    let again = apply(&once).unwrap();
    assert_eq!(
        to_text(&registry, &again).unwrap(),
        once,
        "text -> document -> text is a fixed point"
    );
    assert_eq!(
        doc.canonical_json(),
        again.canonical_json(),
        "same nodes, links, params and IDs"
    );
    assert!(
        !once.contains("angle="),
        "parameters equal to their default are left out:\n{once}"
    );
    let turned = apply("p = profile.polygon@1(points=[[0.1, 0], [0.2, 0], [0.2, 0.1]])\nr = solid.revolve@1(p, angle=90deg)\n").unwrap();
    assert!(
        to_text(&registry, &turned)
            .unwrap()
            .contains("angle=1.5707963267948966rad"),
        "angles print exactly, in radians"
    );
}

#[test]
fn g3_02b_flags_named_inputs_and_blank_lines_survive_the_round_trip() {
    let registry = builtin_registry();
    let text = "\n# a comment\na = solid.box@1(size=[1, 2, 3])\nb = solid.sphere@1(radius=5cm, segments=12, muted=true)\n\nc = solid.union(a, b)\noutput c\n";
    let doc = apply(text).unwrap();
    assert!(doc.nodes[1].flags.muted);
    let printed = to_text(&registry, &doc).unwrap();
    assert!(printed.contains("solid.union@1(a, b)"), "{printed}");
    assert_eq!(
        apply(&printed).unwrap().canonical_json(),
        doc.canonical_json()
    );
    let named = apply("o = profile.polygon@1(points=[[0,0],[1,0],[0,1]])\ni = profile.polygon@1(points=[[0,0],[0.5,0],[0,0.5]])\nw = check.min_wall(inner=i, outer=o, min=1mm)\n").unwrap();
    assert_eq!(named.nodes[2].inputs.len(), 2);
}

#[test]
fn choice_parameters_read_print_and_refuse_other_words() {
    let registry = builtin_registry();
    let doc = apply("p = profile.polygon@1(points=[[0,0],[1,0],[0,1]])\no = profile.offset@1(p, delta=1cm, join=\"round\")\nm = profile.offset@1(p, delta=1cm)\n").unwrap();
    let printed = to_text(&registry, &doc).unwrap();
    assert!(printed.contains("join=\"round\""), "{printed}");
    assert_eq!(
        printed.matches("join=").count(),
        1,
        "the default miter is left out:\n{printed}"
    );
    assert_eq!(
        apply(&printed).unwrap().canonical_json(),
        doc.canonical_json()
    );
    let error = first_error(
        "p = profile.polygon@1(points=[[0,0],[1,0],[0,1]])\no = profile.offset@1(p, delta=1cm, join=\"bevel\")\n",
    );
    assert_eq!(error.code, "param.range");
    assert!(error.message.contains("miter"), "{}", error.message);
    assert_eq!(
        first_error(
            "p = profile.polygon@1(points=[[0,0],[1,0],[0,1]])\no = profile.offset@1(p, delta=1cm, join=3)\n"
        )
        .code,
        "param.type"
    );
}

/// Applies `text` on top of `session`'s document through `parse_text_in`.
fn extend(session: &mut Session, text: &str) -> Result<(), Vec<ve_graph::Diagnostic>> {
    let batch = ve_graph::parse_text_in(&builtin_registry(), session.document(), text)?;
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(&format!("t_more{}", session.document().revision))
            .unwrap(),
        label: None,
        commands: batch.commands.clone(),
    });
    match outcome {
        Outcome::Committed { .. } => Ok(()),
        Outcome::Rejected { diagnostics } => {
            Err(diagnostics.into_iter().map(|d| batch.locate(d)).collect())
        }
        other => panic!("{other:?}"),
    }
}

fn vase_session() -> Session {
    let registry = builtin_registry();
    let mut session = Session::with_builtin(5);
    let batch = parse_text(&registry, VASE).unwrap();
    session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_vase").unwrap(),
        label: None,
        commands: batch.commands,
    });
    session
}

#[test]
fn text_can_build_on_nodes_already_in_the_document() {
    let registry = builtin_registry();
    let mut session = vase_session();
    let old_ids: Vec<_> = session
        .document()
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .collect();
    extend(&mut session, "red = solid.material@1(vase_solid, color=[0.85, 0.08, 0.08], roughness=0.3)\noutput vase = red\n").unwrap();
    let doc = session.document();
    assert_eq!(doc.nodes.len(), 7);
    assert_eq!(
        &doc.nodes
            .iter()
            .map(|n| n.id.clone())
            .take(6)
            .collect::<Vec<_>>(),
        &old_ids,
        "existing IDs are kept"
    );
    assert_eq!(
        doc.outputs.len(),
        1,
        "the output is redirected, not duplicated"
    );
    assert_eq!(doc.outputs[0].from.node, doc.nodes[6].id);
    let printed = to_text(&registry, doc).unwrap();
    assert!(
        printed.contains(
            "red = solid.material@1(vase_solid, color=[0.85, 0.08, 0.08], roughness=0.3)"
        ),
        "{printed}"
    );
    assert!(printed.contains("output vase = red"), "{printed}");
    assert!(
        apply(&printed).unwrap().same_content(doc),
        "and it still reads back as one program"
    );
}

#[test]
fn text_on_a_document_refuses_reused_and_unknown_names() {
    let mut session = vase_session();
    let error = extend(&mut session, "outer = solid.box@1(size=[1, 1, 1])\n")
        .unwrap_err()
        .remove(0);
    assert_eq!(error.code, "parse.duplicate_name");
    let error = extend(&mut session, "x = solid.material@1(nothing_here)\n")
        .unwrap_err()
        .remove(0);
    assert_eq!(error.code, "parse.unknown_name");
    assert_eq!(session.document().nodes.len(), 6, "nothing was added");
}

#[test]
fn g3_03_unknown_operator_suggests_the_closest_name() {
    let error = first_error("a = solid.revolv@1(x)\n");
    assert_eq!(error.code, "parse.unknown_op");
    assert!(error.suggestion.unwrap().contains("solid.revolve@1"));
    assert_eq!(error.span.map(|s| (s.line, s.col, s.len)), Some((1, 5, 14)));
}

#[test]
fn g3_04_missing_and_unknown_parameters_name_type_and_unit() {
    let error = first_error("a = solid.cylinder@1(radius=1)\n");
    assert_eq!(error.code, "parse.missing_param");
    assert!(
        error.message.contains("height") && error.message.contains("metres"),
        "{}",
        error.message
    );
    let error = first_error("a = solid.cylinder@1(radius=1, height=2, colour=3)\n");
    assert_eq!(error.code, "parse.unknown_param");
    assert!(error.suggestion.unwrap().contains("radius"));
    let error = first_error("a = solid.cylinder@1(radius=1deg, height=2)\n");
    assert_eq!(error.code, "param.type");
    let error = first_error("a = solid.cylinder@1(radius=-1, height=2)\n");
    assert_eq!(error.code, "param.range");
    assert_eq!(error.span.map(|s| s.line), Some(1));
}

#[test]
fn g3_05_a_wrong_port_type_is_refused_with_its_source_line() {
    let error = first_error(
        "p = profile.polygon@1(points=[[0,0],[1,0],[0,1]])\nb = solid.box@1(size=[1,1,1])\nu = solid.union@1(p, b)\n",
    );
    assert_eq!(error.code, "graph.port_type");
    assert_eq!(error.span.map(|s| s.line), Some(3));
}

#[test]
fn g3_06_forward_references_and_cycles_are_rejected_with_node_names() {
    let error = first_error("a = xform.move@1(b, by=[1,0,0])\nb = solid.box@1(size=[1,1,1])\n");
    assert_eq!(error.code, "parse.forward_reference");
    assert!(error.message.contains('a') && error.message.contains('b'));
    let error = first_error("a = xform.move@1(b, by=[1,0,0])\nb = xform.move@1(a, by=[1,0,0])\n");
    assert_eq!(error.code, "graph.cycle");
    assert!(error.message.contains("a reads b"), "{}", error.message);
    assert_eq!(
        first_error("a = xform.move@1(a, by=[1,0,0])\n").code,
        "graph.cycle"
    );
    assert_eq!(
        first_error("a = xform.move@1(zzz, by=[1,0,0])\n").code,
        "parse.unknown_name"
    );
}

#[test]
fn syntax_errors_say_what_was_expected_and_where() {
    for (text, line) in [
        ("a = \n", 1),
        ("a solid.box@1()\n", 1),
        ("ok = solid.box@1(size=[1,1,1])\nb = (\n", 2),
        ("a = solid.box@1(size=[1,1,1]) extra\n", 1),
    ] {
        let error = first_error(text);
        assert_eq!(error.code, "parse.syntax", "{text}");
        assert_eq!(error.span.map(|s| s.line), Some(line), "{text}");
    }
    assert_eq!(
        first_error("a = solid.box@1(size=[1,1,1])\na = solid.box@1(size=[1,1,1])\n").code,
        "parse.duplicate_name"
    );
}

#[test]
fn shipped_field_operators_are_ready_for_text_graphs() {
    let registry = builtin_registry();
    for name in ["field.plane@1", "field.ellipsoid@1", "field.tube@1"] {
        let id = OpId::parse(name).unwrap();
        assert_eq!(
            registry.get(&id).unwrap().readiness,
            ve_graph::Readiness::Ready
        );
    }
}
