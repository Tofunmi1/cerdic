//! G9 graph integration for concave polygon subdivision.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

#[test]
fn g9_a3_graph_subdivides_merged_concave_extrusion_and_reports_the_repair()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "profile = profile.polygon@1(points=[[0,0],[2,0],[2,2],[1,0.8],[0,2],[-0.5,1],[-0.5,0.25]])\n\
        solid = solid.extrude@1(profile, height=1)\n\
        triangles = poly.from_solid@1(solid)\n\
        cage = poly.merge_coplanar@1(triangles)\n\
        smooth = poly.subdivide@1(cage, levels=1)\n\
        output smooth";
    let parsed = parse_text(&builtin_registry(), source)
        .map_err(|diagnostics| std::io::Error::other(format!("{diagnostics:?}")))?;
    let mut session = Session::with_builtin(1);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_concave")?,
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|diagnostic| std::io::Error::other(diagnostic.message))?;
    assert!(!result.parts.is_empty());
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| { warning.code == "poly.subdivide.concave_face_triangulated" })
    );
    Ok(())
}
