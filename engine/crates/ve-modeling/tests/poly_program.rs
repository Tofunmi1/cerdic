//! `poly.program`: a short program run once for every vertex of a polygon mesh. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_poly_program").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A 2 by 2 grid of quads, 2 m wide and 2 m tall, in the z = 0 plane: nine vertices.
const GRID: &str = "g = poly.grid@1(width=2, height=2, columns=2, rows=2)\n";

fn positions(source: &str) -> Vec<[f32; 3]> {
    let result = evaluate(source).unwrap();
    let mut seen: Vec<[f32; 3]> = Vec::new();
    for p in &result.parts[0].positions {
        if !seen.contains(p) {
            seen.push(*p);
        }
    }
    seen
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn offset_adds_a_vector_to_every_vertex() {
    let all = positions(&format!(
        "{GRID}m = poly.program@1(g, mode=\"offset\", code=\"out = vec3(0.0, 0.0, 0.5)\")\noutput o = m\n"
    ));
    assert_eq!(all.len(), 9);
    assert!(all.iter().all(|p| near(p[2], 0.5)), "{all:?}");
}

#[test]
fn position_mode_can_use_the_vertex_number_and_count() {
    let all = positions(&format!(
        "{GRID}m = poly.program@1(g, mode=\"position\", code=\"out = vec3(p.x, p.y, i + count * 0.0)\")\noutput o = m\n"
    ));
    let mut heights: Vec<f32> = all.iter().map(|p| p[2]).collect();
    heights.sort_by(f32::total_cmp);
    let expected: Vec<f32> = (0..9).map(|n| n as f32).collect();
    assert_eq!(heights, expected, "each vertex gets its own number");
    let counted = positions(&format!(
        "{GRID}m = poly.program@1(g, mode=\"position\", code=\"out = vec3(p.x, p.y, count)\")\noutput o = m\n"
    ));
    assert!(counted.iter().all(|p| near(p[2], 9.0)));
}

#[test]
fn along_normal_moves_each_vertex_along_its_normal() {
    // The grid faces +z, so its normals are +z.
    let all = positions(&format!(
        "{GRID}m = poly.program@1(g, mode=\"along_normal\", code=\"out = 0.25 + n.z * 0.0\")\noutput o = m\n"
    ));
    assert!(all.iter().all(|p| near(p[2], 0.25)), "{all:?}");
}

#[test]
fn a_rippled_sheet_keeps_its_faces_and_changes_heights() {
    let all = positions(&format!(
        "{GRID}m = poly.program@1(g, mode=\"offset\", code=\"out = vec3(0.0, 0.0, 0.1 * sin(p.x * 8.0 + p.y * 5.0))\")\noutput o = m\n"
    ));
    assert_eq!(all.len(), 9);
    let low = all.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min);
    let high = all.iter().map(|p| p[2]).fold(f32::NEG_INFINITY, f32::max);
    assert!(high - low > 0.05, "heights vary: {low}..{high}");
    assert!(all.iter().all(|p| p[2].abs() <= 0.1001));
}

#[test]
fn a_program_can_write_a_layer_that_a_later_program_reads() {
    let all = positions(&format!(
        "{GRID}w = poly.program@1(g, mode=\"layer\", layer_name=\"lift\", code=\"out = select(p.x > 0.0, 1.0, 0.0)\")\nm = poly.program@1(w, mode=\"along_normal\", code=\"out = 0.5 * attr(\\\"lift\\\")\")\noutput o = m\n"
    ));
    for p in &all {
        let expected = if p[0] > 0.0 { 0.5 } else { 0.0 };
        assert!(near(p[2], expected), "{p:?}");
    }
}

#[test]
fn the_same_program_gives_the_same_mesh_every_time() {
    let source = format!(
        "{GRID}m = poly.program@1(g, mode=\"along_normal\", code=\"out = 0.3 * fbm(p * 3.0, 3.0, 11.0)\")\noutput o = m\n"
    );
    assert_eq!(positions(&source), positions(&source));
}

#[test]
fn a_mistake_in_the_program_names_the_line_and_the_fix() {
    let error = evaluate(&format!(
        "{GRID}m = poly.program@1(g, mode=\"offset\", code=\"out = vec3(1.0, 2.0\")\noutput o = m\n"
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "expr.error");
    assert!(error.1.contains("line 1"), "{}", error.1);
}

#[test]
fn the_wrong_kind_of_result_is_refused_with_both_types() {
    let error = evaluate(&format!(
        "{GRID}m = poly.program@1(g, mode=\"offset\", code=\"out = 1.0\")\noutput o = m\n"
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "expr.error");
    assert!(
        error.1.contains("vec3") && error.1.contains("float"),
        "{}",
        error.1
    );
}

#[test]
fn reading_a_layer_the_mesh_does_not_have_lists_the_ones_it_has() {
    let error = evaluate(&format!(
        "{GRID}m = poly.program@1(g, mode=\"along_normal\", code=\"out = attr(\\\"nope\\\")\")\noutput o = m\n"
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "expr.layer_missing");
    assert!(error.1.contains("nope"), "{}", error.1);
}
