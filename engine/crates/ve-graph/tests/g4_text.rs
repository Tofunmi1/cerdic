//! G4e: the text form for groups (case G4e-01 and the error cases).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use std::sync::atomic::{AtomicU64, Ordering};

use ve_graph::{
    CommandBatch, Diagnostic, Outcome, Session, TransactionId, Value, builtin_registry, parse_text,
    parse_text_in, to_text,
};

static KEY: AtomicU64 = AtomicU64::new(0);

const WHEEL: &str = "\
group wheel
  tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)
  hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)
  spoke = solid.box@1(size=[0.22, 0.03, 0.03])
  spokes = xform.radial@1(spoke, count=4, radius=0.14)
  rim = solid.union@1(tyre, hub, spokes)
  expose radius = tyre.major
  expose width = hub.height
  result rim
end
";

/// Reads `text` into the session's document and applies it; returns the outcome.
fn apply(session: &mut Session, text: &str) -> Outcome {
    let read = parse_text_in(session.registry(), session.document(), text)
        .unwrap_or_else(|e| panic!("the text does not read: {e:?}"));
    let batch = CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(&format!("t_g4t{}", KEY.fetch_add(1, Ordering::Relaxed)))
            .unwrap(),
        label: None,
        commands: read.commands,
    };
    session.apply(&batch)
}

fn session(text: &str) -> Session {
    let mut s = Session::with_builtin(4);
    let outcome = apply(&mut s, text);
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    s
}

fn errors(text: &str) -> Vec<Diagnostic> {
    parse_text(&builtin_registry(), text).unwrap_err()
}

fn code_of(text: &str) -> String {
    errors(text).remove(0).code
}

#[test]
fn g4e_01_a_group_and_its_instances_read_to_the_same_document_the_editor_makes() {
    let s = session(&format!(
        "{WHEEL}a = wheel()\nb = wheel(radius=0.4)\noutput a\noutput b\n"
    ));
    let doc = s.document();
    let group = &doc.groups[0];
    assert_eq!(group.name, "wheel");
    assert_eq!(doc.members(&group.id).count(), 5);
    let exposed: Vec<_> = group.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(exposed, ["radius", "width"]);
    let label = |name: &str| {
        doc.nodes
            .iter()
            .find(|n| n.label.as_deref() == Some(name))
            .unwrap()
    };
    assert_eq!(group.output, label("rim").id, "the result is the union");
    assert!(label("a").instance.as_ref().unwrap().overrides.is_empty());
    assert_eq!(
        label("b").instance.as_ref().unwrap().overrides["radius"],
        Value::Num(0.4)
    );
    assert_eq!(doc.outputs.len(), 2);
    assert!(
        doc.nodes
            .iter()
            .filter(|n| n.group.is_none())
            .all(|n| n.instance.is_some()),
        "only the two instances are top level"
    );
}

#[test]
fn a_document_with_groups_prints_and_reads_back_to_the_same_text() {
    let s = session(&format!(
        "{WHEEL}a = wheel()\nb = wheel(radius=0.4, width=0.25)\nc = wheel(muted=true)\noutput a\noutput b\n"
    ));
    let text = to_text(s.registry(), s.document()).unwrap();
    assert!(text.starts_with("group wheel\n"), "{text}");
    assert!(text.contains("expose radius = tyre.major"), "{text}");
    assert!(text.contains("result rim"), "{text}");
    assert!(text.contains("b = wheel(radius=0.4, width=0.25)"), "{text}");
    assert!(text.contains("c = wheel(muted=true)"), "{text}");
    let again = session(&text);
    assert_eq!(
        to_text(again.registry(), again.document()).unwrap(),
        text,
        "printing is canonical"
    );
}

#[test]
fn group_inputs_are_written_port_equals_node() {
    let s = session(
        "group bracket\n  plate = solid.box@1(size=[0.1, 0.1, 0.1])\n  mount = group.input@1(name=\"mount\")\n  joined = solid.union@1(mount, plate)\n  result joined\nend\npost = solid.box@1(size=[1, 1, 1])\nb = bracket(mount=post)\noutput b\n",
    );
    let doc = s.document();
    let b = doc
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("b"))
        .unwrap();
    let post = doc
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("post"))
        .unwrap();
    assert_eq!(b.inputs["mount"][0].node, post.id);
    let text = to_text(s.registry(), doc).unwrap();
    assert!(text.contains("b = bracket(mount=post)"), "{text}");
    let again = session(&text);
    assert_eq!(to_text(again.registry(), again.document()).unwrap(), text);
}

