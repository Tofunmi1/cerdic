//! Builds a program written in the text language and writes every part's triangles as a Wavefront OBJ file, for tools outside the engine
//! (a remesher spike, a viewer). Instanced parts are written once, untransformed.
//!
//! `cargo run -p ve-modeling --example dump_obj -- program.txt out_dir`
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    reason = "a command line tool prints its report and stops at its own usage errors"
)]
use std::fmt::Write as _;
use std::process::ExitCode;

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(program), Some(directory)) = (args.next(), args.next()) else {
        eprintln!("usage: dump_obj program.txt out_dir");
        return ExitCode::from(2);
    };
    let source = std::fs::read_to_string(&program).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(error) => {
            eprintln!("PARSE ERROR {error:?}");
            return ExitCode::FAILURE;
        }
    };
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_dump_obj").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        eprintln!("REFUSED {applied:?}");
        return ExitCode::FAILURE;
    }
    let result = match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("EVALUATION FAILED {}: {}", error.code, error.message);
            return ExitCode::FAILURE;
        }
    };
    std::fs::create_dir_all(&directory).unwrap();
    let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for part in &result.parts {
        if part.positions.is_empty() {
            continue;
        }
        let mut text = String::new();
        for p in &part.positions {
            writeln!(text, "v {} {} {}", p[0], p[1], p[2]).unwrap();
        }
        for t in part.indices.chunks_exact(3) {
            writeln!(text, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1).unwrap();
        }
        // several materials of one output are several parts with the same name: the later ones get `name__2.obj`, `name__3.obj`
        let seen = names.entry(part.name.clone()).or_insert(0);
        *seen += 1;
        let path = if *seen == 1 {
            format!("{directory}/{}.obj", part.name)
        } else {
            format!("{directory}/{}__{}.obj", part.name, *seen)
        };
        std::fs::write(&path, text).unwrap();
        println!(
            "{path}: {} vertices, {} triangles",
            part.positions.len(),
            part.indices.len() / 3
        );
    }
    ExitCode::SUCCESS
}
