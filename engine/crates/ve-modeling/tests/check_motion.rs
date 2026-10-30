//! `check.motion`: samples the animation of some parts over a clip and reports what a model cannot see: a foot that skates while it should be
//! planted, and a loop that pops at its seam. Built on `ve_measure::motion` (`foot_slide`, `loop_seam`). Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(17);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_motion").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

/// A box foot standing on the ground (its bottom at y = 0) that slides 1 m back along x over one second, looping: a walking foot seen in
/// place. `check` is the text of the check node (it can use `foot_part` and `body`).
fn world(keys: &str, check: &str) -> String {
    format!(
        "foot0 = solid.box@1(size=[0.2, 0.1, 0.2])\n\
         foot1 = xform.move@1(foot0, by=[0, 0.05, 0])\n\
         foot_t = anim.track@1(foot1, property=\"position\", keys=\"{keys}\", interpolation=\"linear\", loop=true, clip=\"walk\")\n\
         {check}\n\
         output foot_part = foot_t\n"
    )
}

const BACKWARD: &str = "0:0.5,0,0; 1:-0.5,0,0";
const PING_PONG: &str = "0:0.5,0,0; 0.5:-0.5,0,0; 1:0.5,0,0";

fn codes(ev: &Evaluation) -> Vec<&str> {
    ev.warnings.iter().map(|w| w.code.as_str()).collect()
}

#[test]
fn a_planted_foot_passes_and_a_skating_foot_is_reported() {
    // The foot moves back at 1 m/s relative to the body. A body walking forward at 1 m/s makes that foot stand still in the world.
    let planted = evaluate(&world(
        BACKWARD,
        "m = check.motion@1(foot_t, feet=\"foot_part\", clip=\"walk\", duration=0.9, speed=1.0)",
    ))
    .unwrap();
    assert!(
        !codes(&planted).contains(&"motion.foot_slide"),
        "{:?}",
        planted.warnings
    );
    // The same clip with a body that does not travel: the foot sweeps the ground at 1 m/s.
    let skating = evaluate(&world(
        BACKWARD,
        "m = check.motion@1(foot_t, feet=\"foot_part\", clip=\"walk\", duration=0.9, speed=0)",
    ))
    .unwrap();
    let warning = skating
        .warnings
        .iter()
        .find(|w| w.code == "motion.foot_slide")
        .expect("a skating foot is reported");
    assert!(warning.message.contains("foot_part"), "{}", warning.message);
    assert!(
        warning.message.contains("1.0")
            || warning.message.contains("0.9")
            || warning.message.contains("1.00"),
        "the message gives the speed: {}",
        warning.message
    );
}

#[test]
fn the_tolerance_decides() {
    let source = |max: &str| {
        world(
            BACKWARD,
            &format!(
                "m = check.motion@1(foot_t, feet=\"foot_part\", clip=\"walk\", duration=0.9, speed=0.9, slide_max={max})"
            ),
        )
    };
    // The body travels at 0.9 m/s against a foot at 1.0 m/s: 0.1 m/s of skate.
    assert!(codes(&evaluate(&source("0.05")).unwrap()).contains(&"motion.foot_slide"));
    assert!(!codes(&evaluate(&source("0.5")).unwrap()).contains(&"motion.foot_slide"));
}

#[test]
fn a_loop_that_ends_where_it_began_is_seamless_and_a_sawtooth_pops() {
    let smooth = evaluate(&world(
        PING_PONG,
        "m = check.motion@1(foot_t, clip=\"walk\", duration=1.0)",
    ))
    .unwrap();
    assert!(
        !codes(&smooth).contains(&"motion.loop_seam"),
        "{:?}",
        smooth.warnings
    );
    let pops = evaluate(&world(
        BACKWARD,
        "m = check.motion@1(foot_t, clip=\"walk\", duration=1.0)",
    ))
    .unwrap();
    let warning = pops
        .warnings
        .iter()
        .find(|w| w.code == "motion.loop_seam")
        .expect("a sawtooth clip pops at its seam");
    assert!(warning.message.contains("walk"), "{}", warning.message);
}

#[test]
fn a_foot_follows_the_part_it_is_attached_to() {
    // The foot has no motion of its own but follows a body that moves 1 m/s along x: in the world it slides.
    let source = "body0 = solid.box@1(size=[1, 0.5, 1])\n\
                  body1 = xform.move@1(body0, by=[0, 0.5, 0])\n\
                  body_t = anim.track@1(body1, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\", interpolation=\"linear\", loop=true, clip=\"walk\")\n\
                  foot0 = solid.box@1(size=[0.2, 0.1, 0.2])\n\
                  foot1 = xform.move@1(foot0, by=[0, 0.05, 0])\n\
                  foot_p = anim.parent@1(foot1, parent=\"body\")\n\
                  m = check.motion@1(foot_p, feet=\"foot_part\", clip=\"walk\", duration=0.9, speed=0)\n\
                  output body = body_t\noutput foot_part = foot_p\n";
    let ev = evaluate(source).unwrap();
    assert!(
        codes(&ev).contains(&"motion.foot_slide"),
        "{:?}",
        ev.warnings
    );
}

#[test]
fn a_clip_nothing_defines_is_reported() {
    let ev = evaluate(&world(
        BACKWARD,
        "m = check.motion@1(foot_t, clip=\"gallop\", duration=1.0)",
    ))
    .unwrap();
    let warning = ev
        .warnings
        .iter()
        .find(|w| w.code == "motion.no_tracks")
        .expect("no track belongs to the clip");
    assert!(warning.message.contains("gallop"), "{}", warning.message);
}

#[test]
fn severity_error_fails_the_evaluation() {
    let error = evaluate(&world(BACKWARD, "m = check.motion@1(foot_t, feet=\"foot_part\", clip=\"walk\", duration=0.9, speed=0, severity=\"error\")")).unwrap_err();
    assert!(error.contains("motion.foot_slide"), "{error}");
}

#[test]
fn bad_parameters_are_refused() {
    for bad in [
        "duration=0",
        "duration=31",
        "fps=4",
        "fps=121",
        "speed=100",
        "slide_max=-1",
        "severity=\"loud\"",
    ] {
        let source = world(
            BACKWARD,
            &format!("m = check.motion@1(foot_t, clip=\"walk\", {bad})"),
        );
        assert!(evaluate(&source).is_err(), "{bad} should be refused");
    }
    let unknown_foot = world(
        BACKWARD,
        "m = check.motion@1(foot_t, feet=\"nowhere\", clip=\"walk\")",
    );
    let error = evaluate(&unknown_foot).unwrap_err();
    assert!(error.contains("nowhere"), "{error}");
}

#[test]
fn integer_values_written_as_numbers_in_a_saved_document_work() {
    let source = world(
        BACKWARD,
        "m = check.motion@1(foot_t, clip=\"walk\", duration=1, fps=30, speed=0, slide_max=1)",
    );
    assert!(evaluate(&source).is_ok());
}

#[test]
fn the_check_does_not_change_the_shapes() {
    let with = evaluate(&world(
        BACKWARD,
        "m = check.motion@1(foot_t, clip=\"walk\", duration=1.0)",
    ))
    .unwrap();
    let without = evaluate(&world(BACKWARD, "")).unwrap();
    assert_eq!(with.parts[0].positions, without.parts[0].positions);
    assert_eq!(with.parts.len(), without.parts.len());
}
