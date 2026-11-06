//! A set drawn only inside a camera-distance band is counted by what one camera can see of it, not by every copy: a forest whose
//! near, middle and far meshes stand in the same places must not be refused as if all three were drawn everywhere.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(params: &str) -> Result<(), String> {
    let text = format!(
        "t = terrain.tile@1(size=[300, 300], cell=2, seed=4, height=20, scale=120)\ns = solid.sphere@1(radius=1, segments=100)\nf = scatter.place@1(s, t, radius=3, seed=1{params})\noutput f = f\n"
    );
    let batch = parse_text(&builtin_registry(), &text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_show_budget").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map(|_| ())
        .map_err(|e| format!("{} {}", e.code, e.message))
}

#[test]
fn thousands_of_heavy_copies_without_a_band_are_accepted_there_is_no_triangle_limit() {
    // Owner decision 2026-11-06: no triangle limit; the copies share one mesh, so only the viewer's culling bounds the drawing.
    evaluate("").unwrap();
}

#[test]
fn the_same_copies_in_a_near_band_are_drawn_within_budget() {
    evaluate(", show_to=20").unwrap();
}
