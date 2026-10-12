//! G3-22: fuzz targets for the text parser and the document loader.
//!
//! Run as properties so they need no extra toolchain. The default is 4,000 cases per target, a few
//! seconds; set `PROPTEST_CASES=200000` for a long run. Inputs are random text, valid programs with
//! random damage, and valid saved files with random damage.
#![cfg(not(target_family = "wasm"))]
#![expect(
    clippy::unwrap_used,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use proptest::prelude::*;
use ve_graph::{
    CommandBatch, DocStore, Document, MemoryBackend, Migrator, Outcome, Session, TransactionId,
    builtin_registry, parse_text, to_text,
};

const VASE: &str = "\
outer = profile.polygon@1(points=[[0, 0], [0.06, 0], [0.11, 0.12], [0.07, 0.26], [0.05, 0.3], [0, 0.3]])
inner = profile.polygon@1(points=[[0, 0.01], [0.056, 0.01], [0.104, 0.12], [0.064, 0.26], [0.04, 0.31], [0, 0.31]])
wall = check.min_wall@1(outer, inner, min=3mm)
body = solid.revolve@1(outer, angle=360deg, segments=64)
cavity = solid.revolve@1(inner, angle=90deg)
vase = solid.difference@1(body, cavity)
output vase
";

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|c| c.parse().ok())
            .unwrap_or(4000),
        ..ProptestConfig::default()
    }
}

/// Characters the grammar uses, plus a few it does not, so damage is mostly near-miss.
const ALPHABET: &[u8] = b"abcxyz_0159.@=(),[]\"# \n-+eE$\\\t";

#[derive(Clone, Debug)]
enum Damage {
    Insert(usize, u8),
    Delete(usize, usize),
    Duplicate(usize, usize),
    Swap(usize, usize),
}

fn damage() -> impl Strategy<Value = Damage> {
    prop_oneof![
        (any::<usize>(), any::<u8>()).prop_map(|(at, c)| Damage::Insert(at, c)),
        (any::<usize>(), 0..40_usize).prop_map(|(at, n)| Damage::Delete(at, n)),
        (any::<usize>(), 0..60_usize).prop_map(|(at, n)| Damage::Duplicate(at, n)),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Damage::Swap(a, b)),
    ]
}

fn damaged(text: &str, edits: &[Damage]) -> String {
    let mut bytes = text.as_bytes().to_vec();
    for edit in edits {
        let len = bytes.len().max(1);
        match *edit {
            Damage::Insert(at, c) => {
                bytes.insert(at % (len + 1), ALPHABET[usize::from(c) % ALPHABET.len()]);
            }
            Damage::Delete(at, n) => {
                let start = at % len;
                bytes.drain(start..(start + n).min(bytes.len()));
            }
            Damage::Duplicate(at, n) => {
                let start = at % len;
                let piece = bytes[start..(start + n).min(bytes.len())].to_vec();
                bytes.splice(start..start, piece);
            }
            Damage::Swap(a, b) => {
                let (a, b) = (a % len, b % len);
                if a < bytes.len() && b < bytes.len() {
                    bytes.swap(a, b);
                }
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn check_text(text: &str) -> Result<(), TestCaseError> {
    let registry = builtin_registry();
    match parse_text(&registry, text) {
        Err(errors) => {
            prop_assert!(!errors.is_empty());
            for e in &errors {
                prop_assert!(!e.code.is_empty() && !e.message.is_empty());
                if let Some(span) = e.span {
                    prop_assert!(span.line >= 1 && span.col >= 1);
                }
            }
        }
        Ok(batch) => {
            prop_assert_eq!(batch.commands.len(), batch.spans.len());
            let mut session = Session::with_builtin(1);
            let outcome = session.apply(&CommandBatch {
                doc: session.document().id.clone(),
                base_revision: 0,
                transaction: TransactionId::parse("t_fuzz").unwrap(),
                label: None,
                commands: batch.commands,
            });
            if matches!(outcome, Outcome::Committed { .. }) {
                // Anything the parser and the document accept prints and reads back unchanged.
                let printed = to_text(&registry, session.document()).unwrap();
                let again = parse_text(&registry, &printed).unwrap();
                let mut second = Session::with_builtin(1);
                let outcome = second.apply(&CommandBatch {
                    doc: second.document().id.clone(),
                    base_revision: 0,
                    transaction: TransactionId::parse("t_fuzz2").unwrap(),
                    label: None,
                    commands: again.commands,
                });
                let committed = matches!(outcome, Outcome::Committed { .. });
                prop_assert!(committed);
                prop_assert_eq!(
                    second.document().canonical_json(),
                    session.document().canonical_json()
                );
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn parser_survives_random_text(text in "\\PC{0,300}") {
        check_text(&text)?;
    }

    #[test]
    fn parser_survives_damaged_programs_and_accepted_ones_round_trip(edits in proptest::collection::vec(damage(), 0..8)) {
        check_text(&damaged(VASE, &edits))?;
    }

    #[test]
    fn loader_survives_damaged_saves(slot in 0..2_usize, edits in proptest::collection::vec(damage(), 1..6)) {
        let registry = builtin_registry();
        let mut session = Session::with_builtin(1);
        let batch = parse_text(&registry, VASE).unwrap();
        session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_save").unwrap(),
            label: None,
            commands: batch.commands,
        });
        let mut store = DocStore::new(MemoryBackend::new(), "fuzz");
        store.save(session.document()).unwrap();
        store.save(session.document()).unwrap();
        let key = format!("ve.doc.fuzz.{}", ['a', 'b'][slot]);
        let text = store.backend().raw(&key).unwrap().clone();
        store.backend_mut().put_raw(&key, &damaged(&text, &edits));
        // Must return a document, a diagnostic or nothing; never panic, never touch storage.
        let before = store.backend().raw(&key).cloned();
        let _ = store.load(&registry, &Migrator::builtin());
        prop_assert_eq!(store.backend().raw(&key).cloned(), before);
    }

    #[test]
    fn loader_survives_random_files(garbage in "\\PC{0,400}") {
        let mut store = DocStore::new(MemoryBackend::new(), "fuzz");
        store.backend_mut().put_raw("ve.doc.fuzz.a", &garbage);
        let _ = store.load(&builtin_registry(), &Migrator::builtin());
    }
}

#[test]
fn a_blank_or_comment_only_program_is_an_empty_batch() {
    let batch = parse_text(&builtin_registry(), "\n# nothing here\n\n").unwrap();
    assert!(batch.commands.is_empty());
    let _ = Document::new(1);
}
