//! `field.noise`: a bounded displacement of a field's surface, so wrinkles, scales and veins exist before meshing.
//! Every assertion here is a number worked out by hand: a sphere of radius r has volume 4/3 pi r^3, and a surface moved by at most `a`
//! lies between the spheres of radius r - a and r + a.
#![expect(
    clippy::unwrap_used,
    reason = "the helpers unwrap fixtures that are known to be valid"
)]
use std::f64::consts::PI;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

const BOUND: &str = "min=[-0.56, -0.56, -0.56], max=[0.56, 0.56, 0.56], edge_length=0.02";

fn sphere_volume(radius: f64) -> f64 {
    4.0 / 3.0 * PI * radius.powi(3)
}

/// Meshes `field.noise` (with `noise_params`) of a sphere of radius 0.5 and returns the part's volume, or the failure code.
fn volume(noise_params: &str, bound: &str) -> Result<f64, String> {
    let source = format!(
        "ball = field.sphere@1(radius=0.5)\n\
         rough = field.noise@1(ball, {noise_params})\n\
         body = field.to_solid@1(rough, {bound})\n\
         output body = body\n"
    );
    let commands = parse_text(&builtin_registry(), &source).unwrap().commands;
    let mut session = Session::with_builtin(3);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_field_noise").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(result) => Ok(result.parts[0].volume),
        Err(error) => Err(format!("{error:?}")),
    }
}

#[test]
fn the_plain_sphere_has_the_analytic_volume() {
    // The baseline the other tests lean on: 0.5236 for radius 0.5, within the meshing error.
    let plain = volume("amplitude=0", BOUND).unwrap();
    assert!(
        (plain - sphere_volume(0.5)).abs() / sphere_volume(0.5) < 0.03,
        "{plain} against {}",
        sphere_volume(0.5)
    );
}

#[test]
fn zero_amplitude_changes_nothing() {
    let plain = volume("amplitude=0", BOUND).unwrap();
    let again = volume("amplitude=0, scale=0.1, octaves=4, seed=9", BOUND).unwrap();
    assert!((plain - again).abs() < 1e-9, "{plain} against {again}");
}

#[test]
fn the_surface_never_moves_further_than_the_amplitude() {
    // Moved by at most 0.05, the surface lies between radius 0.45 and 0.55, so the volume does too (with the 3 percent meshing slack).
    for (scale, octaves, mode) in [(0.3, 1, "smooth"), (0.12, 3, "smooth"), (0.2, 2, "ridged")] {
        let v = volume(
            &format!("amplitude=0.05, scale={scale}, octaves={octaves}, mode=\"{mode}\", seed=2"),
            BOUND,
        )
        .unwrap();
        assert!(
            v > sphere_volume(0.45) * 0.97 && v < sphere_volume(0.55) * 1.03,
            "scale {scale}, {octaves} octaves, {mode}: {v} not in [{}, {}]",
            sphere_volume(0.45),
            sphere_volume(0.55)
        );
    }
}

#[test]
fn noise_does_change_the_shape_and_is_deterministic() {
    let plain = volume("amplitude=0", BOUND).unwrap();
    let a = volume("amplitude=0.05, scale=0.15, seed=1", BOUND).unwrap();
    let a_again = volume("amplitude=0.05, scale=0.15, seed=1", BOUND).unwrap();
    let b = volume("amplitude=0.05, scale=0.15, seed=2", BOUND).unwrap();
    assert!(
        (a - a_again).abs() < 1e-12,
        "same seed, same shape: {a} and {a_again}"
    );
    assert!(
        (a - b).abs() > 1e-6,
        "another seed, another shape: {a} and {b}"
    );
    assert!(
        (a - plain).abs() > 1e-6,
        "the noise moved something: {a} against {plain}"
    );
}

#[test]
fn a_larger_amplitude_than_the_bounds_allow_is_refused_with_the_clipping_diagnostic() {
    // Amplitude 0.3 pushes the surface out to 0.8, past the 0.56 bounds: the mesher must say so, not return a cut-off body.
    let result = volume("amplitude=0.3, scale=0.2, seed=3", BOUND);
    let message = result.unwrap_err();
    assert!(message.contains("field.clipped"), "{message}");
}

#[test]
fn the_operator_is_in_the_registry_with_its_parameters() {
    let registry = builtin_registry();
    let text = format!("{registry:?}");
    assert!(text.contains("field.noise"), "field.noise is registered");
    for parameter in [
        "amplitude",
        "scale",
        "octaves",
        "persistence",
        "seed",
        "mode",
    ] {
        assert!(
            text.contains(parameter),
            "parameter {parameter} is documented"
        );
    }
}
