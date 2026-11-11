//! G6-16 through the node language: a 100,000-face mesh through the polygon operators.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "the helpers unwrap fixtures that are known to be valid, report failures by panicking and print measured times for the report"
)]
use std::time::Instant;

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

/// A 317 x 317 quad sheet: 100,489 faces.
const SHEET: &str = "wall = poly.grid@1(width=317, height=317, columns=317, rows=317)\n";

fn session(text: &str) -> Session {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(3);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g6_large").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    session
}

fn run(name: &str, tail: &str) -> usize {
    let started = Instant::now();
    let result = Evaluator::default()
        .evaluate(
            session(&format!("{SHEET}{tail}")).document(),
            Quality::Final,
        )
        .unwrap_or_else(|e| panic!("{name}: {}: {}", e.code, e.message));
    let triangles = result.parts[0].indices.len() / 3;
    // Measure timings with a release run (`cargo test --release`).
    println!(
        "G6-16 {name}: {triangles} triangles in {:?}",
        started.elapsed()
    );
    triangles
}

#[test]
fn g6_16_a_hundred_thousand_faces_pass_through_each_operator() {
    let source = run("grid", "output wall");
    assert_eq!(source, 2 * 317 * 317);
    assert_eq!(
        run("weld", "w = poly.weld@1(wall, distance=0.001)\noutput w"),
        source
    );
    assert_eq!(
        run(
            "flip",
            "all = poly.select@1(wall)\nf = poly.flip_faces@1(wall, all)\noutput f"
        ),
        source
    );
    // One face edited in a sheet of 100,000.
    let one = "one = poly.select@1(wall, by=\"box\", box_min=[-0.1, -0.1, -1], box_max=[0.1, 0.1, 1], expect=\"one\")\n";
    assert_eq!(
        run(
            "delete",
            &format!("{one}d = poly.delete_faces@1(wall, one)\noutput d")
        ),
        source - 2
    );
    assert_eq!(
        run(
            "extrude",
            &format!("{one}e = poly.extrude_faces@1(wall, one, offset=[0, 0, 0.5])\noutput e")
        ),
        source + 8
    );
    assert_eq!(
        run(
            "inset",
            &format!("{one}i = poly.inset_faces@1(wall, one, distance=0.2)\noutput i")
        ),
        source + 8
    );
}

#[test]
fn g6_16_a_cancelled_job_stops_inside_a_polygon_operator() {
    let session = session(&format!(
        "{SHEET}all = poly.select@1(wall)\nf = poly.flip_faces@1(wall, all)\noutput f"
    ));
    let mut evaluator = Evaluator::default();
    evaluator.begin(session.document(), Quality::Final).unwrap();
    // Run the grid node, then cancel before the 100,000-face flip: the job is discarded.
    assert!(evaluator.step().unwrap().is_none());
    evaluator.cancel();
    assert!(
        evaluator.step().is_err_and(|e| e.code == "eval.no_job"),
        "a cancelled job cannot continue"
    );
    // The partial work was not cached as a result: a fresh run still evaluates every node.
    let full = evaluator
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert!(full.evaluated_nodes >= 3);
}
