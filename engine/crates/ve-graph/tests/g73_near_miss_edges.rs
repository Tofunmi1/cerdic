//! Text coercion boundaries and name suggestions beyond the near-miss acceptance cases.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers require valid fixtures and fail loudly otherwise"
)]

use ve_graph::{
    CommandBatch, Diagnostic, OpId, OperatorRegistry, Outcome, ParamType, Readiness, Session,
    TextBatch, TransactionId, Value, builtin_registry, parse_text, parse_text_in, to_text,
};

fn error(text: &str, code: &str) -> Diagnostic {
    parse_text(&builtin_registry(), text)
        .unwrap_err()
        .into_iter()
        .find(|d| d.code == code)
        .unwrap()
}

fn hint(text: &str, code: &str) -> String {
    error(text, code).suggestion.unwrap_or_default()
}

fn apply(session: &mut Session, batch: TextBatch) {
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse("t_edges").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
}

#[test]
fn broadcasts_vec2_units_and_keeps_canonical_text_and_typed_values_strict() {
    let registry = builtin_registry();
    let batch = parse_text(&registry, "a = profile.rect(size=50cm)\noutput o = a").unwrap();
    let mut session = Session::with_builtin(5);
    apply(&mut session, batch);
    let text = to_text(&registry, session.document()).unwrap();
    assert!(text.contains("size=[0.5, 0.5]"), "{text}");
    assert!(
        ParamType::Vec2 {
            unit: ve_graph::Unit::Length,
            min: 0.0,
            max: 1.0,
        }
        .check(&Value::Num(0.5))
        .is_err()
    );
    assert!(
        ParamType::Choice {
            options: &["x", "y", "z"]
        }
        .check(&Value::Str("X".to_owned()))
        .is_err()
    );
    for (source, components) in [
        ("a = profile.rect(size=[1])", '2'),
        ("a = solid.box(size=[1])", '3'),
    ] {
        let e = error(source, "param.type");
        assert!(e.message.contains(components), "{e:?}");
    }
    assert_eq!(
        error("a = profile.rect(size=-1cm)", "param.range")
            .param
            .as_deref(),
        Some("size")
    );
    assert_eq!(
        error("a = solid.box(size=1000000000000)", "param.range")
            .param
            .as_deref(),
        Some("size")
    );
}

#[test]
fn port_and_parameter_hints_preserve_the_existing_choices() {
    let h = hint("a = solid.box(sizes=1)", "parse.unknown_param");
    assert!(h.starts_with("Did you mean `size`?"), "{h}");
    assert!(h.contains("takes: size"), "{h}");
    let h = hint(
        "a = solid.box(size=1)\nb = xform.move(inn=a)",
        "graph.unknown_port",
    );
    assert!(h.starts_with("Did you mean `in`?"), "{h}");
    assert!(h.contains("takes: in"), "{h}");
    for (text, code) in [
        ("a = solid.box(zzzzzz=1)", "parse.unknown_param"),
        (
            "a = solid.box(size=1)\nb = xform.move(zzzzzz=a)",
            "graph.unknown_port",
        ),
        ("a = zzzzzzzzzzzzzzzzzzzzzz.missing()", "parse.unknown_op"),
    ] {
        let h = hint(text, code);
        assert!(!h.contains("Did you mean"), "{h}");
    }
}

#[test]
fn suggests_only_preceding_labels_and_labels_in_the_document() {
    let h = hint(
        "post = solid.box(size=1)\nb = xform.move(psot)\npsot_later = solid.box(size=1)",
        "parse.unknown_name",
    );
    assert_eq!(h, "Did you mean `post`?");
    let h = hint(
        "b = xform.move(psot)\npost = solid.box(size=1)",
        "parse.unknown_name",
    );
    assert!(h.is_empty(), "{h}");
    let registry = builtin_registry();
    let mut session = Session::with_builtin(5);
    apply(
        &mut session,
        parse_text(&registry, "post = solid.box(size=1)").unwrap(),
    );
    let e = parse_text_in(&registry, session.document(), "b = xform.move(psot, by=0)").unwrap_err();
    assert_eq!(e[0].suggestion.as_deref(), Some("Did you mean `post`?"));
}

#[test]
fn operator_hints_are_family_qualified_ready_and_limited() {
    let builtin = builtin_registry();
    let template = builtin.get(&OpId::parse("solid.box@1").unwrap()).unwrap();
    let mut registry = OperatorRegistry::new();
    for (id, readiness) in [
        ("solid.box@1", Readiness::Ready),
        ("solid.bx@1", Readiness::Draft),
    ] {
        let mut spec = template.clone();
        spec.id = OpId::parse(id).unwrap();
        spec.readiness = readiness;
        registry.register(spec).unwrap();
    }
    let e = parse_text(&registry, "a = solid.bxx(size=1)").unwrap_err();
    assert_eq!(
        e[0].suggestion.as_deref(),
        Some("Did you mean `solid.box`?")
    );
    let h = hint("a = solid.b()", "parse.unknown_op");
    assert!(h.starts_with("Did you mean"), "{h}");
    assert!(h.contains("solid.box"), "{h}");
    assert!(h.matches('`').count() <= 6, "{h}");
}

#[test]
fn ambiguous_case_insensitive_choices_are_refused() {
    let builtin = builtin_registry();
    let mut spec = builtin
        .get(&OpId::parse("xform.mirror@1").unwrap())
        .unwrap()
        .clone();
    spec.params[0].ty = ParamType::Choice {
        options: &["upper", "UPPER"],
    };
    let mut registry = OperatorRegistry::new();
    registry.register(spec).unwrap();
    let e = parse_text(&registry, "a = xform.mirror(axis=\"Upper\")").unwrap_err();
    assert_eq!(e[0].code, "param.range");
    assert!(e[0].message.contains("axis"), "{e:?}");
    assert!(e[0].message.contains("upper, UPPER"), "{e:?}");
    assert!(parse_text(&registry, "a = xform.mirror(axis=\"UPPER\")").is_ok());
}

#[test]
fn group_instance_parameters_and_ports_get_close_hints() {
    let group = "group shifted\nsource = group.input(name=\"source\")\nbody = xform.move(source, by=0)\nexpose offset = body.by\nresult body\nend\na = solid.box(size=1)\n";
    let h = hint(
        &format!("{group}b = shifted(sourc=a)"),
        "graph.unknown_port",
    );
    assert!(h.starts_with("Did you mean `source`?"), "{h}");
    assert!(h.contains("inputs: source"), "{h}");
    let h = hint(
        &format!("{group}b = shifted(source=a, offst=1)"),
        "parse.unknown_param",
    );
    assert!(h.starts_with("Did you mean `offset`?"), "{h}");
    assert!(h.contains("exposes: offset"), "{h}");
}
