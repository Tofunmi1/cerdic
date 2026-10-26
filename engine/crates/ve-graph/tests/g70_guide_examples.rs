//! The compact guide includes these exact files, so its examples must remain valid programs.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};

#[test]
fn compact_guide_examples_parse_and_apply() {
    let examples = [
        include_str!("../../../tools/mcp/guide-examples/compact_pipes.txt"),
        include_str!("../../../tools/mcp/guide-examples/compact_range.txt"),
        include_str!("../../../tools/mcp/guide-examples/compact_list.txt"),
        include_str!("../../../tools/mcp/guide-examples/compact_pairs.txt"),
        include_str!("../../../tools/mcp/guide-examples/compact_nested.txt"),
    ];
    for source in examples {
        let batch = parse_text(&builtin_registry(), source).expect(source);
        let mut session = Session::with_builtin(1);
        let outcome = session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_guide").expect("fixture transaction"),
            label: None,
            commands: batch.commands,
        });
        assert!(
            matches!(outcome, Outcome::Committed { .. }),
            "{source}\n{outcome:?}"
        );
    }
}
