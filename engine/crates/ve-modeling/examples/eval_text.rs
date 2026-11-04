//! Builds a program written in the text language, the way the editor does, and reports what a model needs to judge it: errors with their nodes,
//! the parts with triangles and copies, the drawn total against the provisional Quest budget, and (when the program defines a game) its
//! text form and what bots find out about it.
//!
//! `cargo run -p ve-modeling --example eval_text -- program.txt [policy ...]` where a policy is `idle`, `random` or `seek:<hero tag>:<target tag,...>`.
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    reason = "a command line tool prints its report and stops at its own usage errors"
)]
use std::process::ExitCode;

use ve_game::playtest::{Policy, run};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

const QUEST_TRIANGLES: usize = 300_000;

fn policy(word: &str) -> Option<Policy> {
    match word {
        "idle" => Some(Policy::Idle),
        "random" => Some(Policy::Random),
        _ => {
            let rest = word.strip_prefix("seek:")?;
            let (hero, target) = rest.split_once(':')?;
            Some(Policy::Seek {
                hero: hero.to_owned(),
                target: target.to_owned(),
            })
        }
    }
}

/// Evaluates step by step, timing each node, and prints the slowest ones by the name the program gave them.
fn timed(doc: &ve_graph::Document) -> Result<ve_modeling::Evaluation, Box<ve_graph::Diagnostic>> {
    let mut evaluator = Evaluator::default();
    let started = std::time::Instant::now();
    evaluator.begin(doc, Quality::Final)?;
    let mut spent: Vec<(std::time::Duration, String)> = Vec::new();
    let result = loop {
        let next = evaluator.next_node();
        let before = std::time::Instant::now();
        let done = evaluator.step()?;
        if let Some(next) = next {
            spent.push((
                before.elapsed(),
                format!(
                    "{} = {}",
                    next.label.unwrap_or_else(|| "?".to_owned()),
                    next.operator
                ),
            ));
        }
        if let Some(result) = done {
            spent.push((
                before.elapsed(),
                "(finishing: occlusion bake and parts)".to_owned(),
            ));
            break result;
        }
    };
    println!(
        "TIME {:.1} s in all, {} nodes ran; slowest:",
        started.elapsed().as_secs_f64(),
        spent.len() - 1
    );
    spent.sort_by(|a, b| b.0.cmp(&a.0));
    for (took, name) in spent.iter().take(10) {
        println!("  {:>7.0} ms  {name}", took.as_secs_f64() * 1000.0);
    }
    Ok(result)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: eval_text program.txt [idle|random|seek:<hero>:<target> ...]");
        return ExitCode::from(2);
    };
    let source = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            return ExitCode::from(2);
        }
    };
    // A program that makes a person needs the human pack.
    if let Ok(bytes) = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/human/human.pack"
    )) {
        let _ = ve_modeling::set_human_pack(&bytes);
    }
    let parsed = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed,
        Err(errors) => {
            for error in errors {
                println!("PARSE ERROR {error:?}");
            }
            return ExitCode::FAILURE;
        }
    };
    println!(
        "COMMANDS {} of {} per batch, source {} of {} bytes",
        parsed.commands.len(),
        ve_graph::MAX_COMMANDS,
        source.len(),
        ve_graph::MAX_BATCH_BYTES
    );
    let mut session = Session::with_builtin(1);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_eval_text").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        println!("REFUSED {outcome:?}");
        return ExitCode::FAILURE;
    }
    let result = match timed(session.document()) {
        Ok(result) => result,
        Err(error) => {
            // Name the node as the program does (its label and operator), not by an id nobody wrote.
            let named = error.node.as_ref().and_then(|id| {
                session
                    .document()
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .map(|node| format!("{} = {}", node.label.as_deref().unwrap_or("?"), node.op))
            });
            println!(
                "EVALUATION FAILED {}: {} (node {}){}",
                error.code,
                error.message,
                named.unwrap_or_else(|| format!("{:?}", error.node)),
                error
                    .suggestion
                    .as_ref()
                    .map_or_else(String::new, |s| format!(" -> {s}"))
            );
            return ExitCode::FAILURE;
        }
    };
    let mut drawn = 0_usize;
    for part in &result.parts {
        let copies = part.transforms.as_ref().map_or(1, Vec::len);
        let triangles = part.indices.len() / 3;
        drawn += triangles * copies;
        // The part's box in metres (before any placement), so a world's heights and extents can be read without a viewer.
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &part.positions {
            for axis in 0..3 {
                lo[axis] = lo[axis].min(p[axis]);
                hi[axis] = hi[axis].max(p[axis]);
            }
        }
        println!(
            "PART {} region {} triangles {triangles} copies {copies} box x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1}{}",
            part.name,
            part.region,
            lo[0],
            hi[0],
            lo[1],
            hi[1],
            lo[2],
            hi[2],
            part.visible_range
                .map_or_else(String::new, |(from, to)| format!(" shown {from} to {to} m"))
        );
    }
    println!(
        "DRAWN {drawn} triangles ({}% of the provisional Quest budget of {QUEST_TRIANGLES})",
        drawn * 100 / QUEST_TRIANGLES
    );
    let programs: std::collections::BTreeSet<_> = result
        .parts
        .iter()
        .filter_map(|part| part.material.as_ref()?.texture_program.as_deref())
        .collect();
    println!(
        "COST {} nodes, {} outputs, {} lights, {} distinct texture programs (the viewer holds about 32 texture layers)",
        session.document().nodes.len(),
        result.parts.len(),
        result.lights.len(),
        programs.len()
    );
    for warning in &result.warnings {
        println!(
            "WARNING {}: {} ({})",
            warning.code, warning.message, warning.node
        );
    }
    if let Some(game) = &result.game {
        println!("GAME\n{}", game.to_text());
        for word in args {
            match policy(&word) {
                Some(p) => {
                    let report = run(game, 1, &p, 3600);
                    println!(
                        "PLAYTEST {word}: {:?} after {} steps, vars {:?}, rules that never fired {:?}",
                        report.status, report.steps, report.vars, report.never_fired
                    );
                }
                None => println!("unknown policy {word:?}"),
            }
        }
    }
    ExitCode::SUCCESS
}
