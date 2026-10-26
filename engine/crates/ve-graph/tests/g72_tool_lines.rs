//! Compact revision differences need one line per node, including pipe stages and group members.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, parse_text, to_text_node_lines};

#[test]
fn compact_node_lines_cover_group_members_pipes_and_duplicate_labels() {
    let mut session = Session::with_builtin(1);
    let source = "group block\n  body = solid.box(size=[1, 1, 1])\n  result body\nend\na = block()\npost = solid.box(size=[0.1, 1, 0.1]) | xform.rotate(angles=[0, 90deg, 0])\noutput post";
    let parsed = parse_text(session.registry(), source).expect("fixture parses");
    let batch = CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_tool_lines").expect("fixture transaction"),
        label: None,
        commands: parsed.commands,
    };
    assert!(matches!(session.apply(&batch), Outcome::Committed { .. }));
    let mut document = session.document().clone();
    let lines = to_text_node_lines(session.registry(), &document).expect("compact node lines");
    assert_eq!(lines.len(), document.nodes.len());
    assert!(
        lines
            .values()
            .any(|line| line == "block: body = solid.box(size=[1, 1, 1])"),
        "{lines:?}"
    );
    assert!(lines.values().any(|line| line == "a = block()"));
    assert!(lines.values().any(|line| line.contains("90deg")));
    assert!(
        lines
            .values()
            .all(|line| !line.contains(" | ") && !line.contains('@'))
    );
    for node in document
        .nodes
        .iter_mut()
        .filter(|node| node.group.is_none())
    {
        node.label = Some("duplicate".to_owned());
    }
    let lines = to_text_node_lines(session.registry(), &document).expect("disambiguated lines");
    assert_eq!(lines.len(), document.nodes.len());
    assert!(lines.values().any(|line| line.starts_with("n_")));
}
