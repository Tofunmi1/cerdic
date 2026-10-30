//! Evaluates a program written in the text language and prints the rest segment of every bone part in it (one `BONE name head tail` line each, metres, world
//! coordinates), so an animation can be computed against the exact joints of a body. `cargo run -p ve-modeling --example bone_segments -- program.txt` (add `points` after the file to print the skinned surface too, or `all` for every part)
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    reason = "a command line tool prints its report and stops at its own errors"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality, set_human_pack};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: bone_segments program.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let pack = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(pack).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), &source).unwrap().commands;
    let mut session = Session::with_builtin(1);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_bones").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    for part in &result.parts {
        if let Some(bone) = part.bone {
            println!(
                "BONE {} {:.5},{:.5},{:.5} {:.5},{:.5},{:.5}",
                part.name,
                bone.head[0],
                bone.head[1],
                bone.head[2],
                bone.tail[0],
                bone.tail[1],
                bone.tail[2]
            );
        }
    }
    let mode = std::env::args().nth(2);
    if matches!(mode.as_deref(), Some("points" | "all")) {
        // The rest-pose surface of every skinned part (every part with `all`), one `PT part x,y,z` line per vertex.
        for part in result
            .parts
            .iter()
            .filter(|part| mode.as_deref() == Some("all") || part.skin.is_some())
        {
            for p in &part.positions {
                println!("PT {} {:.4},{:.4},{:.4}", part.name, p[0], p[1], p[2]);
            }
        }
    }
    for warning in &result.warnings {
        eprintln!("WARNING {} {}", warning.code, warning.message);
    }
}
