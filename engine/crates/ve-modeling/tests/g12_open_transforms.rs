//! Open polygon meshes (a leaf card) move, turn, scale and are placed like solids; the solid kernel is never asked to close them.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn run(source: &str) -> Result<Evaluation, String> {
    let parsed = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(1202);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_open").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn positions(source: &str) -> Vec<[f32; 3]> {
    run(source).unwrap().parts[0].positions.clone()
}

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

/// Whether two vertex lists hold the same points, in any order.
fn same_points(a: &[[f32; 3]], b: &[[f32; 3]]) -> bool {
    let unique = |list: &[[f32; 3]]| {
        let mut out: Vec<[f32; 3]> = Vec::new();
        for p in list {
            if !out.iter().any(|q| near(*p, *q)) {
                out.push(*p);
            }
        }
        out
    };
    let (a, b) = (unique(a), unique(b));
    a.len() == b.len() && a.iter().all(|p| b.iter().any(|q| near(*p, *q)))
}

const CARD: &str = "card = poly.grid@1(width=2, height=1, columns=1, rows=1)";

#[test]
fn an_open_card_moves_by_exactly_the_offset() {
    let at_rest = positions(&format!("{CARD}\noutput card = card"));
    let moved = positions(&format!(
        "{CARD}\nup = xform.move@1(card, by=[1, 2, 3])\noutput card = up"
    ));
    assert_eq!(moved.len(), at_rest.len());
    for (a, b) in at_rest.iter().zip(&moved) {
        assert!(
            near([a[0] + 1.0, a[1] + 2.0, a[2] + 3.0], *b),
            "{a:?} became {b:?}"
        );
    }
}

#[test]
fn rotation_and_scale_match_the_solid_kernel_on_a_closed_shape() {
    // The same box, once through the kernel's solid transforms and once as a polygon mesh through the mesh path.
    for transform in [
        "xform.rotate@1(shape, angles=[0.3, -0.7, 1.1])",
        "xform.scale@1(shape, by=[1.5, 0.5, 2])",
        "xform.move@1(shape, by=[-4, 0.25, 9])",
    ] {
        let solid = positions(&format!(
            "box = solid.box@1(size=[1, 2, 3])\nshape = xform.move@1(box, by=[0, 0, 0])\nout = {transform}\noutput out = out"
        ));
        let poly = positions(&format!(
            "box = solid.box@1(size=[1, 2, 3])\nshape = poly.from_solid@1(box)\nout = {transform}\noutput out = out"
        ));
        assert!(
            same_points(&solid, &poly),
            "{transform}: the polygon path and the solid kernel disagree"
        );
    }
}

#[test]
fn a_card_keeps_its_uvs_and_surface_through_a_transform() {
    let source = |tail: &str| {
        format!(
            "{CARD}\npaint = solid.material@1(card, color=[0.2, 0.6, 0.1], roughness=0.8)\n{tail}"
        )
    };
    let rest = run(&source("output card = paint")).unwrap();
    let moved = run(&source(
        "turned = xform.rotate@1(paint, angles=[0, 1.0, 0])\nbig = xform.scale@1(turned, by=[2, 2, 2])\noutput card = big",
    ))
    .unwrap();
    assert!(!rest.parts[0].uvs.is_empty(), "the card has UVs");
    assert_eq!(
        moved.parts[0].uvs, rest.parts[0].uvs,
        "a transform does not touch UVs"
    );
    let color = moved.parts[0].material.as_ref().unwrap().color;
    assert_eq!(color, [0.2, 0.6, 0.1]);
}

#[test]
fn a_transformed_card_is_still_an_editable_polygon_mesh() {
    // Moved away and back again it is where it began, and a polygon edit still works on it.
    let at_rest = positions(&format!("{CARD}\noutput card = card"));
    let there_and_back = positions(&format!(
        "{CARD}\na = xform.move@1(card, by=[5, -3, 2])\nb = xform.move@1(a, by=[-5, 3, -2])\noutput card = b"
    ));
    assert!(same_points(&at_rest, &there_and_back));
    let edited = run(&format!(
        "{CARD}\na = xform.scale@1(card, by=[2, 1, 1])\nsub = poly.subdivide@1(a, levels=1)\noutput card = sub"
    ))
    .unwrap();
    assert!(
        edited.parts[0].indices.len() / 3 > 2,
        "subdividing the stretched card made more triangles"
    );
}

#[test]
fn open_cards_are_placed_in_sets_and_keep_foliage() {
    let result = run(&format!(
        "{CARD}\npaint = solid.material@1(card, color=[0.2, 0.6, 0.1], roughness=0.8)\nleaf = solid.foliage@1(paint, alpha_cutoff=0.5, sway=0.6)\n\
         lean = xform.rotate@1(leaf, angles=[0.4, 0, 0])\nleaves = inst.array@1(lean, count_x=4, count_y=1, count_z=3, spacing=[1, 0, 1])\noutput leaves = leaves"
    ))
    .unwrap();
    let part = &result.parts[0];
    assert_eq!(
        part.transforms.as_ref().map(Vec::len),
        Some(12),
        "twelve placed cards"
    );
    assert_eq!(
        part.indices.len() / 3,
        2,
        "one card is two triangles, stored once"
    );
    let foliage = part.material.as_ref().and_then(|m| m.foliage).unwrap();
    assert!((foliage.sway - 0.6).abs() < 1e-6 && (foliage.alpha_cutoff - 0.5).abs() < 1e-6);
}

#[test]
fn open_cards_scatter_and_move_as_a_set() {
    let result = run(&format!(
        "{CARD}\nleaves = inst.scatter@1(card, count=40, width=6, depth=6, seed=2)\nhigh = xform.move@1(leaves, by=[0, 3, 0])\noutput leaves = high"
    ))
    .unwrap();
    let matrices = result.parts[0].transforms.as_ref().unwrap();
    assert_eq!(matrices.len(), 40);
    assert!(
        matrices.iter().all(|m| (m[13] - 3.0).abs() < 1e-5),
        "every copy was lifted by 3 m"
    );
}

#[test]
fn unioning_open_copies_is_refused_without_a_crash() {
    let error = run(&format!(
        "{CARD}\nleaves = inst.array@1(card, count_x=3, spacing=[1, 0, 0])\nsolid = inst.realize@1(leaves)\noutput solid = solid"
    ))
    .unwrap_err();
    assert!(!error.is_empty());
}
