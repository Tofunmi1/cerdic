//! `poly.sculpt`: brush strokes written as text. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_sculpt").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A flat 4 m grid of 64 by 64 quads in the XY plane facing +Z (the vertex at the origin is a grid point).
const GRID: &str = "g = poly.grid@1(width=4, height=4, columns=64, rows=64)";

fn sculpt(base: &str, node: &str) -> Vec<[f32; 3]> {
    evaluate(&format!("{base}\noutput o = {node}\n"))
        .unwrap()
        .parts[0]
        .positions
        .clone()
}

fn flat(strokes: &str, extra: &str) -> Vec<[f32; 3]> {
    sculpt(
        &format!("{GRID}\ns = poly.sculpt@1(g, strokes=\"{strokes}\"{extra})"),
        "s",
    )
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn radius_xy(p: &[f32; 3], cx: f32, cy: f32) -> f32 {
    ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
}

#[test]
fn draw_lifts_the_centre_by_the_strength_and_leaves_everything_outside_the_radius_alone() {
    let moved = flat("draw 0 0 0 0.5 0.1", "");
    let top = moved.iter().map(|p| p[2]).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        near(top, 0.1, 0.002),
        "the centre rises by the strength: {top}"
    );
    assert!(
        moved
            .iter()
            .filter(|p| p[2].abs() > 1e-6)
            .all(|p| radius_xy(p, 0.0, 0.0) < 0.5),
        "only points within the radius moved"
    );
}

#[test]
fn a_negative_strength_digs_in() {
    let low = flat("draw 0 0 0 0.5 -0.1", "")
        .iter()
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min);
    assert!(near(low, -0.1, 0.002), "{low}");
}

#[test]
fn the_falloff_is_smooth_so_a_vertex_halfway_out_moves_less_than_the_centre() {
    let moved = flat("draw 0 0 0 1 0.2", "");
    let height_at = |r: f32| {
        moved
            .iter()
            .filter(|p| p[1].abs() < 1e-4 && (p[0] - r).abs() < 0.04)
            .map(|p| p[2])
            .fold(0.0_f32, f32::max)
    };
    let (centre, half, edge) = (height_at(0.0), height_at(0.5), height_at(0.98));
    assert!(near(centre, 0.2, 0.003), "{centre}");
    assert!(
        half < centre * 0.7 && half > centre * 0.2,
        "halfway out it is part way: {half}"
    );
    assert!(edge < 0.005, "it dies away at the radius: {edge}");
}

#[test]
fn grab_drags_the_points_under_it_by_the_vector() {
    let moved = flat("grab 0 0 0 0.5 0.2 0 0.1", "");
    assert!(
        moved
            .iter()
            .any(|p| near(p[0], 0.2, 1e-4) && near(p[1], 0.0, 1e-4) && near(p[2], 0.1, 1e-4)),
        "the centre vertex ended at (0.2, 0, 0.1)"
    );
}

#[test]
fn smooth_flattens_a_spike() {
    let spiked = format!(
        "{GRID}\nsp = poly.program@1(g, mode=\"offset\", code=\"out = vec3(0.0, 0.0, select(length(p.xy) < 0.01, 0.5, 0.0))\")"
    );
    // The brush is centred on the spike (0.5 m up), halfway between it and the plane, so both lie within its radius.
    let peak = |node: &str| {
        sculpt(
            &format!("{spiked}\ns = poly.sculpt@1(sp, strokes=\"smooth 0 0 0.25 0.5 1\")"),
            node,
        )
        .iter()
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max)
    };
    let before = peak("sp");
    let after = peak("s");
    assert!(near(before, 0.5, 1e-4), "{before}");
    assert!(
        after < 0.3,
        "one smoothing stroke takes the spike well down: {after}"
    );
}

