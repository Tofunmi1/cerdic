//! G9a-4: the study room fits one bounded atomic command transaction.
use std::{path::Path, time::Instant};
use ve_graph::{
    CommandBatch, MAX_BATCH_BYTES, Outcome, Session, TransactionId, builtin_registry, parse_text,
};

#[test]
fn g9_a4_study_room_applies_and_undoes_as_one_transaction() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/gallery/visual/study_room.txt");
    let source = std::fs::read_to_string(path).expect("study room fixture is present");
    let parsed = parse_text(&builtin_registry(), &source).expect("study room text parses");
    assert_eq!(parsed.commands.len(), 386);
    let mut session = Session::with_builtin(42);
    let before = session.document().clone();
    let batch = CommandBatch {
        doc: before.id.clone(),
        base_revision: before.revision,
        transaction: TransactionId::parse("t_g9_room").expect("valid transaction id"),
        label: Some("build study room".into()),
        commands: parsed.commands,
    };
    let encoded = serde_json::to_vec(&batch).expect("batch serializes");
    assert!(encoded.len() < MAX_BATCH_BYTES, "{} bytes", encoded.len());
    let batch = CommandBatch::from_json(std::str::from_utf8(&encoded).expect("JSON is UTF-8"))
        .expect("study room stays inside the JSON batch cap");
    let started = Instant::now();
    let outcome = session.apply(&batch);
    let apply_elapsed = started.elapsed();
    assert!(
        matches!(outcome, Outcome::Committed { revision: 1, .. }),
        "{outcome:?}"
    );
    assert_eq!(session.document().nodes.len(), 191);
    assert!(
        session.undo().is_some(),
        "the whole fixture is one undo entry"
    );
    assert!(session.document().nodes.is_empty());
    assert!(session.document().outputs.is_empty());
    assert!(
        session.undo().is_none(),
        "no fixture chunks remain in history"
    );
    println!(
        "study room {}-command atomic apply: {apply_elapsed:?}, {} JSON bytes",
        batch.commands.len(),
        encoded.len()
    );
}
