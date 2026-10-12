//! G4a: the small operators a model needs to build furniture and machines.
#![expect(
    clippy::unwrap_used,
    reason = "case helpers unwrap fixtures that are known to be valid"
)]
use serde_json::json;
use std::f64::consts::PI;
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn session(program: &str) -> Session {
    let mut session = Session::with_builtin(6);
    let batch = parse_text(&builtin_registry(), program).unwrap();
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_shapes", "commands": batch.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    session
}

fn run(program: &str) -> Evaluation {
    Evaluator::default()
        .evaluate(session(program).document(), Quality::Final)
        .unwrap()
}

fn volume(program: &str) -> f64 {
    run(program).parts[0].volume
}

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b.abs()
}

fn bounds(result: &Evaluation) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in &result.parts[0].positions {
        for k in 0..3 {
            lo[k] = lo[k].min(f64::from(p[k]));
            hi[k] = hi[k].max(f64::from(p[k]));
        }
    }
    (lo, hi)
}

#[test]
fn rect_and_circle_profiles_extrude_to_their_analytic_volumes() {
    let slab =
        volume("r = profile.rect@1(size=[0.2, 0.1])\ne = solid.extrude@1(r, height=1)\noutput e\n");
    assert!(near(slab, 0.02, 1e-6), "meshes are f32: {slab}");
    let disc = volume(
        "c = profile.circle@1(radius=0.5, segments=256)\ne = solid.extrude@1(c, height=1)\noutput e\n",
    );
    assert!(near(disc, PI * 0.25, 1e-3), "{disc}");
}

#[test]
fn cone_torus_and_capsule_match_their_formulas() {
    let frustum =
        volume("c = solid.cone@1(height=1, radius=0.5, top_radius=0.25, segments=256)\noutput c\n");
    assert!(
        near(frustum, PI / 3.0 * (0.25 + 0.125 + 0.0625), 3e-3),
        "{frustum}"
    );
    let cone = volume("c = solid.cone@1(height=1, radius=0.5, segments=256)\noutput c\n");
    assert!(near(cone, PI * 0.25 / 3.0, 3e-3), "{cone}");
    let torus = volume(
        "t = solid.torus@1(major=0.5, minor=0.1, segments=128, tube_segments=64)\noutput t\n",
    );
    assert!(near(torus, 2.0 * PI * PI * 0.5 * 0.01, 8e-3), "{torus}");
    let capsule = volume("c = solid.capsule@1(radius=0.1, height=0.5, segments=96)\noutput c\n");
    assert!(
        near(capsule, PI * 0.01 * 0.3 + 4.0 / 3.0 * PI * 0.001, 1.5e-2),
        "{capsule}"
    );
    let result = run("c = solid.capsule@1(radius=0.1, height=0.5, segments=96)\noutput c\n");
    let (lo, hi) = bounds(&result);
    assert!(near(hi[1] - lo[1], 0.5, 2e-3), "the capsule is 0.5 m long");
}

#[test]
fn a_torus_must_have_a_tube_smaller_than_its_ring() {
    let program = "t = solid.torus@1(major=0.1, minor=0.2)\noutput t\n";
    let error = Evaluator::default()
        .evaluate(session(program).document(), Quality::Preview)
        .unwrap_err();
    assert_eq!(error.code, "kernel.invalid_input");
}

#[test]
fn mirror_adds_the_mirror_image_across_the_origin_plane() {
    let apart = run(
        "b = solid.box@1(size=[1, 1, 1])\nm = xform.move@1(b, by=[1, 0, 0])\nr = xform.mirror@1(m, axis=\"x\")\noutput r\n",
    );
    assert!(near(apart.parts[0].volume, 2.0, 1e-6));
    let (lo, hi) = bounds(&apart);
    assert!(
        near(hi[0], 1.5, 1e-5) && near(lo[0], -1.5, 1e-5),
        "{lo:?} {hi:?}"
    );
    let overlapping = volume(
        "b = solid.box@1(size=[1, 1, 1])\nm = xform.move@1(b, by=[0.25, 0, 0])\nr = xform.mirror@1(m, axis=\"x\")\noutput r\n",
    );
    assert!(
        near(overlapping, 1.5, 1e-6),
        "overlap is merged: {overlapping}"
    );
    let up = run(
        "b = solid.box@1(size=[1, 1, 1])\nm = xform.move@1(b, by=[0, 2, 0])\nr = xform.mirror@1(m, axis=\"y\")\noutput r\n",
    );
    assert!(near(up.parts[0].volume, 2.0, 1e-6));
}

#[test]
fn array_makes_a_grid_and_radial_a_ring_and_repeat_a_staircase() {
    let grid = run(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\na = xform.array@1(b, count_x=3, count_y=2, spacing=[0.2, 0.2, 0.2])\noutput a\n",
    );
    assert!(near(grid.parts[0].volume, 6.0e-3, 1e-6));
    let (lo, hi) = bounds(&grid);
    assert!(
        near(hi[0] - lo[0], 0.5, 1e-5)
            && near(hi[1] - lo[1], 0.3, 1e-5)
            && near(hi[2] - lo[2], 0.1, 1e-5),
        "{lo:?} {hi:?}"
    );
    let ring = run(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\na = xform.radial@1(b, count=6, radius=1)\noutput a\n",
    );
    assert!(near(ring.parts[0].volume, 6.0e-3, 1e-6));
    let (lo, hi) = bounds(&ring);
    assert!(
        (hi[0] - 1.05).abs() < 1e-3 && (lo[0] + 1.05).abs() < 1e-3,
        "six copies reach out to 1.05 m each way: {lo:?} {hi:?}"
    );
    let stairs = run(
        "b = solid.box@1(size=[0.4, 0.05, 0.2])\na = xform.repeat@1(b, count=5, step=[0, 0.18, 0], rotate_step=[0, 20, 0])\noutput a\n",
    );
    assert!(near(stairs.parts[0].volume, 5.0 * 0.4 * 0.05 * 0.2, 1e-6));
    let (lo, hi) = bounds(&stairs);
    assert!(
        near(hi[1] - lo[1], 4.0 * 0.18 + 0.05, 1e-5),
        "five steps climb 0.72 m plus a tread: {lo:?} {hi:?}"
    );
}

#[test]
fn patterns_are_budgeted_before_any_geometry_is_built() {
    let program = "s = solid.sphere@1(radius=0.05, segments=128)\na = xform.array@1(s, count_x=40, count_y=40, count_z=4, spacing=[0.2, 0.2, 0.2])\noutput a\n";
    let mut evaluator = Evaluator::default();
    let error = evaluator
        .evaluate(session(program).document(), Quality::Preview)
        .unwrap_err();
    assert!(error.code.starts_with("budget."), "{}", error.code);
    assert_eq!(evaluator.evaluated_nodes(), 0);
    let too_many = "b = solid.box@1(size=[1, 1, 1])\na = xform.radial@1(b, count=1000)\noutput a\n";
    assert_eq!(
        parse_text(&builtin_registry(), too_many).unwrap_err()[0].code,
        "param.range"
    );
}

#[test]
fn patterns_keep_the_first_inputs_surface() {
    let result = run(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\nm = solid.material@1(b, color=[0.1, 0.4, 0.9])\na = xform.array@1(m, count_x=3, spacing=[0.2, 0, 0])\nr = xform.mirror@1(a, axis=\"z\")\noutput r\n",
    );
    let material = result.parts[0].material.as_ref().unwrap();
    assert!((material.color[2] - 0.9).abs() < 1e-6);
}
