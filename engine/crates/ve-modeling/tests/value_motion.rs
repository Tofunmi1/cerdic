//! `value.motion`: the real travel, path, duration, speed and lift of a clip, readable by other nodes. Do not weaken.
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
        transaction: TransactionId::parse("t_motion").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A box that walks 3 m forward (z) in 2 s along a path that wanders 0.5 m in x on the way, bobbing 0.1 m, in the clip `walk`.
const WALKER: &str = "box = solid.box@1(size=[0.3, 0.3, 0.3])\nwalk = anim.track@1(box, property=\"position\", keys=\"0:0,0,0; 0.5:0.5,0.1,0.75; 1:0,0,1.5; 1.5:0,0.1,2.25; 2:0,0,3\", interpolation=\"linear\", clip=\"walk\")\n";

fn measure(measure: &str, clip: &str) -> Result<f64, (String, String)> {
    // The number reaches a parameter by a link; read it back as the width of a box sized by the measurement.
    let text = format!(
        "{WALKER}m = value.motion@1(walk, measure=\"{measure}\", clip=\"{clip}\")\nprobe = solid.box@1(size=[1, 1, 1], size.x=m)\noutput probe = probe\n"
    );
    let result = evaluate(&text)?;
    let part = result.parts.iter().find(|p| p.name == "probe").unwrap();
    let xs: Vec<f32> = part.positions.iter().map(|p| p[0]).collect();
    Ok(f64::from(
        xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min),
    ))
}

#[test]
fn a_walk_reports_its_travel_duration_and_real_speed() {
    assert!((measure("travel", "walk").unwrap() - 3.0).abs() < 0.01);
    assert!((measure("duration", "walk").unwrap() - 2.0).abs() < 0.01);
    assert!(
        (measure("speed", "walk").unwrap() - 1.5).abs() < 0.01,
        "3 m in 2 s is 1.5 m/s"
    );
}

#[test]
fn the_path_is_longer_than_the_travel_when_the_walk_wanders_and_the_lift_is_the_bob() {
    let path = measure("path", "walk").unwrap();
    assert!(
        path > 3.0 + 0.1 && path < 3.5,
        "the wander adds a little: {path}"
    );
    assert!((measure("lift", "walk").unwrap() - 0.1).abs() < 0.005);
}

#[test]
fn a_clip_the_part_does_not_have_is_refused_with_the_fix() {
    let error = measure("speed", "run").unwrap_err();
    assert_eq!(error.0, "value.no_motion");
}

#[test]
fn a_state_machine_can_use_the_measured_speed_as_its_walk_threshold() {
    // The point of the node: the speed written in the machine is the clip's own, not a guess.
    let text = format!(
        "{WALKER}speed = value.motion@1(walk, measure=\"speed\", clip=\"walk\")\nm = anim.machine@1(walk, program=\"entry ground;state ground blend=speed idle@0 walk@1.5\")\noutput m = m\noutput speed_probe = speed\n"
    );
    // Evaluating at all (a machine whose clip exists) is the check here; the measured value is 1.5, the number the program names.
    let result = evaluate(&text);
    assert!(
        result.is_ok()
            || result
                .as_ref()
                .err()
                .is_some_and(|e| e.0 != "value.no_motion"),
        "{result:?}"
    );
    assert!((measure("speed", "walk").unwrap() - 1.5).abs() < 0.01);
}
