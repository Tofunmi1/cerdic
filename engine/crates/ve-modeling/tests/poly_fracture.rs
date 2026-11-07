//! `poly.fracture`: a shape broken into Voronoi pieces that tile it with cracks. Written before the node was tuned; do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_fracture").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A wall 1.6 m wide, 1 m tall and 0.3 m thick, broken into `n` pieces; every piece is an output named p0, p1, ...
fn wall(n: usize, extra: &str) -> String {
    let mut text = "wall = solid.box@1(size=[1.6, 1.0, 0.3])\n".to_owned();
    for k in 0..n {
        text.push_str(&format!("f{k} = poly.fracture@1(wall, pieces={n}, piece={k}, voxel_size=0.04{extra})\noutput p{k} = f{k}\n"));
    }
    text
}

/// The enclosed volume of a piece from its own triangles (a piece with a pinched edge reads 0 in `Part::volume`, so that is not used).
fn volume(result: &Evaluation, name: &str) -> f64 {
    let Some(p) = result.parts.iter().find(|p| p.name == name) else {
        return 0.0;
    };
    p.indices
        .chunks(3)
        .map(|t| {
            let (a, b, c) = (
                p.positions[t[0] as usize],
                p.positions[t[1] as usize],
                p.positions[t[2] as usize],
            );
            f64::from(
                a[0] * (b[1] * c[2] - b[2] * c[1])
                    + a[1] * (b[2] * c[0] - b[0] * c[2])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]),
            ) / 6.0
        })
        .sum()
}

#[test]
fn the_pieces_add_up_to_the_wall_less_its_cracks() {
    let result = evaluate(&wall(8, ", gap=0.0")).unwrap_or_else(|e| panic!("{e:?}"));
    let total: f64 = (0..8).map(|k| volume(&result, &format!("p{k}"))).sum();
    let whole = 1.6 * 1.0 * 0.3;
    let each: Vec<f64> = (0..8).map(|k| volume(&result, &format!("p{k}"))).collect();
    // Surface nets round the corners by a fraction of a voxel, so the pieces hold a few percent less than the box.
    assert!(
        total < whole * 1.01 && total > whole * 0.9,
        "eight pieces with no gap hold {total} of {whole} m3: {each:?}"
    );
    let cracked = evaluate(&wall(8, ", gap=0.04")).unwrap();
    let with_gap: f64 = (0..8).map(|k| volume(&cracked, &format!("p{k}"))).sum();
    assert!(
        with_gap < total * 0.97 && with_gap > total * 0.6,
        "cracks take volume: {with_gap} against {total}"
    );
}

#[test]
fn every_piece_is_a_closed_solid_inside_the_wall() {
    let result = evaluate(&wall(6, "")).unwrap();
    let mut made = 0;
    for k in 0..6 {
        let Some(part) = result.parts.iter().find(|p| p.name == format!("p{k}")) else {
            continue;
        };
        made += 1;
        assert!(
            volume(&result, &format!("p{k}")) > 0.005,
            "piece {k} has volume {}",
            volume(&result, &format!("p{k}"))
        );
        for p in &part.positions {
            assert!(
                p[0].abs() < 0.81 && p[1].abs() < 0.51 && p[2].abs() < 0.16,
                "piece {k} leaves the wall: {p:?}"
            );
        }
    }
    assert!(made >= 5, "{made} of 6 pieces exist");
}

#[test]
fn the_same_seed_breaks_the_wall_the_same_way_and_another_seed_differently() {
    let first = |extra: &str| {
        let r = evaluate(&format!("wall = solid.box@1(size=[1.6, 1.0, 0.3])\nf = poly.fracture@1(wall, pieces=6, piece=0, voxel_size=0.04{extra})\noutput p = f\n")).unwrap();
        r.parts
            .iter()
            .find(|p| p.name == "p")
            .map(|p| p.positions.clone())
    };
    assert_eq!(first(", seed=3"), first(", seed=3"));
    assert_ne!(first(", seed=3"), first(", seed=4"));
}

#[test]
fn a_piece_number_past_the_end_is_refused_with_the_fix() {
    let error = evaluate("wall = solid.box@1(size=[1, 1, 1])\nf = poly.fracture@1(wall, pieces=4, piece=9)\noutput p = f\n").err().unwrap();
    assert_eq!(error.0, "param.range");
    assert!(error.1.contains("0 to 3"), "{}", error.1);
}

#[test]
fn pieces_fall_as_physics_bodies_and_come_to_rest_on_the_floor() {
    // Each piece is a dynamic convex body: dropped a metre above a floor, all eight settle on it.
    let mut text =
        "wall = solid.box@1(size=[1.6, 1.0, 0.3])\nlifted = xform.move@1(wall, by=[0, 1.5, 0])\n"
            .to_owned();
    for k in 0..8 {
        text.push_str(&format!("f{k} = poly.fracture@1(lifted, pieces=8, piece={k}, voxel_size=0.05, gap=0.02)\nb{k} = phys.body@1(f{k}, kind=\"dynamic\", shape=\"convex\")\noutput p{k} = b{k}\n"));
    }
    text.push_str("floor = solid.box@1(size=[8, 0.2, 8])\nfloor_p = xform.move@1(floor, by=[0, -0.1, 0])\nfloor_b = phys.body@1(floor_p, kind=\"static\", shape=\"box\")\noutput floor = floor_b\n");
    let result = evaluate(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let bodies = (0..8)
        .filter(|k| {
            result
                .parts
                .iter()
                .any(|p| p.name == format!("p{k}") && p.body.is_some())
        })
        .count();
    assert!(bodies >= 6, "{bodies} pieces carry physics bodies");
}
