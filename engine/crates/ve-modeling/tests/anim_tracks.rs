//! `anim.track`: shapes keep their geometry and gain validated keyframed tracks.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::anim::{Interpolation, TrackProperty};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, (String, String)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_anim_tracks").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(("rejected".to_owned(), format!("{outcome:?}")));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message))
}

const DOOR: &str = "door = solid.box@1(size=[0.9, 2, 0.05])\n\
    swing = anim.track@1(door, property=\"rotation\", keys=\"0:0,0,0; 1.5:0,90,0\", pivot=[-0.45, 0, 0], loop=false)\n\
    output hinged = swing\n";

#[test]
fn a_door_swings_about_its_hinge_and_keeps_its_shape() {
    let still =
        evaluate("door = solid.box@1(size=[0.9, 2, 0.05])\noutput hinged = door\n").unwrap();
    let moving = evaluate(DOOR).unwrap();
    let (a, b) = (&still.parts[0], &moving.parts[0]);
    assert_eq!(a.positions, b.positions, "the geometry is unchanged");
    assert!((a.volume - b.volume).abs() < 1e-12);
    assert!(a.animation.is_empty());
    let [track] = &b.animation[..] else {
        panic!("one track: {:?}", b.animation)
    };
    assert_eq!(track.property, TrackProperty::Rotation);
    assert_eq!(
        track.interpolation,
        Interpolation::Smooth,
        "smooth is the default"
    );
    assert!(!track.looped);
    assert_eq!(track.pivot, [-0.45, 0.0, 0.0]);
    assert_eq!(track.keys.len(), 2);
    assert!((track.duration() - 1.5).abs() < 1e-6);
    assert!((track.keys[1].value[1] - 90.0).abs() < 1e-6);
}

#[test]
fn chained_tracks_accumulate_and_survive_a_material() {
    let program = "fan = solid.box@1(size=[1, 0.05, 0.1])\n\
        spin = anim.track@1(fan, property=\"rotation\", keys=\"0:0,0,0; 1:0,360,0\", interpolation=\"linear\")\n\
        bob = anim.track@1(spin, property=\"position\", keys=\"0:0,0,0; 0.5:0,0.1,0; 1:0,0,0\")\n\
        painted = solid.material@1(bob, color=[0.2, 0.4, 0.8])\n\
        output fan_out = painted\n";
    let part = &evaluate(program).unwrap().parts[0];
    assert_eq!(part.animation.len(), 2, "both tracks play together");
    assert_eq!(part.animation[0].property, TrackProperty::Rotation);
    assert_eq!(part.animation[0].interpolation, Interpolation::Linear);
    assert_eq!(part.animation[1].property, TrackProperty::Position);
    assert!(
        part.animation[0].looped && part.animation[1].looped,
        "tracks loop by default"
    );
    assert!(part.material.is_some(), "the material is kept");
}

#[test]
fn bad_keys_are_refused_with_a_reason() {
    let attempt = |keys: &str| {
        evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(b, property=\"position\", keys=\"{keys}\")\noutput o = t\n"
        ))
    };
    for bad in [
        "0:0,0,0",
        "1:0,0,0; 1:1,1,1",
        "2:0,0,0; 1:1,1,1",
        "0:0,0; 1:1,1,1",
        "0:0,0,0; 1:1,1,5000",
        "-1:0,0,0; 1:1,1,1",
    ] {
        let error = attempt(bad)
            .err()
            .unwrap_or_else(|| panic!("{bad:?} should be refused"));
        assert!(
            error.1.contains("anim.track") || error.0.contains("param"),
            "{bad:?}: {error:?}"
        );
    }
    assert!(attempt("0:0,0,0; 1:1,1,1").is_ok());
}

#[test]
fn a_behaviour_rides_with_the_part_and_needs_a_track() {
    use ve_modeling::anim::{Action, Trigger};
    let program = format!(
        "{}react = behavior.on@1(swing, trigger=\"near\", action=\"toggle\", radius=3)\noutput hinged2 = react\n",
        DOOR.replace("output hinged = swing\n", "")
    );
    let part = &evaluate(&program).unwrap().parts[0];
    let behavior = part.behavior.clone().unwrap();
    assert_eq!(
        (behavior.trigger, behavior.action),
        (Trigger::Near, Action::Toggle)
    );
    assert!((behavior.radius - 3.0).abs() < 1e-6);
    assert_eq!(part.animation.len(), 1, "the track stays");

    let no_track = "b = solid.box@1(size=[1, 1, 1])\nr = behavior.on@1(b)\noutput o = r\n";
    let error = evaluate(no_track).err().unwrap();
    assert!(error.1.contains("anim.track"), "{error:?}");
    let twice = format!("{program}again = behavior.on@1(react)\noutput o2 = again\n");
    assert!(evaluate(&twice).is_err(), "one behaviour per part");
    let far = DOOR.replace(
        "output hinged = swing\n",
        "r = behavior.on@1(swing, radius=500)\noutput o = r\n",
    );
    assert!(
        parse_text(&builtin_registry(), &far).is_err(),
        "radius is bounded by the spec"
    );
}

const SWITCH_AND_DOOR: &str = "door = solid.box@1(size=[0.9, 2, 0.05])\n\
    swing = anim.track@1(door, property=\"rotation\", keys=\"0:0,0,0; 1:0,90,0\", pivot=[-0.45, 0, 0], loop=false)\n\
    listen = behavior.on@1(swing, trigger=\"signal\")\n\
    output front_door = listen\n\
    button = solid.box@1(size=[0.1, 0.1, 0.02])\n\
    press = behavior.on@1(button, trigger=\"select\", target=\"front_door\")\n\
    output door_button = press\n";

