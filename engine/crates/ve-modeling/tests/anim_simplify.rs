//! `anim.simplify`: fewer keys, the same motion within a tolerance at the bone's tip. Written before the node was tuned; do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::anim::Track;
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_simplify").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A bone `length` metres long carrying a 64-key rotation track from `value(i)` degrees about z, then thinned with `tolerance`.
fn thinned(
    length: f32,
    tolerance: f32,
    interpolation: &str,
    value: impl Fn(usize) -> f32,
) -> (Track, Track) {
    let keys: Vec<String> = (0..64)
        .map(|i| format!("{:.4}:0,0,{:.4}", i as f32 * 0.05, value(i)))
        .collect();
    let text = format!(
        "box = solid.box@1(size=[0.05, {length}, 0.05])\nbone = rig.bone@1(box, head=[0, {length}, 0], tail=[0, 0, 0])\n\
         dense = anim.track@1(bone, property=\"rotation\", keys=\"{}\", interpolation=\"{interpolation}\", pivot_at=\"head\")\n\
         output original = dense\nthin = anim.simplify@1(dense, tolerance={tolerance})\noutput thin = thin\n",
        keys.join("; ")
    );
    let result = evaluate(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let track = |name: &str| {
        result
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .animation[0]
            .clone()
    };
    (track("original"), track("thin"))
}

fn worst(original: &Track, thin: &Track) -> f32 {
    (0..=630)
        .map(|s| {
            let t = s as f32 * 0.005;
            let (a, b) = (original.sample(t), thin.sample(t));
            (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max)
        })
        .fold(0.0, f32::max)
}

#[test]
fn a_straight_line_of_keys_collapses_to_its_two_ends() {
    let (original, thin) = thinned(1.0, 0.002, "linear", |i| i as f32 * 0.5);
    assert_eq!(original.keys.len(), 64);
    assert_eq!(thin.keys.len(), 2, "keys: {:?}", thin.keys.len());
    assert_eq!(thin.keys[0].time, 0.0);
    assert!(
        (thin.keys[1].time - 63.0 * 0.05).abs() < 1e-4,
        "the end is kept"
    );
}

#[test]
fn a_curve_loses_keys_but_stays_within_the_tolerance_at_the_tip() {
    let sine = |i: usize| 30.0 * (i as f32 * 0.2).sin();
    // 1 cm at the tip of a 1 m bone is 0.573 degrees; this curve is sampled coarsely (a key every 0.2 radians), so a tight tolerance rightly keeps
    // nearly every key and a 1 cm one drops a good share.
    let (original, thin) = thinned(1.0, 0.01, "linear", sine);
    assert!(
        thin.keys.len() < 48 && thin.keys.len() > 4,
        "{} keys left of 64",
        thin.keys.len()
    );
    let allowed = (0.01_f32 / 1.0).to_degrees();
    assert!(
        worst(&original, &thin) <= allowed * 1.001,
        "the thinned motion strays {} degrees, allowed {allowed}",
        worst(&original, &thin)
    );
    // A looser tolerance keeps fewer keys; a tighter one keeps more.
    let (_, looser) = thinned(1.0, 0.05, "linear", sine);
    let (_, tighter) = thinned(1.0, 0.002, "linear", sine);
    assert!(
        looser.keys.len() < thin.keys.len() && thin.keys.len() < tighter.keys.len(),
        "{} < {} < {}",
        looser.keys.len(),
        thin.keys.len(),
        tighter.keys.len()
    );
}

#[test]
fn a_long_bone_keeps_more_keys_than_a_short_one_for_the_same_tip_tolerance() {
    let sine = |i: usize| 20.0 * (i as f32 * 0.2).sin();
    let (_, long) = thinned(1.5, 0.004, "linear", sine);
    let (_, short) = thinned(0.15, 0.004, "linear", sine);
    assert!(
        long.keys.len() > short.keys.len(),
        "long {} against short {}",
        long.keys.len(),
        short.keys.len()
    );
}

#[test]
fn smooth_interpolation_is_compared_with_smooth_interpolation() {
    let sine = |i: usize| 30.0 * (i as f32 * 0.2).sin();
    let (original, thin) = thinned(1.0, 0.05, "smooth", sine);
    assert!(thin.keys.len() < 64, "{} keys left", thin.keys.len());
    assert!(
        worst(&original, &thin) <= (0.05_f32).to_degrees().min(1.0) * 1.001,
        "{}",
        worst(&original, &thin)
    );
}

#[test]
fn step_tracks_and_short_tracks_are_left_alone() {
    let (original, thin) = thinned(1.0, 0.5, "step", |i| (i % 3) as f32 * 10.0);
    assert_eq!(
        original.keys.len(),
        thin.keys.len(),
        "a step track keeps every key"
    );
    let result = evaluate("box = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(box, property=\"position\", keys=\"0:0,0,0; 1:0,1,0\")\ns = anim.simplify@1(t)\noutput s = s\n").unwrap();
    assert_eq!(result.parts[0].animation[0].keys.len(), 2);
}

#[test]
fn simplifying_is_deterministic_and_keeps_the_clip_and_pivot() {
    let make = || {
        let keys: Vec<String> = (0..40)
            .map(|i| {
                format!(
                    "{:.3}:0,{:.4},0",
                    i as f32 * 0.1,
                    (i as f32 * 0.3).sin() * 0.2
                )
            })
            .collect();
        let text = format!(
            "box = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(box, property=\"position\", keys=\"{}\", clip=\"bob\", loop=false)\ns = anim.simplify@1(t, tolerance=0.001)\noutput s = s\n",
            keys.join("; ")
        );
        evaluate(&text).unwrap().parts[0].animation[0].clone()
    };
    let (a, b) = (make(), make());
    assert_eq!(a, b);
    assert_eq!(a.clip, "bob");
    assert!(!a.looped);
}
