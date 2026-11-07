//! `poly.script`: a sandboxed JavaScript program builds a polygon mesh. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_script").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A node line running `code` (single quotes and semicolons only: it sits inside a text-language string).
fn script(code: &str, extra: &str) -> String {
    format!("s = poly.script@1({extra}code=\"{code}\")\noutput o = s\n")
}

fn positions(result: &Evaluation) -> Vec<[f32; 3]> {
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
fn a_script_builds_a_staircase() {
    let result = evaluate(&script(
        "const m = new Mesh(); const n = 6; for (let i = 0; i < n; i++) { const y = i * 0.2, z = i * 0.3; const a = m.vertex(0, y, z), b = m.vertex(1, y, z), c = m.vertex(1, y, z + 0.3), d = m.vertex(0, y, z + 0.3); m.face(a, d, c, b); } m",
        "",
    ))
    .unwrap();
    let all = positions(&result);
    assert_eq!(all.len(), 24, "four vertices per step");
    let top = all.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
    let far = all.iter().map(|p| p[2]).fold(f32::NEG_INFINITY, f32::max);
    assert!(near(top, 1.0) && near(far, 1.8), "{top} {far}");
}

const GRID: &str = "op('poly.grid@1', {width: 2, height: 2, columns: 2, rows: 2})";

#[test]
fn a_script_can_call_an_operator_and_use_its_mesh() {
    let result = evaluate(&script(
        &format!("const g = {GRID}; const m = new Mesh(); g.points.forEach(p => m.vertex(p[0], p[1], p[2] + 1)); g.faces.forEach(f => m.face(...f)); m"),
        "",
    ))
    .unwrap();
    let all = positions(&result);
    assert_eq!(all.len(), 9);
    assert!(all.iter().all(|p| near(p[2], 1.0)), "{all:?}");
}

#[test]
fn an_operator_result_can_feed_another_operator() {
    let result = evaluate(&script(
        &format!("op('poly.subdivide@1', {{levels: 1}}, {{in: {GRID}}})"),
        "",
    ))
    .unwrap();
    assert!(positions(&result).len() > 9, "subdividing adds vertices");
}

#[test]
fn a_solid_operator_comes_back_as_triangles() {
    let result = evaluate(&script("op('solid.box@1', {size: [1, 2, 3]})", "")).unwrap();
    let all = positions(&result);
    let height = all.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max)
        - all.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    assert!(near(height, 2.0), "{height}");
}

#[test]
fn a_boolean_takes_a_list_for_its_variadic_port() {
    let cut = "op('solid.difference@1', {}, {in: [op('solid.box@1', {size: [2, 2, 2]}), op('solid.box@1', {size: [1, 1, 3]})]})";
    let whole = evaluate(&script("op('solid.box@1', {size: [2, 2, 2]})", "")).unwrap();
    let drilled = evaluate(&script(cut, "")).unwrap();
    assert!(
        drilled.parts[0].positions.len() > whole.parts[0].positions.len(),
        "a hole through the box adds vertices"
    );
}

#[test]
fn a_curve_operator_comes_back_as_points() {
    let result = evaluate(&script(
        "const c = op('curve.expr@1', {samples: 8, code: 'out = vec3(t, 0.0, 0.0)'}); const m = new Mesh(); c.points.forEach(p => m.vertex(p[0], p[1], p[2])); m.face(0, 1, 2); m",
        "",
    ))
    .unwrap();
    assert_eq!(positions(&result).len(), 8);
}

#[test]
fn an_operator_given_a_wrong_parameter_is_refused_with_the_reason() {
    let error = evaluate(&script("op('poly.grid@1', {nonsense: 1})", ""))
        .err()
        .unwrap();
    assert_eq!(error.0, "script.error");
    assert!(
        error.1.contains("poly.grid@1") && error.1.contains("refused"),
        "{}",
        error.1
    );
}

#[test]
fn an_unknown_operator_is_refused() {
    let error = evaluate(&script("op('poly.nonsense@1', {})", ""))
        .err()
        .unwrap();
    assert_eq!(error.0, "script.error");
    assert!(error.1.contains("poly.nonsense@1"), "{}", error.1);
}

