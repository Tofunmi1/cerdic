//! Shear, wave, noise and lattice deformers through the evaluator: volumes, extents, determinism and refusals.
use serde_json::{Value, json};
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn evaluate(op: &str, params: &Value, size: [f64; 3]) -> Result<ve_modeling::Evaluation, String> {
    let mut session = Session::with_builtin(3);
    let batch = json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_deform_more",
        "commands": [
            {"add_node": {"op": "solid.box@1", "params": {"size": size}, "alias": "$box"}},
            {"add_node": {"op": op, "params": params, "alias": "$deformed"}},
            {"connect": {"from": {"node": "$box", "port": "out"}, "to": {"node": "$deformed", "port": "in"}}},
            {"set_output": {"name": "part", "from": {"node": "$deformed", "port": "out"}}}
        ]
    });
    if let outcome @ Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err(format!("rejected: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

const TALL: [f64; 3] = [0.2, 1.0, 0.2];

fn extent(positions: &[[f32; 3]], axis: usize) -> (f32, f32) {
    positions.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p[axis]), hi.max(p[axis]))
    })
}

#[test]
fn shear_keeps_the_volume_and_slides_the_top() {
    let result = evaluate(
        "xform.shear@1",
        &json!({"x_shift": 0.3, "z_shift": 0.0}),
        TALL,
    )
    .unwrap();
    let part = &result.parts[0];
    assert!(
        (part.volume - 0.04).abs() < 1e-5,
        "a shear preserves volume: {}",
        part.volume
    );
    let (lo, hi) = extent(&part.positions, 0);
    assert!(
        (lo + 0.1).abs() < 1e-5 && (hi - 0.4).abs() < 1e-5,
        "x runs from -0.1 at the bottom to 0.4 at the top: {lo} {hi}"
    );
}

#[test]
fn a_wave_ripples_a_thin_panel_and_a_gentle_noise_keeps_it_one_solid() {
    let panel = [1.0, 1.0, 0.05];
    let wave = evaluate(
        "xform.wave@1",
        &json!({"amplitude": 0.1, "wavelength": 0.5, "direction": "z", "tolerance": 0.005}),
        panel,
    );
    let part = &wave.as_ref().unwrap().parts[0];
    let (lo, hi) = extent(&part.positions, 2);
    assert!(
        hi - lo > 0.15,
        "the panel now spans the ripples in Z: {lo} {hi}"
    );
    assert!(
        (part.volume - 0.05).abs() < 3e-3,
        "a ripple keeps the volume: {}",
        part.volume
    );
}

#[test]
fn noise_is_repeatable_depends_on_the_seed_and_stays_near_the_original_size() {
    let blob = [1.0, 1.0, 1.0];
    let run = |seed: u32| {
        evaluate(
            "xform.noise@1",
            &json!({"amplitude": 0.03, "scale": 0.6, "seed": seed, "tolerance": 0.02}),
            blob,
        )
        .unwrap()
    };
    let (a, b, c) = (run(5), run(5), run(6));
    assert_eq!(
        a.parts[0].positions, b.parts[0].positions,
        "same seed, same shape"
    );
    assert_ne!(
        a.parts[0].positions, c.parts[0].positions,
        "another seed, another shape"
    );
    assert!(
        (a.parts[0].volume - 1.0).abs() < 0.1,
        "volume {}",
        a.parts[0].volume
    );
}

#[test]
fn a_lattice_without_offsets_changes_nothing_and_a_pulled_corner_grows_the_solid() {
    let plain = evaluate("xform.lattice@1", &json!({"divisions": 3}), TALL).unwrap();
    assert!((plain.parts[0].volume - 0.04).abs() < 1e-6);
    let pulled = evaluate("xform.lattice@1", &json!({"divisions": 2, "offsets": "1 1 1 0 0.5 0; 0 1 0 0 0.5 0; 1 1 0 0 0.5 0; 0 1 1 0 0.5 0"}), TALL).unwrap();
    let part = &pulled.parts[0];
    let (_, top) = extent(&part.positions, 1);
    assert!(
        (top - 1.0).abs() < 1e-5,
        "the top face moved up by 0.5 from y=0.5: {top}"
    );
    assert!(
        (part.volume - 0.06).abs() < 1e-4,
        "stretching the top by half the height adds half the volume: {}",
        part.volume
    );
}

#[test]
fn bad_lattice_offsets_and_folding_deformations_are_refused_with_a_reason() {
    for bad in ["5 0 0 0 0 0", "0 0 0 1 2", "0 0 0 a b c", "0 0 0 0 0 5000"] {
        let error = evaluate(
            "xform.lattice@1",
            &json!({"divisions": 3, "offsets": bad}),
            TALL,
        )
        .unwrap_err();
        assert!(
            error.contains("param.") || error.contains("lattice"),
            "{bad}: {error}"
        );
    }
    let fold = evaluate(
        "xform.noise@1",
        &json!({"amplitude": 0.8, "scale": 0.1, "seed": 1}),
        [1.0, 1.0, 1.0],
    )
    .unwrap_err();
    assert!(
        fold.contains("geometry.deformation") || fold.contains("budget.triangles"),
        "a violent noise is refused, not silently wrong: {fold}"
    );
}
