//! `solid.finish` thickness: sunlight passing through a thin body (a wing membrane, a leaf, an ear, wax) and glowing on the side that faces the viewer when the
//! sun is behind it. The finish layer carries it to the renderer; 0 is the plain surface again. Written before the code; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(11);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_thickness").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

fn finish_of(params: &str) -> Result<Option<f32>, String> {
    let source = format!(
        "a = solid.box@1(size=[1,1,0.02])\nm = solid.material@1(a, color=[0.2,0.5,0.1], roughness=0.6)\nf = solid.finish@1(m{params})\noutput p = f\n"
    );
    let evaluation = evaluate(&source)?;
    Ok(evaluation.parts[0]
        .material
        .as_ref()
        .and_then(|m| m.finish)
        .map(|f| f.thickness))
}

#[test]
fn thickness_reaches_the_surface_finish() {
    let thickness = finish_of(", thickness=0.7").unwrap().unwrap();
    assert!((thickness - 0.7).abs() < 1e-6, "{thickness}");
}

#[test]
fn thickness_alone_is_a_finish_and_zero_is_the_plain_surface() {
    assert!(
        finish_of(", thickness=0.3").unwrap().is_some(),
        "a surface with only thickness has a finish layer"
    );
    assert!(
        finish_of(", thickness=0").unwrap().is_none(),
        "all zero is the plain surface again"
    );
}

#[test]
fn thickness_is_checked() {
    assert!(finish_of(", thickness=1.5").is_err());
    assert!(finish_of(", thickness=-0.1").is_err());
}

#[test]
fn thickness_can_sit_beside_the_other_layers() {
    let source = "a = solid.box@1(size=[1,1,0.02])\nf = solid.finish@1(a, subsurface=0.5, translucency=0.2, thickness=0.8)\noutput p = f\n";
    let finish = evaluate(source).unwrap().parts[0]
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap();
    assert!(
        (finish.subsurface - 0.5).abs() < 1e-6
            && (finish.translucency - 0.2).abs() < 1e-6
            && (finish.thickness - 0.8).abs() < 1e-6
    );
}