#[test]
fn a_later_text_can_instance_a_group_that_is_already_in_the_document() {
    let mut s = session(&format!("{WHEEL}first = wheel()\n"));
    let outcome = apply(
        &mut s,
        "second = wheel(radius=0.5)\nnext = xform.move@1(second, by=[1, 0, 0])\n",
    );
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    assert_eq!(s.document().groups.len(), 1);
    assert_eq!(
        s.document()
            .instances_of(&s.document().groups[0].id)
            .count(),
        2
    );
    // A member of a group is not a node the text can read.
    let read = parse_text_in(
        s.registry(),
        s.document(),
        "x = xform.move@1(hub, by=[1, 0, 0])\n",
    );
    assert_eq!(read.unwrap_err()[0].code, "parse.unknown_name");
}

#[test]
fn nested_groups_can_be_written() {
    let s = session(&format!(
        "{WHEEL}group axle\n  left = wheel()\n  right = wheel(radius=0.35)\n  both = solid.union@1(left, right)\n  result both\nend\nfront = axle()\n"
    ));
    assert_eq!(s.document().groups.len(), 2);
    let text = to_text(s.registry(), s.document()).unwrap();
    assert!(
        text.find("group wheel").unwrap() < text.find("group axle").unwrap(),
        "a group is printed before the group that uses it"
    );
    let again = session(&text);
    assert_eq!(to_text(again.registry(), again.document()).unwrap(), text);
}

#[test]
fn mistakes_in_group_text_say_what_is_wrong_and_where() {
    assert_eq!(code_of("a = nowhere()\n"), "parse.unknown_group");
    let e = errors(&format!("{WHEEL}a = wheel(spokes=3)\n"));
    assert_eq!(e[0].code, "parse.unknown_param");
    assert!(e[0].suggestion.as_deref().unwrap().contains("radius"));
    assert_eq!(e[0].span.map(|s| s.line), Some(11));
    assert_eq!(
        code_of(&format!("{WHEEL}a = wheel@1()\n")),
        "parse.group_version"
    );
    assert_eq!(
        code_of(&format!(
            "{WHEEL}post = solid.box@1(size=[1, 1, 1])\na = wheel(post)\n"
        )),
        "parse.syntax",
        "a group's inputs are named"
    );
    assert_eq!(
        code_of("group g\n  a = solid.box@1(size=[1, 1, 1])\nend\n"),
        "parse.missing_result"
    );
    assert_eq!(
        code_of("group g\n  a = solid.box@1(size=[1, 1, 1])\n  result a\n"),
        "parse.syntax"
    );
    assert_eq!(
        code_of("group g\n  group h\n  end\nend\n"),
        "parse.syntax",
        "groups are defined at the top level"
    );
    assert_eq!(
        code_of(
            "outside = solid.box@1(size=[1, 1, 1])\ngroup g\n  a = xform.move@1(outside, by=[0, 0, 0])\n  result a\nend\n"
        ),
        "parse.unknown_name",
        "a group cannot read a node outside it"
    );
    assert_eq!(
        code_of(&format!("{WHEEL}{WHEEL}")),
        "parse.duplicate_name",
        "two groups with one name"
    );
    assert_eq!(
        code_of(
            "group g\n  a = solid.box@1(size=[1, 1, 1])\n  expose x = nope.size\n  result a\nend\n"
        ),
        "parse.unknown_name"
    );
    assert_eq!(
        code_of(
            "group g\n  a = solid.box@1(size=[1, 1, 1])\n  expose x = a.nope\n  result a\nend\n"
        ),
        "parse.unknown_param"
    );
}

#[test]
fn a_value_the_member_refuses_is_refused_at_the_instance_line() {
    // The member's own range applies to the override, and the text says where it was written.
    let s = session(WHEEL);
    let read = parse_text_in(s.registry(), s.document(), "\na = wheel(radius=-1)\n");
    let diagnostics = read.unwrap_err();
    assert_eq!(diagnostics[0].code, "param.range");
    assert_eq!(diagnostics[0].span.map(|s| s.line), Some(2));
    assert_eq!(diagnostics[0].param.as_deref(), Some("radius"));
}

#[test]
fn an_instance_behind_its_group_cannot_be_printed() {
    let mut s = session(&format!("{WHEEL}a = wheel()\n"));
    let read = parse_text_in(s.registry(), s.document(), "").unwrap();
    assert!(read.commands.is_empty());
    let group = s.document().groups[0].id.clone();
    let batch = CommandBatch {
        doc: s.document().id.clone(),
        base_revision: s.document().revision,
        transaction: TransactionId::parse("t_g4tunx").unwrap(),
        label: None,
        commands: serde_json::from_value(serde_json::json!([
            {"unexpose_param": {"group": group.as_str(), "name": "width"}}
        ]))
        .unwrap(),
    };
    assert!(matches!(s.apply(&batch), Outcome::Committed { .. }));
    let error = to_text(s.registry(), s.document()).unwrap_err();
    assert_eq!(error.code, "text.unsupported");
    assert!(error.message.contains("upgrade"), "{}", error.message);
}