#[test]
fn a_script_cannot_start_a_script() {
    let error = evaluate(&script("op('poly.script@1', {})", ""))
        .err()
        .unwrap();
    assert!(error.1.contains("cannot call poly.script"), "{}", error.1);
}

#[test]
fn too_many_operator_calls_are_refused_with_the_limit() {
    let error = evaluate(&script(
        "for (let i = 0; i < 200; i++) op('solid.box@1', {size: [1, 1, 1]}); new Mesh()",
        "",
    ))
    .err()
    .unwrap();
    assert!(error.1.contains("128"), "{}", error.1);
}

#[test]
fn a_script_can_read_and_change_the_linked_mesh() {
    let source = format!(
        "g = poly.grid@1(width=2, height=2, columns=2, rows=2)\ns = poly.script@1(g, code=\"const m = new Mesh(); for (const p of input.points) m.vertex(p[0], p[1], p[2] + 1.5); for (const f of input.faces) m.face(...f); m\")\noutput o = s\n"
    );
    let result = evaluate(&source).unwrap();
    let all = positions(&result);
    assert_eq!(all.len(), 9);
    assert!(all.iter().all(|p| near(p[2], 1.5)), "{all:?}");
}

#[test]
fn the_same_script_and_seed_give_the_same_mesh_and_another_seed_a_different_one() {
    let code = "const m = new Mesh(); for (let i = 0; i < 4; i++) m.vertex(Math.random(), Math.random(), Math.random()); m.face(0, 1, 2); m.face(0, 2, 3); m";
    let a = positions(&evaluate(&script(code, "seed=5, ")).unwrap());
    let b = positions(&evaluate(&script(code, "seed=5, ")).unwrap());
    let c = positions(&evaluate(&script(code, "seed=6, ")).unwrap());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn an_error_in_the_script_names_the_line_and_the_message() {
    let error = evaluate(&script(
        "let a = 1;\\nlet b = 2;\\nthrow new Error('no good');",
        "",
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "script.error");
    assert!(
        error.1.contains("no good") && error.1.contains("line 3"),
        "{}",
        error.1
    );
}

#[test]
fn an_endless_loop_is_stopped_by_the_budget_with_the_fix() {
    let error = evaluate(&script("while (true) {}", "budget=20, "))
        .err()
        .unwrap();
    assert_eq!(error.0, "script.budget");
}

#[test]
fn a_memory_bomb_is_an_error_not_a_crash() {
    let error = evaluate(&script(
        "const k = []; while (true) k.push(new Array(1000000).fill(1.5));",
        "budget=20000, ",
    ))
    .err()
    .unwrap();
    assert!(error.0.starts_with("script."), "{error:?}");
}

#[test]
fn a_face_that_uses_a_missing_vertex_says_which_and_how_many_exist() {
    let error = evaluate(&script(
        "const m = new Mesh(); m.vertex(0, 0, 0); m.vertex(1, 0, 0); m.vertex(0, 1, 0); m.face(0, 1, 7); m",
        "",
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "script.result");
    assert!(
        error.1.contains("face 0")
            && error.1.contains("vertex 7")
            && error.1.contains("3 vertices"),
        "{}",
        error.1
    );
}

#[test]
fn a_result_that_is_not_a_mesh_says_so() {
    let error = evaluate(&script("42", "")).err().unwrap();
    assert_eq!(error.0, "script.result");
}

#[test]
fn a_point_that_is_not_finite_is_refused_by_index() {
    let error = evaluate(&script(
        "const m = new Mesh(); m.vertex(0, 0, 0); m.vertex(1, 0, 0); m.vertex(0, 1 / 0, 0); m.face(0, 1, 2); m",
        "",
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "script.result");
    assert!(error.1.contains("point 2"), "{}", error.1);
}

#[test]
fn too_many_points_are_refused_with_the_limit() {
    let error = evaluate(&script(
        "const m = new Mesh(); for (let i = 0; i < 250000; i++) m.vertex(i, 0, 0); m.face(0, 1, 2); m",
        "",
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "script.result");
    assert!(error.1.contains("200000"), "{}", error.1);
}