#[test]
fn inflate_pushes_every_point_out_along_its_own_normal() {
    let setup = "b = solid.sphere@1(radius=1, segments=32)\nm = poly.from_solid@1(b)\ns = poly.sculpt@1(m, strokes=\"inflate 0 1 0 0.5 0.1\")";
    let outermost = |node: &str| {
        sculpt(setup, node)
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
            .fold(0.0_f32, f32::max)
    };
    assert!(outermost("m") < 1.001, "a sphere of radius 1");
    assert!(
        outermost("s") > 1.08,
        "the top is pushed out by about 0.1: {}",
        outermost("s")
    );
}

#[test]
fn flatten_levels_a_bumpy_patch() {
    let bumpy = format!(
        "{GRID}\nb = poly.program@1(g, mode=\"offset\", code=\"out = vec3(0.0, 0.0, 0.2 * sin(p.x * 14.0) * cos(p.y * 11.0))\")"
    );
    let spread = |node: &str, strokes: &str| {
        let src = format!("{bumpy}\ns = poly.sculpt@1(b, strokes=\"{strokes}\")");
        let inside: Vec<f32> = sculpt(&src, node)
            .iter()
            .filter(|p| radius_xy(p, 0.0, 0.0) < 0.3)
            .map(|p| p[2])
            .collect();
        inside.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            - inside.iter().cloned().fold(f32::INFINITY, f32::min)
    };
    let before = spread("b", "flatten 0 0 0 0.6 1");
    let after = spread("s", "flatten 0 0 0 0.6 1");
    assert!(
        after < before * 0.5,
        "the bumps inside the brush are levelled: {after} from {before}"
    );
}

#[test]
fn pinch_pulls_points_toward_the_centre() {
    // Points are drawn in toward the centre (the ones at the radius hardly move, by design), so more of them end up near it.
    let crowded = |points: &[[f32; 3]]| {
        points
            .iter()
            .filter(|p| radius_xy(p, 0.0, 0.0) < 0.3)
            .count()
    };
    let before = crowded(&flat("draw 0 0 0 0.1 0", ""));
    let after = crowded(&flat("pinch 0 0 0 0.6 1", ""));
    assert!(
        after > before + 10,
        "more points within 0.3 of the centre after pinching: {before} then {after}"
    );
}

#[test]
fn symmetry_mirrors_every_stroke_about_the_plane() {
    let moved = flat("draw 1 0 0 0.4 0.1", ", symmetry=\"x\"");
    let lift = |x: f32| {
        moved
            .iter()
            .filter(|p| near(p[0], x, 0.01) && p[1].abs() < 0.01)
            .map(|p| p[2])
            .fold(0.0_f32, f32::max)
    };
    assert!(
        near(lift(1.0), 0.1, 0.003) && near(lift(-1.0), 0.1, 0.003),
        "{} and {}",
        lift(1.0),
        lift(-1.0)
    );
}

#[test]
fn surface_falloff_does_not_sculpt_a_nearby_disconnected_shell() {
    let obj = "v -0.05 -0.05 0\nv 0.05 -0.05 0\nv 0.05 0.05 0\nv -0.05 0.05 0\nv -0.05 -0.05 0.08\nv 0.05 -0.05 0.08\nv 0.05 0.05 0.08\nv -0.05 0.05 0.08\nf 1 2 3 4\nf 5 6 7 8\n";
    let data = obj.replace('\n', "\\n");
    let source = format!(
        "m = poly.import@1(format=\"obj\", data=\"{data}\")\ns = poly.sculpt@1(m, strokes=\"inflate 0 0 0 0.3 0.05\", falloff=\"surface\")\noutput o = s\n"
    );
    let moved = sculpt(&source, "s");
    assert!(
        moved.iter().any(|p| p[2] > 0.015 && p[2] < 0.08),
        "the near shell receives the brush"
    );
    assert!(
        moved.iter().any(|p| near(p[2], 0.08, 1.0e-6)),
        "the disconnected shell behind it stays unchanged"
    );
}

