//! `poly.skin` in the text language: a skeleton of named joints becomes one closed quad surface that the rest of the poly nodes accept, with a `joint` attribute.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(source: &str) -> Result<ve_modeling::Evaluation, (String, String)> {
    let commands = match parse_text(&builtin_registry(), source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(5);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_skin").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "apply: {applied:?}");
    Evaluator::default().evaluate(session.document(), Quality::Final).map_err(|e| (e.code.to_string(), e.message.to_string()))
}

const FIGURE: &str = "joints=\"pelvis 0 0.95 0 0.16 0.1 root; chest 0 1.3 0 0.16 0.1; neck 0 1.5 0 0.05 0.05; head 0 1.68 0.02 0.09 0.1; \
hip 0.1 0.9 0 0.09 0.09; knee 0.1 0.5 0.01 0.06 0.06; ankle 0.1 0.08 0 0.04 0.04; toe 0.1 0.03 0.14 0.04 0.03; \
shoulder 0.2 1.42 0 0.06 0.06; elbow 0.26 1.12 0.02 0.045 0.045; wrist 0.28 0.86 0.04 0.03 0.03\"";
const BONES: &str = "bones=\"pelvis-chest; chest-neck; neck-head; pelvis-hip; hip-knee; knee-ankle; ankle-toe; chest-shoulder; shoulder-elbow; elbow-wrist\"";

#[test]
fn a_one_sided_figure_skins_into_a_closed_quad_surface() {
    let ev = evaluate(&format!("s = poly.skin@1({FIGURE}, {BONES})\noutput body = s\n")).unwrap();
    let body = ev.parts.iter().find(|p| p.name == "body").unwrap();
    assert!(!body.positions.is_empty() && body.indices.len() % 3 == 0, "the skin has triangles");
    let top = body.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let bottom = body.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    assert!((1.66..1.72).contains(&top), "the head joint is the top: {top}");
    assert!(bottom > -0.01 && bottom < 0.06, "the toe is the bottom: {bottom}");
}

#[test]
fn the_skin_feeds_subdivision_and_materials() {
    let source = format!("s = poly.skin@1({FIGURE}, {BONES})\nsmooth = poly.subdivide@1(s, levels=2)\nm = solid.material@1(smooth, color=[0.8, 0.6, 0.5], roughness=0.5)\noutput body = m\n");
    let ev = evaluate(&source).unwrap();
    let body = ev.parts.iter().find(|p| p.name == "body").unwrap();
    assert!(body.indices.len() / 3 > 2000, "subdividing the skin gives a dense surface: {} triangles", body.indices.len() / 3);
}

#[test]
fn mistakes_are_refused_with_the_fix() {
    let no_root = evaluate("s = poly.skin@1(joints=\"a 0 0 0 0.1 0.1; b 0 1 0 0.1 0.1\", bones=\"a-b\")\noutput o = s\n").unwrap_err();
    assert_eq!(no_root.0, "skin.no_root", "{no_root:?}");
    let unknown = evaluate("s = poly.skin@1(joints=\"a 0 0 0 0.1 0.1 root; b 0 1 0 0.1 0.1\", bones=\"a-c\")\noutput o = s\n").unwrap_err();
    assert_eq!(unknown.0, "skin.bone_unknown_joint", "{unknown:?}");
    let syntax = evaluate("s = poly.skin@1(joints=\"a 0 0 root; b 0 1 0 0.1 0.1\", bones=\"a-b\")\noutput o = s\n").unwrap_err();
    assert_eq!(syntax.0, "skin.joint_syntax", "{syntax:?}");
}
