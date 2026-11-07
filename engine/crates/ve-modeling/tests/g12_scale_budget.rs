//! Placed copies of one shape cost memory once: the stored-triangle budget counts the shared mesh, a separate (much larger)
//! bound counts what the placements would draw, and a scene that would draw a lot is warned about.
#![expect(
    clippy::unwrap_used,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn run(source: &str, quality: Quality) -> Result<Evaluation, String> {
    let parsed = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(1201);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_scale").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), quality)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn forest(shape: &str, count: usize) -> String {
    format!(
        "{shape}\nfield = inst.scatter@1(tree, count={count}, width=500, depth=500, seed=3)\noutput field = field"
    )
}

fn drawn(evaluation: &Evaluation) -> usize {
    evaluation
        .parts
        .iter()
        .map(|p| p.indices.len() / 3 * p.transforms.as_ref().map_or(1, Vec::len))
        .sum()
}

const CONE: &str = "tree = solid.cone@1(radius=0.7, height=3)";

#[test]
fn a_placed_shape_counts_once_against_the_stored_budget() {
    // 4,000 cones of 62 triangles draw 248,000 triangles: over the 200,000 preview limit if every copy counted, but the mesh
    // is stored once.
    let result = run(&forest(CONE, 4000), Quality::Preview).unwrap();
    assert!(
        drawn(&result) > 200_000,
        "the premise: {} drawn",
        drawn(&result)
    );
    let stored: usize = result.parts.iter().map(|p| p.indices.len() / 3).sum();
    assert!(stored < 1000, "one cone's triangles are stored ({stored})");
    assert_eq!(
        result.parts[0].transforms.as_ref().map(Vec::len),
        Some(4000)
    );
}

#[test]
fn a_heavy_shape_is_accepted_there_is_no_stored_triangle_limit() {
    // A sphere refined far past what the old budgets allowed evaluates (owner decision 2026-11-06).
    let sphere = "tree = solid.torus@1(major=0.5, minor=0.2, segments=512, tube_segments=256)";
    let result = run(&forest(sphere, 2), Quality::Preview).unwrap();
    let stored: usize = result.parts.iter().map(|p| p.indices.len() / 3).sum();
    assert!(stored > 200_000, "{stored} stored");
}

#[test]
fn a_scene_that_would_draw_a_lot_is_warned_about_but_still_evaluates() {
    let sphere = "tree = solid.sphere@1(radius=1, segments=64)";
    let result = run(&forest(sphere, 2500), Quality::Final).unwrap();
    assert!(
        drawn(&result) > 4_000_000,
        "the premise: {} drawn",
        drawn(&result)
    );
    let warning = result
        .warnings
        .iter()
        .find(|w| w.code == "budget.drawn_heavy")
        .unwrap_or_else(|| panic!("no heavy-scene warning in {:?}", result.warnings));
    assert!(
        warning.message.contains("triangles at full detail"),
        "{}",
        warning.message
    );
    assert!(!warning.node.is_empty());
}

#[test]
fn a_light_scene_gets_no_heavy_warning() {
    let result = run(&forest(CONE, 500), Quality::Final).unwrap();
    assert!(drawn(&result) < 4_000_000);
    assert!(
        result
            .warnings
            .iter()
            .all(|w| w.code != "budget.drawn_heavy")
    );
}

#[test]
fn what_the_placements_would_draw_has_no_hard_bound_but_is_warned_about() {
    // About 32,000 triangles a copy, 16,000 copies: over 500 million drawn, though stored it is only one mesh. No limit any more
    // (owner decision 2026-11-06); the author still gets the heavy-scene warning.
    let sphere = "tree = solid.sphere@1(radius=1, segments=256)";
    let one = run(&format!("{sphere}\noutput tree = tree"), Quality::Final).unwrap();
    let per_copy = one.parts[0].indices.len() / 3;
    assert!(
        per_copy * 16_000 > 200_000_000,
        "the premise: {per_copy} a copy"
    );
    let result = run(&forest(sphere, 16_000), Quality::Final).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.code == "budget.drawn_heavy")
    );
}
