//! Evaluates a text program natively and prints each output part's volume and triangle count, or the
//! diagnostic that refused it: `cargo run -p ve-modeling --example eval_text -- program.txt`.
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool that prints its result"
)]

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: eval_text <program.txt>");
        std::process::exit(2);
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("cannot read {path}");
        std::process::exit(2);
    };
    let batch = match parse_text(&builtin_registry(), &text) {
        Ok(batch) => batch,
        Err(error) => {
            eprintln!("{error:?}");
            std::process::exit(1);
        }
    };
    let Ok(transaction) = TransactionId::parse("t_eval_text") else {
        std::process::exit(1);
    };
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction,
        label: None,
        commands: batch.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        eprintln!("not committed: {outcome:?}");
        std::process::exit(1);
    }
    match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(evaluation) => {
            for part in &evaluation.parts {
                println!(
                    "{}: volume {:.6}, {} triangles",
                    part.name,
                    part.volume,
                    part.indices.len() / 3
                );
            }
        }
        Err(error) => println!("{}: {} (node {:?})", error.code, error.message, error.node),
    }
}
