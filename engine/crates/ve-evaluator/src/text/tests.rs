use super::{compact, parse};
use serde_json::Value;
use ve_graph::{
    CommandBatch, Document, Outcome, Session, TransactionId, builtin_registry, parse_text, to_text,
};

fn document(source: &str) -> Document {
    let mut session = Session::with_builtin(1);
    let parsed = parse_text(&builtin_registry(), source).expect("fixture parses");
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_text").expect("fixture transaction"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    session.document().clone()
}

fn json(source: &str) -> Value {
    serde_json::from_str(source).expect("valid response JSON")
}

#[test]
fn compact_snapshot_folds_pipes_and_preserves_canonical_text() {
    let doc = document(
        "post = solid.box(size=[0.1, 1, 0.1]) | xform.rotate(angles=[2deg, 0deg, 10deg])\noutput post",
    );
    let canonical = to_text(&builtin_registry(), &doc).expect("canonical fixture");
    let source = serde_json::to_string(&doc).expect("document JSON");
    let reply = json(&compact(&source, false));
    let text = reply["text"].as_str().expect("compact text");
    assert!(text.contains(" | "), "{text}");
    assert!(text.contains("10deg"), "{text}");
    assert!(!text.contains('@'), "{text}");
    assert_eq!(
        to_text(&builtin_registry(), &document(text)).expect("round trip"),
        canonical
    );
    assert_eq!(
        to_text(&builtin_registry(), &doc).expect("unchanged fixture"),
        canonical
    );
}

#[test]
fn node_lines_are_standalone_compact_and_keep_full_parameters() {
    let doc = document(
        "post = solid.box(size=[0.1, 1, 0.1]) | xform.rotate(angles=[2deg, 0deg, 10deg])\nread_2 = script.js(source=\"a long script that must stay complete rather than being truncated\")",
    );
    let source = serde_json::to_string(&doc).expect("document JSON");
    let reply = json(&compact(&source, true));
    for node in &doc.nodes {
        let line = reply["lines"][node.id.as_str()]
            .as_str()
            .expect("line per node");
        assert!(
            !line.contains('@') || node.op.to_string().starts_with("group.use"),
            "{line}"
        );
        assert!(!line.contains(" | "), "{line}");
    }
    let rotation = &reply["lines"][doc.nodes[1].id.as_str()];
    assert_eq!(
        rotation,
        "post = xform.rotate(post_1, angles=[2deg, 0deg, 10deg])"
    );
    let script = reply["lines"][doc.nodes[2].id.as_str()]
        .as_str()
        .expect("script line");
    assert!(script.contains("rather than being truncated"), "{script}");
}

#[test]
fn node_lines_handle_duplicate_labels_and_groups() {
    let mut doc = document(
        "group block\n  body = solid.box(size=[1, 1, 1])\n  result body\nend\na = block()\nb = block()\noutput a",
    );
    let mut top = doc.nodes.iter_mut().filter(|node| node.group.is_none());
    top.next().expect("first instance").label = Some("duplicate".to_owned());
    top.next().expect("second instance").label = Some("duplicate".to_owned());
    let source = serde_json::to_string(&doc).expect("document JSON");
    let reply = json(&compact(&source, true));
    assert_eq!(reply["lines"].as_object().expect("lines map").len(), 2);
    for node in doc.nodes.iter().filter(|node| node.group.is_none()) {
        assert!(reply["lines"][node.id.as_str()].is_string(), "{reply}");
    }
}

#[test]
fn parser_uses_existing_labels_and_reports_loop_diagnostics() {
    let source = serde_json::to_string(&document("base = solid.box(size=[1, 1, 1])"))
        .expect("document JSON");
    let reply = json(&parse(
        &source,
        "for i in 0..2\n  post_{i} = base | xform.move(by=[{i}, 0, 0])\nend",
    ));
    assert!(reply["commands"].is_array(), "{reply}");
    assert!(reply["spans"].is_array(), "{reply}");
    let bad = json(&parse(
        &source,
        "for i in 0..2\n  post_{i} = no.such()\nend",
    ));
    assert_eq!(bad["diagnostics"][0]["code"], "parse.unknown_op");
    assert!(
        bad["diagnostics"][0]["message"]
            .as_str()
            .expect("diagnostic message")
            .contains("i=0")
    );
    assert_eq!(bad["diagnostics"][0]["span"]["line"], 2);
}

#[test]
fn invalid_snapshots_return_typed_errors() {
    for reply in [
        compact("not JSON", false),
        compact("{}", true),
        parse("{}", ""),
    ] {
        assert_eq!(json(&reply)["error"]["code"], "parse.syntax");
    }
}

#[test]
fn javascript_integer_numbers_keep_radians_and_default_omission() {
    let doc = document(
        "post = solid.box(size=[1, 1, 1]) | xform.rotate(angles=[1rad, 0rad, 0rad])\npaint = solid.material(post, metallic=0, opacity=1)\noutput paint",
    );
    let source = serde_json::to_string(&doc)
        .expect("document JSON")
        .replace(".0", "");
    let reply = json(&compact(&source, false));
    let text = reply["text"].as_str().expect("compact text");
    assert!(text.contains("angles=[1rad, 0deg, 0deg]"), "{text}");
    assert!(
        !text.contains("metallic=") && !text.contains("opacity="),
        "defaults omitted: {text}"
    );
    assert_eq!(
        to_text(&builtin_registry(), &document(text)).expect("round trip"),
        to_text(&builtin_registry(), &doc).expect("original")
    );
    let doc = document(
        "group tilted\n  block = solid.box(size=[1, 1, 1])\n  turn = xform.rotate(block, angles=[0rad, 0rad, 0rad])\n  expose angles = turn.angles\n  result turn\nend\na = tilted(angles=[1rad, 0rad, 0rad])\noutput a",
    );
    let source = serde_json::to_string(&doc)
        .expect("group JSON")
        .replace(".0", "");
    let reply = json(&compact(&source, false));
    let text = reply["text"].as_str().expect("compact group text");
    assert!(
        text.contains("a = tilted(angles=[1rad, 0deg, 0deg])"),
        "{text}"
    );
    assert_eq!(
        to_text(&builtin_registry(), &document(text)).expect("group round trip"),
        to_text(&builtin_registry(), &doc).expect("original group")
    );
}