#[test]
fn surface_falloff_does_not_pull_through_the_back_of_a_connected_shell() {
    let source = "box = solid.box(size=[0.4,0.4,0.08])\ntri = poly.from_solid(box)\ncage = poly.merge_coplanar(tri)\ndense = poly.subdivide(cage, levels=2)\nthrough = poly.sculpt(dense, strokes=\"inflate 0 0 0.04 0.2 0.1\", falloff=\"sphere\")\non_surface = poly.sculpt(dense, strokes=\"inflate 0 0 0.04 0.2 0.1\", falloff=\"surface\")\noutput through\noutput on_surface";
    let result = evaluate(source).unwrap();
    let through = &result.parts[0].positions;
    let on_surface = &result.parts[1].positions;
    assert_eq!(through.len(), on_surface.len());
    let back_center = |positions: &[[f32; 3]]| {
        positions
            .iter()
            .min_by(|a, b| {
                let da = a[0] * a[0] + a[1] * a[1] + (a[2] + 0.04).powi(2);
                let db = b[0] * b[0] + b[1] * b[1] + (b[2] + 0.04).powi(2);
                da.total_cmp(&db)
            })
            .map_or(f32::NAN, |point| point[2])
    };
    let (sphere_back, surface_back) = (back_center(through), back_center(on_surface));
    assert!(
        sphere_back < surface_back - 0.02,
        "sphere reaches through: {sphere_back} vs {surface_back}"
    );
}

#[test]
fn mask_and_unmask_strokes_protect_and_restore_geometry() {
    let center_height = |middle: &str| {
        evaluate(&format!(
            "{GRID}\nmask = poly.sculpt@1(g, strokes=\"mask 0 0 0 0.5 1\")\n{middle}\noutput o = final\n"
        ))
            .unwrap()
            .parts[0]
            .positions
            .iter()
            .find(|p| p[0].abs() < 1.0e-6 && p[1].abs() < 1.0e-6)
            .map_or(f32::NAN, |p| p[2])
    };
    let protected = center_height("final = poly.sculpt@1(mask, strokes=\"draw 0 0 0 0.5 0.1\")");
    let restored = center_height(
        "unmasked = poly.sculpt@1(mask, strokes=\"unmask 0 0 0 0.5 1\")\nfinal = poly.sculpt@1(unmasked, strokes=\"draw 0 0 0 0.5 0.1\")",
    );
    assert!(
        near(protected, 0.0, 1.0e-6),
        "the mask holds the centre: {protected}"
    );
    assert!(
        near(restored, 0.1, 0.002),
        "unmask restores sculpting: {restored}"
    );
}

#[test]
fn the_same_strokes_give_the_same_mesh() {
    let strokes = "draw 0.3 0.2 0 0.6 0.1; smooth 0 0 0 0.8 0.5; grab -0.5 0.5 0 0.5 0 0 0.2";
    assert_eq!(flat(strokes, ""), flat(strokes, ""));
}

#[test]
fn a_mistake_in_a_stroke_names_the_stroke_and_the_fix() {
    let run = |strokes: &str| {
        evaluate(&format!(
            "{GRID}\ns = poly.sculpt@1(g, strokes=\"{strokes}\")\noutput o = s\n"
        ))
        .err()
        .unwrap()
    };
    let unknown = run("draw 0 0 0 0.3 0.05; carve 0 0 0 0.3 0.1");
    assert_eq!(unknown.0, "poly.sculpt_stroke");
    assert!(
        unknown.1.contains("stroke 1")
            && unknown.1.contains("carve")
            && unknown.1.contains("inflate"),
        "{}",
        unknown.1
    );
    let count = run("draw 0 0 0 0.3");
    assert!(count.1.contains("needs 5"), "{}", count.1);
    let radius = run("draw 0 0 0 0 0.1");
    assert!(radius.1.contains("radius"), "{}", radius.1);
    let word = run("draw 0 0 high 0.3 0.1");
    assert!(word.1.contains("high"), "{}", word.1);
}

#[test]
fn more_than_the_most_strokes_is_refused() {
    let many = vec!["draw 0 0 0 0.1 0.01"; 65].join("; ");
    let error = evaluate(&format!(
        "{GRID}\ns = poly.sculpt@1(g, strokes=\"{many}\")\noutput o = s\n"
    ))
    .err()
    .unwrap();
    assert!(error.1.contains("64"), "{}", error.1);
}
