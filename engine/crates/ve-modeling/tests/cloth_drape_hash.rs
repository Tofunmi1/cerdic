//! The draped panel of `web/tests/fixtures/drape.txt`, hashed bit for bit (FNV-1a 32 over the points' f32 bits, little endian).
//! The browser test `web/tests/d1-drape-wasm.mjs` evaluates the same text in the wasm evaluator and expects this hash: native and wasm
//! must drape identically. If the solver changes on purpose, run this with --nocapture and update the constant in both places.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

/// Expected hash of the draped points (also in the browser test).
const EXPECTED: u32 = 2_686_809_893;

#[test]
fn the_draped_panel_has_a_fixed_hash() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/drape.txt"
    ))
    .unwrap();
    let commands = parse_text(&builtin_registry(), &source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_drape_hash").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    let part = result.parts.iter().find(|p| p.name == "cloth").unwrap();
    let mut hash: u32 = 0x811c_9dc5;
    for p in &part.positions {
        for v in p {
            for byte in v.to_bits().to_le_bytes() {
                hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
            }
        }
    }
    eprintln!("DRAPE_HASH {hash} points {}", part.positions.len());
    assert_eq!(hash, EXPECTED, "native drape hash changed");
}