#[test]
fn a_switch_names_the_part_it_runs_and_needs_no_clip_of_its_own() {
    use ve_modeling::anim::Trigger;
    let parts = evaluate(SWITCH_AND_DOOR).unwrap().parts;
    let button = parts.iter().find(|p| p.name == "door_button").unwrap();
    let door = parts.iter().find(|p| p.name == "front_door").unwrap();
    let sent = button.behavior.as_ref().unwrap();
    assert_eq!(
        (sent.trigger, sent.target.as_deref()),
        (Trigger::Select, Some("front_door"))
    );
    assert!(button.animation.is_empty(), "the button has no clip");
    assert_eq!(door.behavior.as_ref().unwrap().trigger, Trigger::Signal);
    assert_eq!(door.animation.len(), 1);
}

#[test]
fn signal_parts_only_listen_and_senders_need_something_to_run() {
    let listener_with_target = "b = solid.box@1(size=[1, 1, 1])\n\
        t = anim.track@1(b, property=\"position\", keys=\"0:0,0,0; 1:0,1,0\")\n\
        r = behavior.on@1(t, trigger=\"signal\", target=\"x\")\noutput o = r\n";
    assert!(
        evaluate(listener_with_target)
            .err()
            .unwrap()
            .1
            .contains("only listens")
    );
    let signal_without_clip =
        "b = solid.box@1(size=[1, 1, 1])\nr = behavior.on@1(b, trigger=\"signal\")\noutput o = r\n";
    assert!(
        evaluate(signal_without_clip)
            .err()
            .unwrap()
            .1
            .contains("anim.track")
    );
}

const ARM: &str = "upper = solid.box@1(size=[0.1, 0.6, 0.1])\n\
    shoulder = anim.track@1(upper, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,45\", pivot=[0, 0.3, 0])\n\
    output upper_arm = shoulder\n\
    lower = solid.box@1(size=[0.1, 0.5, 0.1])\n\
    low_moved = xform.move@1(lower, by=[0, -0.55, 0])\n\
    attached = anim.parent@1(low_moved, parent=\"upper_arm\")\n\
    elbow = anim.track@1(attached, property=\"rotation\", keys=\"0:0,0,0; 1:90,0,0\", pivot=[0, -0.3, 0])\n\
    output forearm = elbow\n";

#[test]
fn a_child_part_names_its_parent_and_keeps_its_own_track() {
    let parts = evaluate(ARM).unwrap().parts;
    let forearm = parts.iter().find(|p| p.name == "forearm").unwrap();
    let upper = parts.iter().find(|p| p.name == "upper_arm").unwrap();
    assert_eq!(forearm.parent.as_deref(), Some("upper_arm"));
    assert_eq!(upper.parent, None);
    assert_eq!(forearm.animation.len(), 1);
}

#[test]
fn parents_that_name_nothing_or_loop_are_dropped_with_a_warning() {
    let missing = "b = solid.box@1(size=[1, 1, 1])\np = anim.parent@1(b, parent=\"ghost\")\noutput lonely = p\n";
    let evaluation = evaluate(missing).unwrap();
    assert_eq!(evaluation.parts[0].parent, None);
    assert!(
        evaluation
            .warnings
            .iter()
            .any(|w| w.code == "anim.parent_missing"),
        "{:?}",
        evaluation.warnings
    );

    let loops = "a = solid.box@1(size=[1, 1, 1])\npa = anim.parent@1(a, parent=\"second\")\noutput first = pa\n\
        b = solid.box@1(size=[1, 1, 1])\npb = anim.parent@1(b, parent=\"first\")\noutput second = pb\n";
    let evaluation = evaluate(loops).unwrap();
    assert!(
        evaluation.parts.iter().all(|p| p.parent.is_none()),
        "a loop is cut"
    );
    assert!(
        evaluation
            .warnings
            .iter()
            .any(|w| w.code == "anim.parent_cycle")
    );

    let own =
        "b = solid.box@1(size=[1, 1, 1])\np = anim.parent@1(b, parent=\"me\")\noutput me = p\n";
    assert!(
        evaluate(own)
            .unwrap()
            .warnings
            .iter()
            .any(|w| w.code == "anim.parent_cycle"),
        "following itself is a loop"
    );
}

#[test]
fn glow_tracks_take_one_number_and_refuse_negative_brightness() {
    let lamp = |keys: &str| {
        evaluate(&format!(
            "bulb = solid.sphere@1(radius=0.1)\nlit = solid.material@1(bulb, color=[1, 0.9, 0.6], emissive=[2, 1.8, 1.2], emits_light=true)\n\
             g = anim.track@1(lit, property=\"glow\", keys=\"{keys}\", loop=false)\noutput bulb_out = g\n"
        ))
    };
    let part = lamp("0:0; 0.5:1")
        .unwrap()
        .parts
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(part.animation[0].property, TrackProperty::Glow);
    assert!(
        (part.animation[0].sample(0.25)[0] - 0.5).abs() < 1e-6,
        "smooth glow is half-lit at the middle"
    );
    assert!(lamp("0:-1; 1:1").is_err(), "negative brightness is refused");
    assert!(lamp("0:0; 1:500").is_err(), "absurd brightness is refused");
}

include!("anim_tracks/part_1.rs");
include!("anim_tracks/part_2.rs");
include!("anim_tracks/part_3.rs");
