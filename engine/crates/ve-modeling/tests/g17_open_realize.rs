//! Open meshes (leaf cards, planes, fences of panels) can be realized from a set of copies and planted again: `inst.realize` joins the
//! placed copies into one mesh without needing the pieces to be closed solids. This is what lets a whole leafy crown be one mesh that is then
//! placed hundreds of times (ADR 0060 phase 6, rigid transforms on any mesh). Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g17_open").unwrap(),
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

const CARDS: &str = "c = poly.grid@1(width=1, height=1, columns=1, rows=1)\na = inst.array@1(c, count_x=3, count_y=1, count_z=1, spacing=[2, 0, 0])\nr = inst.realize@1(a)\n";

#[test]
fn copies_of_an_open_card_realize_into_one_mesh() {
    let result = evaluate(&format!("{CARDS}output r = r\n")).unwrap();
    let triangles: usize = result.parts.iter().map(|p| p.indices.len() / 3).sum();
    assert_eq!(triangles, 6, "three quads of two triangles each");
    let xs: Vec<f32> = result
        .parts
        .iter()
        .flat_map(|p| p.positions.iter().map(|v| v[0]))
        .collect();
    let (low, high) = xs
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
    assert!(
        (low + 0.5).abs() < 1e-3 && (high - 4.5).abs() < 1e-3,
        "the cards stand at 0, 2 and 4: {low} to {high}"
    );
}

#[test]
fn a_realized_open_mesh_keeps_its_texture_coordinates() {
    let result = evaluate(&format!("{CARDS}output r = r\n")).unwrap();
    let part = &result.parts[0];
    assert!(!part.uvs.is_empty(), "the cards still carry UVs");
    assert!(
        part.uvs
            .iter()
            .all(|uv| (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]))
    );
}

#[test]
fn a_realized_open_mesh_can_be_planted_as_a_set() {
    let source = format!(
        "{CARDS}p = inst.array@1(r, count_x=1, count_y=1, count_z=4, spacing=[0, 0, 10])\noutput p = p\n"
    );
    let result = evaluate(&source).unwrap();
    let part = &result.parts[0];
    assert_eq!(
        part.transforms.as_ref().unwrap().len(),
        4,
        "four placements of the realized crown"
    );
    assert_eq!(part.indices.len() / 3, 6, "each is the six triangles");
}

#[test]
fn a_closed_solid_still_goes_through_the_boolean_path() {
    let source = "b = solid.box@1(size=[1, 1, 1])\na = inst.array@1(b, count_x=2, count_y=1, count_z=1, spacing=[0.5, 0, 0])\nr = inst.realize@1(a)\noutput r = r\n";
    let result = evaluate(source).unwrap();
    let volume: f64 = result.parts.iter().map(|p| p.volume).sum();
    assert!(
        (volume - 1.5).abs() < 1e-3,
        "overlapping boxes are unioned (1.5), not just concatenated (2): {volume}"
    );
}
